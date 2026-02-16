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
use oatty_registry::CommandRegistry;
use oatty_types::workflow::{RuntimeWorkflow, WorkflowInputDefinition, WorkflowValueProvider, collect_missing_catalog_requirements};
use oatty_types::{CommandSpec, SchemaProperty};
use rmcp::model::ErrorData;
use serde_json::Value;
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

    Ok(all_violations)
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

#[cfg(test)]
mod tests {
    use super::{SelectValueFieldValidation, validate_select_value_field};
    use oatty_types::SchemaProperty;
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
}
