//! Workflow runtime execution helpers.
//!
//! Utilities in this module convert a [`RuntimeWorkflow`](RuntimeWorkflow)
//! into the engine's internal [`WorkflowSpec`](WorkflowSpec) so the existing
//! executor pipeline can be reused without duplicating planning logic.

use std::collections::HashMap;

use crate::model::{ContractField, OutputContract, StepRepeat, StepSpec, WorkflowSpec};
use oatty_types::workflow::{RuntimeWorkflow, WorkflowOutputContract, WorkflowOutputField, WorkflowRepeat, WorkflowStepDefinition};
use indexmap::IndexMap;
use serde_json::{Map as JsonMap, Value};

/// Builds an engine-friendly `WorkflowSpec` from a runtime workflow definition.
pub fn workflow_spec_from_runtime(workflow: &RuntimeWorkflow) -> WorkflowSpec {
    WorkflowSpec {
        workflow: Some(workflow.identifier.clone()),
        name: workflow.title.clone(),
        inputs: HashMap::new(),
        steps: workflow.steps.iter().map(step_definition_to_spec).collect(),
    }
}

fn step_definition_to_spec(definition: &WorkflowStepDefinition) -> StepSpec {
    StepSpec {
        id: definition.id.clone(),
        depends_on: definition.depends_on.clone(),
        run: definition.run.clone(),
        with: convert_with_map(&definition.with),
        body: match definition.body.clone() {
            Value::Null => None,
            other => Some(other),
        },
        repeat: definition.repeat.as_ref().and_then(convert_repeat),
        r#if: normalize_condition_expression(definition.r#if.as_deref()),
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
    let until = normalize_condition_expression(repeat.until.as_deref()).unwrap_or_default();
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

fn normalize_condition_expression(raw: Option<&str>) -> Option<String> {
    let text = raw?.trim();
    if text.is_empty() {
        return None;
    }

    if let Some(stripped) = text.strip_prefix("${{") {
        let inner = stripped.trim();
        let inner = inner.strip_suffix("}}").unwrap_or(inner);
        let inner = inner.trim();
        if inner.is_empty() { None } else { Some(inner.to_string()) }
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oatty_types::workflow::{WorkflowOutputContract, WorkflowOutputField, WorkflowRepeat};

    fn sample_runtime_workflow() -> RuntimeWorkflow {
        let mut with = IndexMap::new();
        with.insert("app".into(), Value::String("${{ inputs.app }}".into()));
        let steps = vec![WorkflowStepDefinition {
            id: "s1".into(),
            run: "apps list".into(),
            description: None,
            depends_on: Vec::new(),
            r#if: Some("${{ inputs.enabled }}".into()),
            with,
            body: Value::Null,
            repeat: Some(WorkflowRepeat {
                until: Some("steps.s1.status == \\\"succeeded\\\"".into()),
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
        assert_eq!(step.r#if.as_deref(), Some("inputs.enabled"));
        assert!(step.repeat.is_some());
        assert!(step.output_contract.is_some());
    }

    #[test]
    fn repeat_until_allows_template_wrapper() {
        let mut runtime = sample_runtime_workflow();
        runtime.steps[0].repeat = Some(WorkflowRepeat {
            until: Some("${{ steps.s1.status == \"succeeded\" }}".into()),
            every: Some("5s".into()),
            timeout: Some("1m".into()),
            max_attempts: Some(5),
        });

        let spec = workflow_spec_from_runtime(&runtime);
        let repeat = spec.steps[0].repeat.as_ref().expect("repeat missing");
        assert_eq!(repeat.until, "steps.s1.status == \"succeeded\"");
        assert_eq!(repeat.every, "5s");
        assert_eq!(repeat.timeout.as_deref(), Some("1m"));
        assert_eq!(repeat.max_attempts, Some(5));
    }

    #[test]
    fn repeat_without_until_is_ignored() {
        let mut runtime = sample_runtime_workflow();
        runtime.steps[0].repeat = Some(WorkflowRepeat {
            until: None,
            every: Some("5s".into()),
            timeout: Some("1m".into()),
            max_attempts: Some(3),
        });

        let spec = workflow_spec_from_runtime(&runtime);
        assert!(spec.steps[0].repeat.is_none());
    }

    #[test]
    fn step_body_and_flags_are_preserved() {
        let mut with = IndexMap::new();
        with.insert("app".into(), Value::String("${{ inputs.app }}".into()));
        let step = WorkflowStepDefinition {
            id: "body_step".into(),
            run: "apps:create".into(),
            description: None,
            depends_on: vec!["s1".into()],
            r#if: Some("${{ inputs.flag }}".into()),
            with,
            body: serde_json::json!({
                "name": "${{ inputs.app }}",
                "region": "us"
            }),
            repeat: None,
            output_contract: None,
        };
        let runtime = RuntimeWorkflow {
            identifier: "demo".into(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![step],
        };

        let spec = workflow_spec_from_runtime(&runtime);
        let spec_step = &spec.steps[0];
        assert_eq!(spec_step.run, "apps:create");
        assert_eq!(spec_step.depends_on, vec!["s1"]);

        let flags = spec_step.with.as_ref().expect("with map missing");
        assert_eq!(flags.get("app"), Some(&Value::String("${{ inputs.app }}".into())));

        let body = spec_step.body.as_ref().expect("body missing");
        assert_eq!(body["region"], "us");
        assert_eq!(body["name"], "${{ inputs.app }}");
    }

    #[test]
    fn normalizes_condition_expressions() {
        let step = WorkflowStepDefinition {
            id: "conditional".into(),
            run: "apps:list".into(),
            description: None,
            depends_on: Vec::new(),
            r#if: Some("  ${{  inputs.flag  }}  ".into()),
            with: IndexMap::new(),
            body: Value::Null,
            repeat: None,
            output_contract: None,
        };
        let runtime = RuntimeWorkflow {
            identifier: "conditional_test".into(),
            title: None,
            description: None,
            inputs: IndexMap::new(),
            steps: vec![step],
        };

        let spec = workflow_spec_from_runtime(&runtime);
        assert_eq!(spec.steps[0].r#if.as_deref(), Some("inputs.flag"));
    }
}
