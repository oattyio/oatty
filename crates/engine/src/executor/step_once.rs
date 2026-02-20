//! Single-step execution helpers.

use serde_json::Value;

use crate::resolve::{RunContext, eval_condition, find_unresolved_references_in_condition};

use super::{CommandRunner, PreparedStep, StepResult, StepStatus};

/// Execute a prepared step once using the provided runner.
///
/// Returns a `StepResult` with `attempts = 1` on success or failure, or `Skipped`
/// if the step's condition evaluates to false.
pub fn run_step_with(step: &PreparedStep, run_context: &RunContext, runner: &dyn CommandRunner) -> StepResult {
    let mut result = StepResult {
        id: step.id.clone(),
        ..Default::default()
    };

    if let Some(condition) = &step.r#if
        && !eval_condition(condition, run_context)
    {
        let unresolved_references = find_unresolved_references_in_condition(condition, run_context);
        result.status = StepStatus::Skipped;
        if unresolved_references.is_empty() {
            result.logs.push(format!("step '{}' skipped by condition", step.id));
        } else {
            result.logs.push(format!(
                "step '{}' skipped by unresolved condition references: {}",
                step.id,
                unresolved_references.join(", ")
            ));
        }
        return result;
    }

    let with_value = step.with.as_ref().map(|map| Value::Object(map.clone()));
    match runner.run(&step.run, with_value.as_ref(), step.body.as_ref(), run_context) {
        Ok(output) => {
            result.status = StepStatus::Succeeded;
            result.output = output;
            result.logs.push(format!("step '{}' executed", step.id));
            result.attempts = 1;
        }
        Err(error) => {
            result.status = StepStatus::Failed;
            result.logs.push(format!("step '{}' failed: {}", step.id, error));
            result.attempts = 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::run_step_with;
    use crate::{executor::PreparedStep, executor::runner::CommandRunner, resolve::RunContext};
    use anyhow::Result;
    use serde_json::{Value, json};

    struct EchoRunner;

    impl CommandRunner for EchoRunner {
        fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, _ctx: &RunContext) -> Result<Value> {
            Ok(json!({
                "cmd": run,
                "with": with.cloned().unwrap_or(Value::Null),
                "body": body.cloned().unwrap_or(Value::Null),
                "status": "ok"
            }))
        }
    }

    #[test]
    fn run_step_persists_output_and_respects_condition() {
        let step = PreparedStep {
            id: "s1".into(),
            depends_on: vec![],
            run: "do".into(),
            with: None,
            body: None,
            r#if: Some("inputs.enabled == \"true\"".into()),
            repeat: None,
        };
        let runner = EchoRunner;

        let mut run_context = RunContext::default();
        run_context.inputs.insert("enabled".into(), json!("true"));
        let result = run_step_with(&step, &run_context, &runner);
        assert_eq!(result.status, crate::executor::StepStatus::Succeeded);

        let mut skipped_context = RunContext::default();
        skipped_context.inputs.insert("enabled".into(), json!("false"));
        let skipped_result = run_step_with(&step, &skipped_context, &runner);
        assert_eq!(skipped_result.status, crate::executor::StepStatus::Skipped);
    }

    #[test]
    fn run_step_skips_when_optional_input_missing() {
        let step = PreparedStep {
            id: "optional".into(),
            depends_on: vec![],
            run: "noop".into(),
            with: None,
            body: None,
            r#if: Some("inputs.optional_field".into()),
            repeat: None,
        };
        let runner = EchoRunner;
        let run_context = RunContext::default();

        let result = run_step_with(&step, &run_context, &runner);
        assert_eq!(result.status, crate::executor::StepStatus::Skipped);
        assert!(result.logs.iter().any(|line| line.contains("skipped")));
    }

    #[test]
    fn run_step_skips_when_condition_has_unresolved_reference() {
        let step = PreparedStep {
            id: "conditional".into(),
            depends_on: vec![],
            run: "noop".into(),
            with: None,
            body: None,
            r#if: Some("steps.lookup.value != null".into()),
            repeat: None,
        };
        let runner = EchoRunner;
        let mut run_context = RunContext::default();
        run_context.steps.insert("lookup".into(), json!([]));

        let result = run_step_with(&step, &run_context, &runner);
        assert_eq!(result.status, crate::executor::StepStatus::Skipped);
        assert!(result.logs.iter().any(|line| line.contains("unresolved condition references")));
    }

    #[test]
    fn run_step_executes_when_missing_input_is_compared_to_null() {
        let step = PreparedStep {
            id: "null_guard".into(),
            depends_on: vec![],
            run: "noop".into(),
            with: None,
            body: None,
            r#if: Some("inputs.optional_field == null".into()),
            repeat: None,
        };
        let runner = EchoRunner;
        let run_context = RunContext::default();

        let result = run_step_with(&step, &run_context, &runner);
        assert_eq!(result.status, crate::executor::StepStatus::Succeeded);
    }
}
