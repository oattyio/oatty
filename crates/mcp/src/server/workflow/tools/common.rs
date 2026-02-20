//! Shared helpers for resolving runtime workflows from tool request payloads.

use crate::server::workflow::errors::{internal_error, validation_error_with_violations};
use crate::server::workflow::services::storage::{find_manifest_record, parse_manifest_content};
use anyhow::Result;
use oatty_engine::RegistryCommandRunner;
use oatty_engine::field_paths::{
    is_non_scalar_schema_type, missing_details_from_schema, non_scalar_suggested_next_step, non_scalar_validation_message,
    resolve_schema_path,
};
use oatty_engine::provider::parse_provider_group_and_command;
use oatty_engine::templates::{extract_template_expressions, parse_step_reference_expression};
use oatty_registry::CommandRegistry;
use oatty_types::workflow::{
    RuntimeWorkflow, WorkflowInputDefinition, WorkflowStepDefinition, WorkflowValueProvider, collect_missing_catalog_requirements,
};
use oatty_types::{CommandSpec, SchemaProperty};
use rmcp::model::ErrorData;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn resolve_runtime_workflow(
    workflow_identifier: Option<&str>,
    manifest_content: Option<&str>,
    format_hint: Option<&str>,
) -> Result<RuntimeWorkflow> {
    if let Some(workflow_identifier) = workflow_identifier {
        let Some(record) = find_manifest_record(workflow_identifier)? else {
            return Err(anyhow::anyhow!("workflow '{}' was not found", workflow_identifier));
        };
        return oatty_engine::workflow::document::runtime_workflow_from_definition(&record.definition);
    }

    let manifest_content = manifest_content.ok_or_else(|| anyhow::anyhow!("either workflow_id or manifest_content must be provided"))?;
    let (definition, _) = parse_manifest_content(manifest_content, format_hint)?;
    oatty_engine::workflow::document::runtime_workflow_from_definition(&definition)
}

/// Collects structured preflight violations for workflow command/catalog readiness.
pub fn collect_workflow_preflight_violations(
    workflow: &RuntimeWorkflow,
    command_registry: &Arc<Mutex<CommandRegistry>>,
) -> Result<Vec<Value>, ErrorData> {
    let registry_snapshot = command_registry
        .lock()
        .map_err(|error| {
            internal_error(
                "WORKFLOW_COMMAND_VALIDATION_REGISTRY_LOCK_FAILED",
                format!("registry lock failed: {error}"),
                serde_json::json!({ "workflow_id": workflow.identifier }),
                "Retry workflow validation or run.",
            )
        })?
        .clone();
    let available_catalogs = registry_snapshot.config.catalogs.clone().unwrap_or_default();
    let runner = RegistryCommandRunner::new(registry_snapshot.clone());

    let violations: Vec<Value> = runner
        .validate_workflow_execution_readiness(workflow)
        .into_iter()
        .map(|violation| {
            serde_json::json!({
                "path": format!("steps[{}].run", violation.step_index),
                "rule": violation.code,
                "message": violation.message,
                "step_id": violation.step_id,
                "run": violation.run,
                "next_step": violation.suggested_action,
            })
        })
        .collect();

    let missing_catalog_violations = collect_missing_catalog_requirements(workflow.requires.as_ref(), available_catalogs.as_slice())
        .into_iter()
        .map(|missing_requirement| {
            let source_hint = missing_requirement.requirement.source.clone();
            let source_type_hint = missing_requirement.requirement.source_type.map(|source_type| match source_type {
                oatty_types::workflow::WorkflowCatalogRequirementSourceType::Path => "path".to_string(),
                oatty_types::workflow::WorkflowCatalogRequirementSourceType::Url => "url".to_string(),
            });
            let next_step = if let Some(source) = source_hint.as_deref() {
                format!(
                    "Install required catalog '{}' (vendor '{}') from {}{} and retry.",
                    missing_requirement.requirement.title.as_deref().unwrap_or("<untitled>"),
                    missing_requirement.requirement.vendor,
                    source,
                    source_type_hint
                        .as_deref()
                        .map(|source_type| format!(" (source_type={source_type})"))
                        .unwrap_or_default()
                )
            } else {
                format!(
                    "Install or enable a catalog for vendor '{}'{} and retry.",
                    missing_requirement.requirement.vendor,
                    missing_requirement
                        .requirement
                        .title
                        .as_deref()
                        .map(|title| format!(" with title '{}'", title))
                        .unwrap_or_default()
                )
            };

            serde_json::json!({
                "path": format!("$.requires.catalogs[{}]", missing_requirement.index),
                "rule": "catalog_requirement",
                "message": missing_requirement.reason,
                "vendor": missing_requirement.requirement.vendor,
                "title": missing_requirement.requirement.title,
                "source": source_hint,
                "source_type": source_type_hint,
                "next_step": next_step,
            })
        })
        .collect::<Vec<Value>>();

    let mut all_violations = violations;
    all_violations.extend(missing_catalog_violations);
    all_violations.extend(collect_provider_select_value_field_violations(workflow, &registry_snapshot));
    all_violations.extend(collect_step_template_output_path_violations(workflow, &registry_snapshot));

    Ok(all_violations)
}

/// Collects non-fatal workflow validation warnings.
pub fn collect_workflow_validation_warnings(
    workflow: &RuntimeWorkflow,
    command_registry: &Arc<Mutex<CommandRegistry>>,
) -> Result<Vec<Value>, ErrorData> {
    let registry_snapshot = command_registry
        .lock()
        .map_err(|error| {
            internal_error(
                "WORKFLOW_REGISTRY_LOCK_FAILED",
                error.to_string(),
                serde_json::json!({}),
                "Retry the request.",
            )
        })?
        .clone();

    let mut warnings = collect_quoted_template_non_string_binding_warnings(workflow, &registry_snapshot);
    warnings.extend(collect_conditional_dependency_warnings(workflow));
    warnings.extend(collect_step_template_array_index_warnings(workflow, &registry_snapshot));
    warnings.extend(collect_mutation_preflight_warnings(workflow, &registry_snapshot));
    warnings.extend(collect_endpoint_context_warnings(workflow));
    Ok(warnings)
}

/// Builds a structured invalid-params error when preflight violations exist.
pub fn build_preflight_validation_error(
    workflow_identifier: &str,
    violations: Vec<Value>,
    error_code: &str,
    message: &str,
    suggested_action: &str,
) -> Option<ErrorData> {
    if violations.is_empty() {
        return None;
    }

    Some(validation_error_with_violations(
        error_code,
        message,
        serde_json::json!({
            "workflow_id": workflow_identifier,
            "violation_count": violations.len(),
        }),
        suggested_action,
        violations,
    ))
}

fn collect_provider_select_value_field_violations(workflow: &RuntimeWorkflow, registry: &CommandRegistry) -> Vec<Value> {
    workflow
        .inputs
        .iter()
        .filter_map(|(input_name, definition)| {
            let value_field = definition.select.as_ref().and_then(|select| select.value_field.as_deref())?;
            let value_field = value_field.trim();
            if value_field.is_empty() {
                return None;
            }

            let provider_identifier = provider_identifier(definition)?;
            let (group, command_name) = parse_provider_group_and_command(provider_identifier.as_str())?;
            let command_spec = registry.find_by_group_and_cmd_cloned(&group, &command_name).ok()?;
            let item_schema = provider_item_schema(&command_spec)?;

            match validate_select_value_field(item_schema, value_field) {
                SelectValueFieldValidation::Valid => None,
                SelectValueFieldValidation::NonScalar { resolved_type } => Some(serde_json::json!({
                    "path": format!("$.inputs.{}.select.value_field", input_name),
                    "rule": "provider_select_value_field_non_scalar",
                    "message": non_scalar_validation_message(input_name, value_field, &resolved_type),
                    "input": input_name,
                    "provider": provider_identifier,
                    "value_field": value_field,
                    "next_step": non_scalar_suggested_next_step(),
                })),
                SelectValueFieldValidation::Missing { details } => Some(serde_json::json!({
                    "path": format!("$.inputs.{}.select.value_field", input_name),
                    "rule": "provider_select_value_field_missing",
                    "message": details.validation_message(input_name),
                    "input": input_name,
                    "provider": provider_identifier,
                    "value_field": value_field,
                    "nested_candidates": details.nested_candidates,
                    "available_fields": details.available_fields,
                    "next_step": details.suggested_next_step(),
                })),
            }
        })
        .collect()
}

pub fn provider_identifier(definition: &WorkflowInputDefinition) -> Option<String> {
    definition.provider.as_ref().map(|provider| match provider {
        WorkflowValueProvider::Id(identifier) => identifier.clone(),
        WorkflowValueProvider::Detailed(detailed) => detailed.id.clone(),
    })
}

fn provider_item_schema(command_spec: &CommandSpec) -> Option<&SchemaProperty> {
    let http_spec = command_spec.http()?;
    let output_schema = http_spec.output_schema.as_ref()?;

    if let Some(list_response_path) = http_spec.list_response_path.as_deref()
        && let Some(list_schema) = resolve_schema_path(output_schema, list_response_path)
    {
        return Some(coerce_item_schema(list_schema));
    }

    if output_schema.r#type == "array" {
        return output_schema.items.as_deref();
    }

    if output_schema.r#type == "object"
        && let Some(properties) = output_schema.properties.as_ref()
    {
        let mut array_candidates = properties.values().filter_map(|property| {
            let property = property.as_ref();
            if property.r#type == "array" {
                property.items.as_deref()
            } else {
                None
            }
        });
        if let Some(single_candidate) = array_candidates.next()
            && array_candidates.next().is_none()
        {
            return Some(single_candidate);
        }
    }

    Some(output_schema)
}

fn coerce_item_schema(schema: &SchemaProperty) -> &SchemaProperty {
    if schema.r#type == "array"
        && let Some(items) = schema.items.as_deref()
    {
        return items;
    }
    schema
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum SelectValueFieldValidation {
    Valid,
    Missing {
        details: oatty_engine::field_paths::SelectValueFieldMissingDetails,
    },
    NonScalar {
        resolved_type: String,
    },
}

fn validate_select_value_field(item_schema: &SchemaProperty, value_field: &str) -> SelectValueFieldValidation {
    if let Some(resolved_schema) = resolve_schema_path(item_schema, value_field) {
        if is_non_scalar_schema_type(resolved_schema) {
            return SelectValueFieldValidation::NonScalar {
                resolved_type: resolved_schema.r#type.clone(),
            };
        }
        return SelectValueFieldValidation::Valid;
    }

    let details = missing_details_from_schema(item_schema, value_field);
    SelectValueFieldValidation::Missing { details }
}

#[derive(Debug, Clone)]
struct StepTemplateReference {
    location_path: String,
    expression: String,
    referenced_step_id: String,
    field_path: String,
}

fn collect_step_template_output_path_violations(workflow: &RuntimeWorkflow, registry: &CommandRegistry) -> Vec<Value> {
    let step_lookup: HashMap<&str, &WorkflowStepDefinition> = workflow.steps.iter().map(|step| (step.id.as_str(), step)).collect();
    let mut violations = Vec::new();

    for (step_index, step) in workflow.steps.iter().enumerate() {
        let references = collect_step_template_references(step_index, step);
        for reference in references {
            let Some(referenced_step) = step_lookup.get(reference.referenced_step_id.as_str()) else {
                violations.push(serde_json::json!({
                    "path": reference.location_path,
                    "rule": "step_template_unknown_step",
                    "message": format!(
                        "template reference '{}' points to unknown step '{}'",
                        reference.expression, reference.referenced_step_id
                    ),
                    "step_id": step.id,
                    "referenced_step_id": reference.referenced_step_id,
                    "next_step": "Fix the template to reference an existing step id or update the workflow step identifiers.",
                }));
                continue;
            };

            let output_schema = output_schema_for_step(referenced_step, registry);
            if output_schema_supports_reference(output_schema, reference.field_path.as_str())
                || output_contract_supports_reference(referenced_step, reference.field_path.as_str())
            {
                continue;
            }

            if output_schema.is_none() {
                violations.push(serde_json::json!({
                    "path": reference.location_path,
                    "rule": "step_template_output_unverifiable",
                    "message": format!(
                        "template reference '{}' targets step '{}' but its command output schema is unavailable, so path '{}' cannot be verified",
                        reference.expression,
                        referenced_step.id,
                        reference.field_path
                    ),
                    "step_id": step.id,
                    "run": step.run,
                    "referenced_step_id": referenced_step.id,
                    "referenced_run": referenced_step.run,
                    "field_path": reference.field_path,
                    "template_expression": reference.expression,
                    "next_step": "Use a command with an output schema and reference a concrete output path, or select a different upstream step.",
                }));
                continue;
            }

            if reference.field_path == "value" && referenced_step.output_contract.is_none() {
                violations.push(serde_json::json!({
                    "path": reference.location_path,
                    "rule": "step_template_value_field_missing_output_contract",
                    "message": format!(
                        "template reference '{}' expects synthetic field 'value' on step '{}' but that step does not declare output_contract.fields.name='value'",
                        reference.expression,
                        referenced_step.id
                    ),
                    "step_id": step.id,
                    "run": step.run,
                    "referenced_step_id": referenced_step.id,
                    "referenced_run": referenced_step.run,
                    "field_path": reference.field_path,
                    "template_expression": reference.expression,
                    "next_step": format!(
                        "Reference a concrete output path (for example '${{{{ steps.{}.0.<field> }}}}') or define output_contract.fields with a scalar field named 'value'.",
                        referenced_step.id
                    ),
                }));
                continue;
            }

            if let Some(output_schema) = output_schema {
                let details = missing_details_from_schema(output_schema, reference.field_path.as_str());
                violations.push(serde_json::json!({
                    "path": reference.location_path,
                    "rule": "step_template_output_path_missing",
                    "message": format!(
                        "template reference '{}' uses missing output path '{}' on step '{}'",
                        reference.expression,
                        reference.field_path,
                        referenced_step.id
                    ),
                    "step_id": step.id,
                    "run": step.run,
                    "referenced_step_id": referenced_step.id,
                    "referenced_run": referenced_step.run,
                    "field_path": reference.field_path,
                    "template_expression": reference.expression,
                    "nested_candidates": details.nested_candidates,
                    "available_fields": details.available_fields,
                    "next_step": step_template_missing_path_next_step(referenced_step.id.as_str(), &details),
                }));
            }
        }
    }

    violations
}

fn collect_quoted_template_non_string_binding_warnings(workflow: &RuntimeWorkflow, registry: &CommandRegistry) -> Vec<Value> {
    let mut warnings = Vec::new();

    for (step_index, step) in workflow.steps.iter().enumerate() {
        let Some((group, command_name)) = parse_provider_group_and_command(step.run.as_str()) else {
            continue;
        };
        let Ok(command_specification) = registry.find_by_group_and_cmd_ref(group.as_str(), command_name.as_str()) else {
            continue;
        };

        for (field_name, field_value) in &step.with {
            let Value::String(raw_value) = field_value else {
                continue;
            };
            if !is_exact_template_expression(raw_value) {
                continue;
            }

            let Some(expected_type) = expected_input_type(command_specification, field_name.as_str()) else {
                continue;
            };
            if expected_type == "string" {
                continue;
            }

            warnings.push(serde_json::json!({
                "path": format!("$.steps[{step_index}].with.{field_name}"),
                "rule": "template_expression_quoted_non_string_input",
                "message": format!(
                    "step '{}' input '{}' expects type '{}' but received a quoted template expression '{}'",
                    step.id,
                    field_name,
                    expected_type,
                    raw_value
                ),
                "step_id": step.id,
                "run": step.run,
                "input": field_name,
                "expected_type": expected_type,
                "next_step": format!(
                    "If this input expects structured/non-string data, remove quotes or provide a value compatible with type '{}'.",
                    expected_type
                ),
            }));
        }
    }

    warnings
}

fn collect_conditional_dependency_warnings(workflow: &RuntimeWorkflow) -> Vec<Value> {
    let conditional_steps: HashMap<&str, &WorkflowStepDefinition> = workflow
        .steps
        .iter()
        .filter(|step| step.r#if.as_ref().map(|condition| !condition.trim().is_empty()).unwrap_or(false))
        .map(|step| (step.id.as_str(), step))
        .collect();

    let mut warnings = Vec::new();
    for (step_index, step) in workflow.steps.iter().enumerate() {
        for dependency in &step.depends_on {
            let Some(conditional_dependency) = conditional_steps.get(dependency.as_str()) else {
                continue;
            };
            warnings.push(serde_json::json!({
                "path": format!("$.steps[{step_index}].depends_on"),
                "rule": "depends_on_conditional_step",
                "message": format!(
                    "step '{}' depends on conditional step '{}' (if: '{}'); if '{}' is skipped, '{}' will also be skipped",
                    step.id,
                    conditional_dependency.id,
                    conditional_dependency.r#if.as_deref().unwrap_or(""),
                    conditional_dependency.id,
                    step.id
                ),
                "step_id": step.id,
                "dependency_step_id": conditional_dependency.id,
                "next_step": "Consider removing this dependency if the downstream step can run independently, or keep it intentionally to enforce skip propagation.",
            }));
        }
    }
    warnings
}

fn collect_step_template_array_index_warnings(workflow: &RuntimeWorkflow, registry: &CommandRegistry) -> Vec<Value> {
    let step_lookup: HashMap<&str, &WorkflowStepDefinition> = workflow.steps.iter().map(|step| (step.id.as_str(), step)).collect();
    let mut warnings = Vec::new();

    for (step_index, step) in workflow.steps.iter().enumerate() {
        let references = collect_step_template_references(step_index, step);
        for reference in references {
            let Some(referenced_step) = step_lookup.get(reference.referenced_step_id.as_str()) else {
                continue;
            };
            let Some(output_schema) = output_schema_for_step(referenced_step, registry) else {
                continue;
            };
            if output_schema.r#type != "array" {
                continue;
            }
            let first_segment = reference.field_path.split('.').next().unwrap_or_default();
            let starts_with_index = matches!(first_segment, "[]" | "*") || first_segment.parse::<usize>().is_ok();
            if starts_with_index {
                continue;
            }

            warnings.push(serde_json::json!({
                "path": reference.location_path,
                "rule": "step_template_array_output_missing_index",
                "message": format!(
                    "template reference '{}' targets step '{}' whose output schema is an array; path '{}' likely needs an index prefix",
                    reference.expression,
                    referenced_step.id,
                    reference.field_path
                ),
                "step_id": step.id,
                "referenced_step_id": referenced_step.id,
                "field_path": reference.field_path,
                "next_step": format!(
                    "Use an indexed path such as '${{{{ steps.{}.0.{} }}}}' (or '[]' depending on intent).",
                    referenced_step.id,
                    reference.field_path
                ),
            }));
        }
    }

    warnings
}

/// Collects preflight-oriented warnings for workflows that perform mutating operations.
///
/// These checks are intentionally conservative and non-fatal. They guide authors toward
/// safer migration and reconciliation patterns by encouraging:
/// - read/check steps before mutations,
/// - explicit guards for destructive operations,
/// - existence checks before provisioning resources.
fn collect_mutation_preflight_warnings(workflow: &RuntimeWorkflow, registry: &CommandRegistry) -> Vec<Value> {
    let mut warnings = Vec::new();
    let has_read_step_before_index = build_read_before_index_map(workflow, registry);

    for (step_index, step) in workflow.steps.iter().enumerate() {
        let Some(command_specification) = command_spec_for_run(step.run.as_str(), registry) else {
            continue;
        };
        let Some(http_specification) = command_specification.http() else {
            continue;
        };
        let http_method = http_specification.method.to_ascii_uppercase();
        if !is_mutating_http_method(http_method.as_str()) {
            continue;
        }

        if !has_read_step_before_index[step_index] {
            warnings.push(serde_json::json!({
                "path": format!("$.steps[{step_index}]"),
                "rule": "workflow_mutation_before_preflight",
                "message": format!(
                    "step '{}' performs mutating HTTP method '{}' before any prior read/check step",
                    step.id,
                    http_method
                ),
                "step_id": step.id,
                "run": step.run,
                "method": http_method,
                "next_step": "Add one or more read-only preflight steps (for example list/info/get) before mutating operations.",
            }));
        }

        if step.depends_on.is_empty() && step.r#if.as_ref().map(|condition| condition.trim().is_empty()).unwrap_or(true) {
            warnings.push(serde_json::json!({
                "path": format!("$.steps[{step_index}]"),
                "rule": "mutating_step_missing_guard",
                "message": format!(
                    "step '{}' performs mutating HTTP method '{}' without depends_on or if guard",
                    step.id,
                    http_method
                ),
                "step_id": step.id,
                "run": step.run,
                "method": http_method,
                "next_step": "Gate this step with depends_on and/or an if condition so mutation only runs after explicit preflight checks.",
            }));
        }

        if is_create_operation(step.run.as_str()) && !has_same_vendor_read_preflight(step_index, step, workflow, registry) {
            warnings.push(serde_json::json!({
                "path": format!("$.steps[{step_index}]"),
                "rule": "provision_step_without_existence_check",
                "message": format!(
                    "step '{}' appears to provision resources ('{}') without an earlier same-vendor existence/read check",
                    step.id,
                    step.run
                ),
                "step_id": step.id,
                "run": step.run,
                "method": http_method,
                "next_step": "Add a same-vendor list/info check step before provisioning and branch with an if condition (reuse/delete/create).",
            }));
        }
    }

    warnings
}

fn build_read_before_index_map(workflow: &RuntimeWorkflow, registry: &CommandRegistry) -> Vec<bool> {
    let mut has_read_before = vec![false; workflow.steps.len()];
    let mut encountered_read_step = false;

    for (step_index, step) in workflow.steps.iter().enumerate() {
        has_read_before[step_index] = encountered_read_step;
        if is_read_step(step, registry) {
            encountered_read_step = true;
        }
    }

    has_read_before
}

fn is_read_step(step: &WorkflowStepDefinition, registry: &CommandRegistry) -> bool {
    let Some(command_specification) = command_spec_for_run(step.run.as_str(), registry) else {
        return false;
    };
    let Some(http_specification) = command_specification.http() else {
        return false;
    };
    http_specification.method.eq_ignore_ascii_case("GET")
}

fn has_same_vendor_read_preflight(
    current_step_index: usize,
    current_step: &WorkflowStepDefinition,
    workflow: &RuntimeWorkflow,
    registry: &CommandRegistry,
) -> bool {
    let Some((current_group, _)) = parse_provider_group_and_command(current_step.run.as_str()) else {
        return false;
    };

    workflow.steps.iter().take(current_step_index).any(|candidate| {
        if !is_read_step(candidate, registry) {
            return false;
        }
        let Some((candidate_group, _)) = parse_provider_group_and_command(candidate.run.as_str()) else {
            return false;
        };
        candidate_group == current_group
    })
}

fn command_spec_for_run<'a>(run_identifier: &str, registry: &'a CommandRegistry) -> Option<&'a CommandSpec> {
    let (group, command_name) = parse_provider_group_and_command(run_identifier)?;
    registry.find_by_group_and_cmd_ref(group.as_str(), command_name.as_str()).ok()
}

fn is_mutating_http_method(http_method: &str) -> bool {
    matches!(http_method, "POST" | "PUT" | "PATCH" | "DELETE")
}

fn is_create_operation(run_identifier: &str) -> bool {
    run_identifier
        .split_whitespace()
        .nth(1)
        .map(|command_name| command_name.ends_with(":create"))
        .unwrap_or(false)
}

/// Collects vendor-agnostic warnings for endpoint/context mismatches that
/// frequently pass schema validation but fail at runtime.
fn collect_endpoint_context_warnings(workflow: &RuntimeWorkflow) -> Vec<Value> {
    let mut warnings = Vec::new();
    let has_contextual_inputs = workflow.inputs.keys().any(|input_name| is_context_key(input_name.as_str()));

    for (step_index, step) in workflow.steps.iter().enumerate() {
        if !looks_like_single_resource_lookup(step.run.as_str()) {
            continue;
        }

        let has_identifier_binding = step.with.keys().any(|key| is_identifier_key(key.as_str()));
        if !has_identifier_binding {
            continue;
        }

        let has_context_binding_key = step.with.keys().any(|key| is_context_key(key.as_str()));
        let has_context_binding_expression = step
            .with
            .values()
            .any(|value| value.as_str().map(references_contextual_input).unwrap_or(false));

        if !has_contextual_inputs || has_context_binding_key || has_context_binding_expression {
            continue;
        }

        warnings.push(serde_json::json!({
            "path": format!("$.steps[{step_index}]"),
            "rule": "endpoint_context_mismatch",
            "message": format!(
                "step '{}' appears to fetch a single resource ('{}') using only identifier-like inputs while the workflow defines contextual inputs; scoped APIs often require project/org/workspace/team context",
                step.id,
                step.run
            ),
            "step_id": step.id,
            "run": step.run,
            "next_step": "Add contextual bindings (for example project/org/workspace/team/account/repo) or use a context-scoped command variant before continuing.",
        }));
    }

    warnings
}

fn looks_like_single_resource_lookup(run_identifier: &str) -> bool {
    run_identifier.contains(":info") || run_identifier.contains(":get")
}

fn is_identifier_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "id" || key == "idorname" || key.ends_with("id") || key.ends_with("_id")
}

fn is_context_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let context_tokens = [
        "project",
        "workspace",
        "team",
        "org",
        "organization",
        "owner",
        "account",
        "tenant",
        "subscription",
        "repo",
        "repository",
        "environment",
    ];
    context_tokens.iter().any(|token| key.contains(token))
}

fn references_contextual_input(raw_value: &str) -> bool {
    extract_template_expressions(raw_value).iter().any(|expression| {
        if !expression.starts_with("inputs.") {
            return false;
        }
        is_context_key(expression.trim_start_matches("inputs."))
    })
}

fn step_template_missing_path_next_step(
    referenced_step_identifier: &str,
    details: &oatty_engine::field_paths::SelectValueFieldMissingDetails,
) -> String {
    if details.nested_candidates.len() == 1 {
        return format!(
            "Update the template to use '${{{{ steps.{}.{} }}}}' and rerun workflow.validate.",
            referenced_step_identifier, details.nested_candidates[0]
        );
    }
    if !details.nested_candidates.is_empty() {
        return format!(
            "Update the template to an explicit output path from step '{}' (candidates: {}) and rerun workflow.validate.",
            referenced_step_identifier,
            details.nested_candidates.join(", ")
        );
    }
    if details.available_fields.is_empty() {
        return format!(
            "Update the template to a valid output path from step '{}' or use a command with a richer output schema, then rerun workflow.validate.",
            referenced_step_identifier
        );
    }
    format!(
        "Update the template to one of step '{}' output fields ({}) and rerun workflow.validate.",
        referenced_step_identifier,
        details.available_fields.join(", ")
    )
}

fn collect_step_template_references(step_index: usize, step: &WorkflowStepDefinition) -> Vec<StepTemplateReference> {
    let mut references = Vec::new();
    for (key, value) in &step.with {
        collect_step_template_references_from_value(value, format!("$.steps[{step_index}].with.{key}").as_str(), &mut references);
    }
    collect_step_template_references_from_value(&step.body, format!("$.steps[{step_index}].body").as_str(), &mut references);
    references
}

fn collect_step_template_references_from_value(value: &Value, location_path: &str, references: &mut Vec<StepTemplateReference>) {
    match value {
        Value::String(text) => {
            for expression in extract_template_expressions(text) {
                if let Some((referenced_step_id, field_path)) = parse_step_reference_expression(expression.as_str()) {
                    references.push(StepTemplateReference {
                        location_path: location_path.to_string(),
                        expression,
                        referenced_step_id,
                        field_path,
                    });
                }
            }
        }
        Value::Array(values) => {
            for (index, entry) in values.iter().enumerate() {
                collect_step_template_references_from_value(entry, format!("{location_path}[{index}]").as_str(), references);
            }
        }
        Value::Object(map) => {
            for (key, nested_value) in map {
                collect_step_template_references_from_value(nested_value, format!("{location_path}.{key}").as_str(), references);
            }
        }
        _ => {}
    }
}

fn output_schema_for_step<'a>(step: &WorkflowStepDefinition, registry: &'a CommandRegistry) -> Option<&'a SchemaProperty> {
    let (group, command_name) = parse_provider_group_and_command(step.run.as_str())?;
    let command_specification = registry.find_by_group_and_cmd_ref(group.as_str(), command_name.as_str()).ok()?;
    command_specification.http()?.output_schema.as_ref()
}

fn is_exact_template_expression(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("${{") && trimmed.ends_with("}}") && extract_template_expressions(trimmed).len() == 1
}

fn expected_input_type(command_specification: &oatty_types::CommandSpec, field_name: &str) -> Option<String> {
    if let Some(flag) = command_specification
        .flags
        .iter()
        .find(|flag| flag.name == field_name || flag.short_name.as_ref().map(|short_name| short_name == field_name).unwrap_or(false))
    {
        return Some(flag.r#type.to_ascii_lowercase());
    }

    None
}

fn output_schema_supports_reference(output_schema: Option<&SchemaProperty>, field_path: &str) -> bool {
    if let Some(schema) = output_schema
        && resolve_schema_path(schema, field_path).is_some()
    {
        return true;
    }
    false
}

fn output_contract_supports_reference(step: &WorkflowStepDefinition, field_path: &str) -> bool {
    let normalized_field_path = field_path.trim();
    if normalized_field_path.is_empty() || normalized_field_path.contains('.') {
        return false;
    }

    let Some(output_contract) = step.output_contract.as_ref() else {
        return false;
    };

    output_contract
        .fields
        .iter()
        .any(|field| field.name == normalized_field_path && !field_declares_non_scalar_type(field.r#type.as_deref()))
}

fn field_declares_non_scalar_type(declared_type: Option<&str>) -> bool {
    matches!(
        declared_type.map(|value| value.to_ascii_lowercase()),
        Some(value) if value == "object" || value == "array"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        SelectValueFieldValidation, collect_conditional_dependency_warnings, collect_endpoint_context_warnings,
        collect_mutation_preflight_warnings, collect_quoted_template_non_string_binding_warnings,
        collect_step_template_array_index_warnings, collect_step_template_output_path_violations, validate_select_value_field,
    };
    use indexmap::IndexMap;
    use oatty_engine::templates::parse_step_reference_expression;
    use oatty_registry::CommandRegistry;
    use oatty_registry::RegistryConfig;
    use oatty_types::command::HttpCommandSpec;
    use oatty_types::manifest::RegistryCatalog;
    use oatty_types::workflow::{RuntimeWorkflow, WorkflowStepDefinition};
    use oatty_types::{CommandSpec, SchemaProperty};
    use serde_json::Value;
    use std::collections::HashMap;

    fn schema(ty: &str) -> SchemaProperty {
        SchemaProperty {
            r#type: ty.to_string(),
            description: String::new(),
            properties: None,
            required: Vec::new(),
            items: None,
            enum_values: Vec::new(),
            format: None,
            tags: Vec::new(),
        }
    }

    fn object_schema(properties: Vec<(&str, SchemaProperty)>) -> SchemaProperty {
        let mut map = HashMap::new();
        for (name, property) in properties {
            map.insert(name.to_string(), Box::new(property));
        }
        let mut root = schema("object");
        root.properties = Some(map);
        root
    }

    #[test]
    fn value_field_validation_accepts_exact_scalar_path() {
        let schema = object_schema(vec![("owner", object_schema(vec![("id", schema("string"))]))]);
        let validation = validate_select_value_field(&schema, "owner.id");
        assert!(matches!(validation, SelectValueFieldValidation::Valid));
    }

    #[test]
    fn value_field_validation_reports_nested_candidate_for_missing_root_leaf() {
        let schema = object_schema(vec![
            ("owner", object_schema(vec![("id", schema("string"))])),
            ("name", schema("string")),
        ]);
        let validation = validate_select_value_field(&schema, "id");
        match validation {
            SelectValueFieldValidation::Missing { details } => {
                assert_eq!(details.nested_candidates, vec!["owner.id".to_string()])
            }
            _ => panic!("expected missing validation"),
        }
    }

    #[test]
    fn value_field_validation_reports_ambiguous_nested_candidates() {
        let schema = object_schema(vec![
            ("owner", object_schema(vec![("id", schema("string"))])),
            ("team", object_schema(vec![("id", schema("string"))])),
        ]);
        let validation = validate_select_value_field(&schema, "id");
        match validation {
            SelectValueFieldValidation::Missing { details } => {
                assert_eq!(details.nested_candidates, vec!["owner.id".to_string(), "team.id".to_string()])
            }
            _ => panic!("expected missing validation"),
        }
    }

    #[test]
    fn value_field_validation_rejects_non_scalar_path() {
        let schema = object_schema(vec![("owner", object_schema(vec![("id", schema("string"))]))]);
        let validation = validate_select_value_field(&schema, "owner");
        assert!(matches!(validation, SelectValueFieldValidation::NonScalar { .. }));
    }

    #[test]
    fn parse_step_reference_expression_extracts_step_and_path() {
        let parsed = parse_step_reference_expression("steps.trigger_initial_deploy.deploy.id").expect("step reference");
        assert_eq!(parsed.0, "trigger_initial_deploy");
        assert_eq!(parsed.1, "deploy.id");
    }

    #[test]
    fn parse_step_reference_expression_accepts_bracket_index_paths() {
        let parsed = parse_step_reference_expression("steps.find_render_service[0].service.id").expect("step reference");
        assert_eq!(parsed.0, "find_render_service");
        assert_eq!(parsed.1, "0.service.id");
    }

    #[test]
    fn step_template_validation_reports_missing_output_path() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "path_check".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "trigger_initial_deploy".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "wait_for_deploy".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec!["trigger_initial_deploy".to_string()],
                    with: IndexMap::from_iter([(
                        "deployId".to_string(),
                        Value::String("${{ steps.trigger_initial_deploy.id }}".to_string()),
                    )]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let violations = collect_step_template_output_path_violations(&workflow, &registry);
        let violation = violations
            .iter()
            .find(|candidate| candidate["rule"] == "step_template_output_path_missing")
            .expect("expected missing output path violation");

        assert_eq!(violation["path"], serde_json::json!("$.steps[1].with.deployId"));
        assert_eq!(violation["field_path"], serde_json::json!("id"));
        assert_eq!(violation["referenced_step_id"], serde_json::json!("trigger_initial_deploy"));
        let nested_candidates = violation["nested_candidates"]
            .as_array()
            .expect("nested candidates should be an array");
        assert!(
            nested_candidates
                .iter()
                .any(|candidate| candidate == &serde_json::json!("deploy.id")),
            "expected deploy.id in nested candidates, got {:?}",
            nested_candidates
        );
        let next_step = violation["next_step"].as_str().expect("next_step string");
        assert!(
            !next_step.contains("select.value_field"),
            "step template guidance should not mention provider select.value_field: {next_step}"
        );
    }

    #[test]
    fn step_template_validation_accepts_existing_output_path() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "path_ok".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "trigger_initial_deploy".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "wait_for_deploy".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec!["trigger_initial_deploy".to_string()],
                    with: IndexMap::from_iter([(
                        "deployId".to_string(),
                        Value::String("${{ steps.trigger_initial_deploy.deploy.id }}".to_string()),
                    )]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let violations = collect_step_template_output_path_violations(&workflow, &registry);
        assert!(
            violations
                .iter()
                .all(|violation| violation["rule"] != serde_json::json!("step_template_output_path_missing"))
        );
    }

    #[test]
    fn step_template_validation_rejects_output_contract_only_path_without_schema_path() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "contract_path_allowed".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "fetch_source_database_url".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: Some(oatty_types::workflow::WorkflowOutputContract {
                        fields: vec![oatty_types::workflow::WorkflowOutputField {
                            name: "value".to_string(),
                            tags: vec!["secret".to_string()],
                            description: Some("Database URL".to_string()),
                            r#type: Some("string".to_string()),
                        }],
                    }),
                },
                WorkflowStepDefinition {
                    id: "consume".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec!["fetch_source_database_url".to_string()],
                    with: IndexMap::from_iter([(
                        "database_url".to_string(),
                        Value::String("${{ steps.fetch_source_database_url.value }}".to_string()),
                    )]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let violations = collect_step_template_output_path_violations(&workflow, &registry);
        assert!(
            violations
                .iter()
                .all(|violation| violation["rule"] != serde_json::json!("step_template_output_path_missing")),
            "expected output_contract field mapping to satisfy template validation, got violations: {violations:?}"
        );
    }

    #[test]
    fn quoted_template_warning_flags_non_string_input_binding() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "quoted_non_string".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "source".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "target".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::from_iter([(
                        "includeMeta".to_string(),
                        Value::String("${{ steps.source.deploy.id }}".to_string()),
                    )]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let warnings = collect_quoted_template_non_string_binding_warnings(&workflow, &registry);
        let warning = warnings
            .iter()
            .find(|candidate| candidate["rule"] == "template_expression_quoted_non_string_input")
            .expect("expected quoted non-string warning");
        assert_eq!(warning["path"], serde_json::json!("$.steps[1].with.includeMeta"));
        assert_eq!(warning["expected_type"], serde_json::json!("boolean"));
    }

    #[test]
    fn conditional_dependency_warning_flags_skip_chain_risk() {
        let workflow = RuntimeWorkflow {
            identifier: "dependency_warning".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "find".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: Some("inputs.enabled == \"true\"".to_string()),
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "delete".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec!["find".to_string()],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let warnings = collect_conditional_dependency_warnings(&workflow);
        let warning = warnings
            .iter()
            .find(|candidate| candidate["rule"] == "depends_on_conditional_step")
            .expect("expected conditional dependency warning");
        assert_eq!(warning["step_id"], serde_json::json!("delete"));
        assert_eq!(warning["dependency_step_id"], serde_json::json!("find"));
    }

    #[test]
    fn step_template_validation_reports_missing_value_field_output_contract() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "value_field_missing_contract".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "find_render_service".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "delete_render_service".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec!["find_render_service".to_string()],
                    with: IndexMap::from_iter([(
                        "serviceId".to_string(),
                        Value::String("${{ steps.find_render_service.value }}".to_string()),
                    )]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let violations = collect_step_template_output_path_violations(&workflow, &registry);
        let violation = violations
            .iter()
            .find(|candidate| candidate["rule"] == "step_template_value_field_missing_output_contract")
            .expect("expected missing output_contract value violation");
        assert_eq!(violation["step_id"], serde_json::json!("delete_render_service"));
    }

    #[test]
    fn step_template_validation_reports_unverifiable_output_path_when_schema_missing() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "unverifiable_output".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "trigger_initial_deploy".to_string(),
                    run: "render services:deploys:create_no_schema".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "wait_for_deploy".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec!["trigger_initial_deploy".to_string()],
                    with: IndexMap::from_iter([(
                        "deployId".to_string(),
                        Value::String("${{ steps.trigger_initial_deploy.id }}".to_string()),
                    )]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let violations = collect_step_template_output_path_violations(&workflow, &registry);
        let violation = violations
            .iter()
            .find(|candidate| candidate["rule"] == "step_template_output_unverifiable")
            .expect("expected unverifiable output violation");
        assert_eq!(violation["step_id"], serde_json::json!("wait_for_deploy"));
        assert_eq!(violation["referenced_step_id"], serde_json::json!("trigger_initial_deploy"));
    }

    #[test]
    fn array_output_warning_flags_unindexed_step_reference_path() {
        let mut registry = build_workflow_step_validation_registry();
        let array_schema = SchemaProperty {
            r#type: "array".to_string(),
            description: String::new(),
            properties: None,
            required: Vec::new(),
            items: Some(Box::new(object_schema(vec![("id", schema("string"))]))),
            enum_values: Vec::new(),
            format: None,
            tags: Vec::new(),
        };
        registry
            .commands
            .iter_mut()
            .find(|command| command.group == "render" && command.name == "services:deploys:info")
            .and_then(|command| command.http_mut())
            .expect("http command")
            .output_schema = Some(array_schema);

        let workflow = RuntimeWorkflow {
            identifier: "array_index_warning".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "list_services".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "consume".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec!["list_services".to_string()],
                    with: IndexMap::from_iter([("serviceId".to_string(), Value::String("${{ steps.list_services.id }}".to_string()))]),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let warnings = collect_step_template_array_index_warnings(&workflow, &registry);
        let warning = warnings
            .iter()
            .find(|candidate| candidate["rule"] == "step_template_array_output_missing_index")
            .expect("expected array output index warning");
        assert_eq!(warning["step_id"], serde_json::json!("consume"));
        assert_eq!(warning["referenced_step_id"], serde_json::json!("list_services"));
    }

    #[test]
    fn mutation_preflight_warning_flags_mutation_without_prior_read_check() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "mutation_preflight_missing".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![WorkflowStepDefinition {
                id: "create_service".to_string(),
                run: "render services:deploys:create".to_string(),
                description: None,
                depends_on: vec![],
                with: IndexMap::new(),
                body: Value::Null,
                r#if: None,
                repeat: None,
                output_contract: None,
            }],
            final_output: None,
            requires: None,
        };

        let warnings = collect_mutation_preflight_warnings(&workflow, &registry);
        assert!(
            warnings
                .iter()
                .any(|warning| warning["rule"] == serde_json::json!("workflow_mutation_before_preflight"))
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning["rule"] == serde_json::json!("mutating_step_missing_guard"))
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning["rule"] == serde_json::json!("provision_step_without_existence_check"))
        );
    }

    #[test]
    fn mutation_preflight_warning_allows_create_after_same_vendor_read_check() {
        let registry = build_workflow_step_validation_registry();
        let workflow = RuntimeWorkflow {
            identifier: "mutation_preflight_ok".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![
                WorkflowStepDefinition {
                    id: "check_existing".to_string(),
                    run: "render services:deploys:info".to_string(),
                    description: None,
                    depends_on: vec![],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: None,
                    repeat: None,
                    output_contract: None,
                },
                WorkflowStepDefinition {
                    id: "create_service".to_string(),
                    run: "render services:deploys:create".to_string(),
                    description: None,
                    depends_on: vec!["check_existing".to_string()],
                    with: IndexMap::new(),
                    body: Value::Null,
                    r#if: Some("inputs.force_create == \"true\"".to_string()),
                    repeat: None,
                    output_contract: None,
                },
            ],
            final_output: None,
            requires: None,
        };

        let warnings = collect_mutation_preflight_warnings(&workflow, &registry);
        assert!(
            warnings
                .iter()
                .all(|warning| warning["rule"] != serde_json::json!("workflow_mutation_before_preflight"))
        );
        assert!(
            warnings
                .iter()
                .all(|warning| warning["rule"] != serde_json::json!("mutating_step_missing_guard"))
        );
        assert!(
            warnings
                .iter()
                .all(|warning| warning["rule"] != serde_json::json!("provision_step_without_existence_check"))
        );
    }

    #[test]
    fn endpoint_context_warning_flags_identifier_only_lookup_without_context_binding() {
        let workflow = RuntimeWorkflow {
            identifier: "context_warning".to_string(),
            title: None,
            description: None,
            inputs: IndexMap::from_iter([
                (
                    "project_id".to_string(),
                    oatty_types::workflow::WorkflowInputDefinition {
                        r#type: Some("string".to_string()),
                        ..Default::default()
                    },
                ),
                (
                    "vercel_database_env_id".to_string(),
                    oatty_types::workflow::WorkflowInputDefinition {
                        r#type: Some("string".to_string()),
                        ..Default::default()
                    },
                ),
            ]),
            steps: vec![WorkflowStepDefinition {
                id: "fetch_env".to_string(),
                run: "vendor env:info".to_string(),
                description: None,
                depends_on: vec![],
                with: IndexMap::from_iter([("id".to_string(), Value::String("${{ inputs.vercel_database_env_id }}".to_string()))]),
                body: Value::Null,
                r#if: None,
                repeat: None,
                output_contract: None,
            }],
            final_output: None,
            requires: None,
        };

        let warnings = collect_endpoint_context_warnings(&workflow);
        assert!(
            warnings
                .iter()
                .any(|warning| warning["rule"] == serde_json::json!("endpoint_context_mismatch"))
        );
    }

    fn build_workflow_step_validation_registry() -> CommandRegistry {
        let deploy_create_schema = object_schema(vec![
            (
                "deploy",
                object_schema(vec![("id", schema("string")), ("status", schema("string"))]),
            ),
            ("service", object_schema(vec![("id", schema("string"))])),
        ]);
        let deploy_info_schema = object_schema(vec![("status", schema("string"))]);

        let commands = vec![
            CommandSpec::new_http(
                "render".to_string(),
                "services:deploys:create".to_string(),
                "Create deploy".to_string(),
                Vec::new(),
                Vec::new(),
                HttpCommandSpec::new("POST", "/services/{id}/deploys", Some(deploy_create_schema), None),
                0,
            ),
            CommandSpec::new_http(
                "render".to_string(),
                "services:deploys:info".to_string(),
                "Deploy info".to_string(),
                Vec::new(),
                vec![oatty_types::CommandFlag {
                    name: "includeMeta".to_string(),
                    short_name: None,
                    required: false,
                    r#type: "boolean".to_string(),
                    enum_values: Vec::new(),
                    default_value: None,
                    description: Some("Include metadata".to_string()),
                    provider: None,
                }],
                HttpCommandSpec::new("GET", "/services/{id}/deploys/{deploy_id}", Some(deploy_info_schema), None),
                1,
            ),
            CommandSpec::new_http(
                "render".to_string(),
                "services:deploys:create_no_schema".to_string(),
                "Create deploy (no schema)".to_string(),
                Vec::new(),
                Vec::new(),
                HttpCommandSpec::new("POST", "/services/{id}/deploys", None, None),
                2,
            ),
        ];

        let catalog = RegistryCatalog {
            title: "Render API".to_string(),
            description: "Render".to_string(),
            vendor: Some("render".to_string()),
            manifest_path: String::new(),
            import_source: None,
            import_source_type: None,
            headers: indexmap::IndexSet::new(),
            base_urls: vec!["https://api.render.com".to_string()],
            base_url_index: 0,
            manifest: None,
            is_enabled: true,
        };

        let mut registry = CommandRegistry::default().with_commands(commands);
        registry.config = RegistryConfig {
            catalogs: Some(vec![catalog]),
        };
        registry
    }
}
