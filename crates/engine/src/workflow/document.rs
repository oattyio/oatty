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
use oatty_types::{RuntimeWorkflow, WorkflowDefinition};

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

    Ok(RuntimeWorkflow {
        identifier,
        title: definition.title.clone(),
        description: definition.description.clone(),
        inputs,
        steps,
    })
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
    use indexmap::IndexMap;
    use oatty_types::WorkflowStepDefinition;

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
        };

        let error = runtime_workflow_from_definition(&definition).expect_err("expected missing steps error");
        assert!(error.to_string().contains("must declare at least one step"));
    }
}
