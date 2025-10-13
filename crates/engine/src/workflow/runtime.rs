//! Workflow runtime execution helpers.
//!
//! Utilities in this module convert a [`RuntimeWorkflow`](RuntimeWorkflow)
//! into the engine's internal [`WorkflowSpec`](WorkflowSpec) so the existing
//! executor pipeline can be reused without duplicating planning logic.

use std::collections::HashMap;

use crate::model::{ContractField, OutputContract, StepRepeat, StepSpec, WorkflowSpec};
use heroku_types::workflow::{RuntimeWorkflow, WorkflowOutputContract, WorkflowOutputField, WorkflowRepeat, WorkflowStepDefinition};
use indexmap::IndexMap;
use serde_json::{Map as JsonMap, Value};

/// Builds an engine-friendly `WorkflowSpec` from a runtime workflow definition.
pub fn workflow_spec_from_runtime(workflow: &RuntimeWorkflow) -> WorkflowSpec {
    WorkflowSpec {
        workflow: Some(workflow.identifier.clone()),
        inputs: HashMap::new(),
        steps: workflow.steps.iter().map(step_definition_to_spec).collect(),
    }
}

fn step_definition_to_spec(definition: &WorkflowStepDefinition) -> StepSpec {
    StepSpec {
        id: definition.id.clone(),
        run: definition.run.clone(),
        with: convert_with_map(&definition.with),
        body: match definition.body.clone() {
            Value::Null => None,
            other => Some(other),
        },
        repeat: definition.repeat.as_ref().and_then(convert_repeat),
        r#if: None,
        output_contract: definition.output_contract.as_ref().map(convert_output_contract),
    }
}

fn convert_with_map(values: &IndexMap<String, Value>) -> Option<JsonMap<String, Value>> {
    if values.is_empty() {
        return None;
    }
    let map: JsonMap<String, Value> = values.iter().map(|(key, value)| (key.clone(), value.clone())).collect();
    Some(map)
}

fn convert_repeat(repeat: &WorkflowRepeat) -> Option<StepRepeat> {
    let until = repeat.until.clone().unwrap_or_default();
    let every = repeat.every.clone().unwrap_or_else(|| "1s".to_string());

    if until.is_empty() {
        return None;
    }

    Some(StepRepeat {
        until,
        every,
        timeout: repeat.timeout.clone(),
        max_attempts: repeat.max_attempts,
    })
}

fn convert_output_contract(contract: &WorkflowOutputContract) -> OutputContract {
    OutputContract {
        fields: contract.fields.iter().map(convert_output_field).collect(),
    }
}

fn convert_output_field(field: &WorkflowOutputField) -> ContractField {
    ContractField {
        name: field.name.clone(),
        r#type: field.r#type.clone(),
        tags: field.tags.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heroku_types::workflow::{WorkflowOutputContract, WorkflowOutputField, WorkflowRepeat};

    fn sample_runtime_workflow() -> RuntimeWorkflow {
        let mut with = IndexMap::new();
        with.insert("app".into(), Value::String("${{ inputs.app }}".into()));
        let steps = vec![WorkflowStepDefinition {
            id: "s1".into(),
            run: "apps list".into(),
            description: None,
            depends_on: Vec::new(),
            with,
            body: Value::Null,
            repeat: Some(WorkflowRepeat {
                until: Some("steps.s1.status == \"succeeded\"".into()),
                every: Some("5s".into()),
                timeout: None,
                max_attempts: None,
            }),
            output_contract: Some(WorkflowOutputContract {
                fields: vec![WorkflowOutputField {
                    name: "id".into(),
                    tags: vec!["app_id".into()],
                    description: None,
                    r#type: Some("string".into()),
                }],
            }),
        }];

        RuntimeWorkflow {
            identifier: "demo".into(),
            title: Some("Demo".into()),
            description: Some("Workflow demo".into()),
            inputs: IndexMap::new(),
            steps,
        }
    }

    #[test]
    fn converts_runtime_workflow_to_spec() {
        let runtime = sample_runtime_workflow();
        let spec = workflow_spec_from_runtime(&runtime);

        assert_eq!(spec.workflow.as_deref(), Some("demo"));
        assert_eq!(spec.steps.len(), 1);

        let step = &spec.steps[0];
        assert_eq!(step.id, "s1");
        assert!(step.with.as_ref().unwrap().contains_key("app"));
        assert!(step.repeat.is_some());
        assert!(step.output_contract.is_some());
    }
}
