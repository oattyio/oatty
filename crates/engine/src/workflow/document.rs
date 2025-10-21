//! Runtime workflow catalog and conversion utilities.
//!
//! The registry embeds authored workflow definitions using the shared
//! [`heroku_types::workflow`] schema. The engine consumes those definitions at
//! runtime, normalizing identifiers and providing convenient lookups for the
//! execution pipeline. This module owns the lightweight conversion layer that
//! maps raw manifest entries into engine-friendly structures while preserving
//! authoring order.

use anyhow::{Context, Result, bail};
use heroku_types::{RuntimeWorkflow, WorkflowDefinition};
use indexmap::IndexMap;

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
    use heroku_types::WorkflowStepDefinition;
    use indexmap::IndexMap;

    #[test]
    fn converts_definition_to_runtime_workflow() {
        let yaml_text = include_str!("../../../../workflows/create_app_and_db.yaml");
        let definition: WorkflowDefinition = serde_yaml::from_str(yaml_text).expect("parse workflow definition");

        let runtime = runtime_workflow_from_definition(&definition).expect("convert to runtime");

        assert_eq!(runtime.identifier, "app_with_db");
        assert_eq!(runtime.inputs.len(), definition.inputs.len());
        assert_eq!(runtime.steps.len(), definition.steps.len());
    }

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
    fn rejects_duplicate_identifiers() {
        let yaml_text = include_str!("../../../../workflows/create_app_and_db.yaml");
        let definition: WorkflowDefinition = serde_yaml::from_str(yaml_text).expect("parse workflow definition");

        let definitions = vec![definition.clone(), definition];
        let error = build_runtime_catalog(&definitions).expect_err("expected duplicate error");
        assert!(error.to_string().contains("duplicate workflow identifier"));
    }
}
