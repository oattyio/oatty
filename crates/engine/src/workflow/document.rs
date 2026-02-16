//! Runtime workflow catalog and conversion utilities.
//!
//! The registry embeds authored workflow definitions using the shared
//! [`oatty_types::workflow`] schema. The engine consumes those definitions at
//! runtime, normalizing identifiers and providing convenient lookups for the
//! execution pipeline. This module owns the lightweight conversion layer that
//! maps raw manifest entries into engine-friendly structures while preserving
//! authoring order.

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use oatty_types::{RuntimeWorkflow, WorkflowDefinition, WorkflowInputDefinition, WorkflowProviderArgumentValue};

/// Builds a runtime workflow from a manifest definition.
pub fn runtime_workflow_from_definition(definition: &WorkflowDefinition) -> Result<RuntimeWorkflow> {
    let identifier = definition.workflow.trim().to_string();

    if identifier.is_empty() {
        bail!("workflow definition is missing the required 'workflow' identifier");
    }

    let inputs = definition.inputs.clone();
    let steps = definition.steps.clone();

    if steps.is_empty() {
        bail!("workflow '{}' must declare at least one step", identifier);
    }

    validate_provider_dependency_bindings(&identifier, &inputs)?;

    Ok(RuntimeWorkflow {
        identifier,
        title: definition.title.clone(),
        description: definition.description.clone(),
        inputs,
        steps,
        final_output: definition.final_output.clone(),
        requires: definition.requires.clone(),
    })
}

/// Ensures provider-backed inputs declare explicit `depends_on` bindings when
/// provider arguments reference upstream inputs or step outputs.
fn validate_provider_dependency_bindings(workflow_identifier: &str, inputs: &IndexMap<String, WorkflowInputDefinition>) -> Result<()> {
    for (input_name, definition) in inputs {
        if definition.provider.is_none() {
            continue;
        }

        for (argument_name, argument_value) in &definition.provider_args {
            if !argument_value_references_upstream_value(argument_value) {
                continue;
            }

            let Some(dependency_value) = definition.depends_on.get(argument_name) else {
                bail!(
                    "workflow '{}' input '{}' provider argument '{}' references upstream data but is missing a matching depends_on binding",
                    workflow_identifier,
                    input_name,
                    argument_name
                );
            };

            if !argument_value_references_upstream_value(dependency_value) {
                bail!(
                    "workflow '{}' input '{}' depends_on.{} must reference an upstream input or step",
                    workflow_identifier,
                    input_name,
                    argument_name
                );
            }
        }
    }

    Ok(())
}

/// Returns true when an argument references upstream workflow context.
fn argument_value_references_upstream_value(argument_value: &WorkflowProviderArgumentValue) -> bool {
    match argument_value {
        WorkflowProviderArgumentValue::Binding(binding) => binding.from_input.is_some() || binding.from_step.is_some(),
        WorkflowProviderArgumentValue::Literal(template) => template.contains("${{ inputs.") || template.contains("${{ steps."),
    }
}

/// Builds an ordered catalogue of runtime workflows keyed by identifier.
pub fn build_runtime_catalog(definitions: &[WorkflowDefinition]) -> Result<IndexMap<String, RuntimeWorkflow>> {
    let mut catalog = IndexMap::new();

    for definition in definitions {
        let workflow = runtime_workflow_from_definition(definition).with_context(|| {
            let identifier = if definition.workflow.trim().is_empty() {
                "<missing>".to_string()
            } else {
                definition.workflow.clone()
            };
            format!("failed to normalise workflow '{identifier}'")
        })?;

        if catalog.contains_key(&workflow.identifier) {
            bail!("duplicate workflow identifier detected: '{}'", workflow.identifier);
        }

        catalog.insert(workflow.identifier.clone(), workflow);
    }

    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::{IndexMap, indexmap};
    use oatty_types::{WorkflowProviderArgumentBinding, WorkflowStepDefinition};

    #[test]
    fn rejects_missing_identifier() {
        let definition = WorkflowDefinition {
            workflow: String::new(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![WorkflowStepDefinition {
                id: "step".into(),
                run: "apps:list".into(),
                description: None,
                depends_on: Vec::new(),
                r#if: None,
                with: IndexMap::new(),
                body: serde_json::Value::Null,
                repeat: None,
                output_contract: None,
            }],
            final_output: None,
            requires: None,
        };

        let error = runtime_workflow_from_definition(&definition).expect_err("expected identifier error");
        assert!(error.to_string().contains("workflow definition is missing"));
    }

    #[test]
    fn rejects_workflows_without_steps() {
        let definition = WorkflowDefinition {
            workflow: "missing_steps".into(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: Vec::new(),
            final_output: None,
            requires: None,
        };

        let error = runtime_workflow_from_definition(&definition).expect_err("expected missing steps error");
        assert!(error.to_string().contains("must declare at least one step"));
    }

    #[test]
    fn rejects_provider_argument_referencing_upstream_without_depends_on() {
        let definition = WorkflowDefinition {
            workflow: "missing_depends_on".into(),
            title: None,
            description: None,
            inputs: indexmap! {
                "target".into() => WorkflowInputDefinition {
                    provider: Some(oatty_types::WorkflowValueProvider::Id("apps:list".into())),
                    provider_args: indexmap! {
                        "app".into() => WorkflowProviderArgumentValue::Binding(WorkflowProviderArgumentBinding {
                            from_input: Some("source".into()),
                            from_step: None,
                            path: None,
                            required: None,
                            on_missing: None,
                        })
                    },
                    ..WorkflowInputDefinition::default()
                }
            },
            steps: vec![WorkflowStepDefinition {
                id: "step".into(),
                run: "apps:list".into(),
                description: None,
                depends_on: Vec::new(),
                r#if: None,
                with: IndexMap::new(),
                body: serde_json::Value::Null,
                repeat: None,
                output_contract: None,
            }],
            final_output: None,
            requires: None,
        };

        let error = runtime_workflow_from_definition(&definition).expect_err("expected depends_on error");
        assert!(error.to_string().contains("missing a matching depends_on binding"));
    }

    #[test]
    fn accepts_provider_argument_referencing_upstream_with_matching_depends_on() {
        let definition = WorkflowDefinition {
            workflow: "with_depends_on".into(),
            title: None,
            description: None,
            inputs: indexmap! {
                "target".into() => WorkflowInputDefinition {
                    provider: Some(oatty_types::WorkflowValueProvider::Id("apps:list".into())),
                    provider_args: indexmap! {
                        "app".into() => WorkflowProviderArgumentValue::Binding(WorkflowProviderArgumentBinding {
                            from_input: Some("source".into()),
                            from_step: None,
                            path: None,
                            required: None,
                            on_missing: None,
                        })
                    },
                    depends_on: indexmap! {
                        "app".into() => WorkflowProviderArgumentValue::Binding(WorkflowProviderArgumentBinding {
                            from_input: Some("source".into()),
                            from_step: None,
                            path: None,
                            required: None,
                            on_missing: None,
                        })
                    },
                    ..WorkflowInputDefinition::default()
                }
            },
            steps: vec![WorkflowStepDefinition {
                id: "step".into(),
                run: "apps:list".into(),
                description: None,
                depends_on: Vec::new(),
                r#if: None,
                with: IndexMap::new(),
                body: serde_json::Value::Null,
                repeat: None,
                output_contract: None,
            }],
            final_output: None,
            requires: None,
        };

        let runtime = runtime_workflow_from_definition(&definition).expect("definition should be valid");
        assert_eq!(runtime.identifier, "with_depends_on");
    }
}
