//! Step preparation helpers.

use crate::{
    model::StepSpec,
    resolve::{RunContext, interpolate_value},
    templates::{UnresolvedTemplateRef, collect_unresolved_templates_from_value},
};

use super::PreparedStep;
use serde_json::Value;

/// Prepare a step from a workflow spec by interpolating inputs/env into step parameters.
///
/// This function call must be performed as late as possible to
/// resolve references to prior `steps.<id>` bindings.
pub fn prepare_step(step: &StepSpec, run_context: &RunContext) -> PreparedStep {
    PreparedStep {
        id: step.id.clone(),
        depends_on: step.depends_on.clone(),
        run: step.run.clone(),
        with: step.with.as_ref().map(|map| {
            let value = Value::Object(map.clone());
            match interpolate_value(&value, run_context) {
                Value::Object(object) => object,
                _ => map.clone(),
            }
        }),
        body: step.body.as_ref().map(|value| interpolate_value(value, run_context)),
        r#if: step.r#if.clone(),
        repeat: step.repeat.clone(),
    }
}

/// Reports unresolved template expressions found in a step's `with` and `body`.
///
/// Returned entries include the source field path and original expression.
pub fn collect_unresolved_step_templates(step: &StepSpec, run_context: &RunContext) -> Vec<UnresolvedTemplateRef> {
    let mut unresolved = Vec::new();

    if let Some(with_values) = &step.with {
        for (field_name, field_value) in with_values {
            collect_unresolved_templates_from_value(field_value, format!("with.{field_name}").as_str(), run_context, &mut unresolved);
        }
    }

    if let Some(body) = &step.body {
        collect_unresolved_templates_from_value(body, "body", run_context, &mut unresolved);
    }

    unresolved
}
#[cfg(test)]
mod tests {
    use super::{collect_unresolved_step_templates, prepare_step};
    use crate::{executor::order_steps_for_execution, model::WorkflowSpec, resolve::RunContext};
    use serde_json::json;

    #[test]
    fn prepare_plan_interpolates_inputs() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![crate::model::StepSpec {
                id: "s1".into(),
                depends_on: vec![],
                run: "echo".into(),
                with: Some(json!({"name": "${{ inputs.app }}"}).as_object().expect("object").clone()),
                body: None,
                repeat: None,
                r#if: None,
                output_contract: None,
            }],
        };

        let mut run_context = RunContext::default();
        run_context.inputs.insert("app".into(), json!("myapp"));

        let steps = order_steps_for_execution(&spec.steps).expect("plan");
        let prepared_step = prepare_step(steps[0], &run_context);

        assert_eq!(prepared_step.with.as_ref().expect("with map")["name"], "myapp");
    }

    #[test]
    fn unresolved_templates_are_reported_for_with_and_body() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![crate::model::StepSpec {
                id: "s1".into(),
                depends_on: vec![],
                run: "echo".into(),
                with: Some(json!({"serviceId": "${{ steps.find.value }}"}).as_object().expect("object").clone()),
                body: Some(json!({"id": "${{ steps.find.value }}"})),
                repeat: None,
                r#if: None,
                output_contract: None,
            }],
        };
        let run_context = RunContext::default();
        let unresolved = collect_unresolved_step_templates(&spec.steps[0], &run_context);
        assert_eq!(unresolved.len(), 2);
        assert_eq!(unresolved[0].source_path, "with.serviceId");
        assert_eq!(unresolved[0].expression, "steps.find.value");
    }
}
