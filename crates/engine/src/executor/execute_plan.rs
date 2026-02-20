//! Workflow plan execution helpers.

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::model::{StepSpec, WorkflowSpec};
use crate::resolve::{RunContext, eval_condition, find_unresolved_references_in_condition};

use super::{
    CommandRunner, NoopRunner, StepResult, StepStatus, collect_unresolved_step_templates, order_steps_for_execution, prepare_step,
    run_step_repeating_with, run_step_with,
};

/// Execute all steps sequentially, updating the context after each.
///
/// Each step's output is persisted under `ctx.steps[step.id]` after it runs.
pub fn execute_workflow(spec: &WorkflowSpec, run_context: &mut RunContext) -> Result<Vec<StepResult>> {
    let runner = NoopRunner;
    execute_workflow_with_runner(spec, run_context, &runner)
}

/// Execute all steps using a custom command runner.
///
/// Use this to run real commands via `RegistryCommandRunner` or a custom implementation.
pub fn execute_workflow_with_runner(
    spec: &WorkflowSpec,
    run_context: &mut RunContext,
    runner: &dyn CommandRunner,
) -> Result<Vec<StepResult>> {
    let ordered_steps = order_steps_for_execution(&spec.steps)?;
    info!(
        workflow = spec.workflow.as_deref().unwrap_or("unnamed"),
        workflow_name = spec.name.as_deref().unwrap_or("unnamed"),
        step_count = ordered_steps.len(),
        "workflow execution started"
    );

    let results = execute_plan_steps(ordered_steps, run_context, runner);
    let summary = summarize_step_statuses(&results);

    info!(
        workflow = spec.workflow.as_deref().unwrap_or("unnamed"),
        workflow_name = spec.name.as_deref().unwrap_or("unnamed"),
        step_count = results.len(),
        succeeded = summary.succeeded,
        failed = summary.failed,
        skipped = summary.skipped,
        "workflow execution finished"
    );

    Ok(results)
}

fn execute_plan_steps(steps: Vec<&StepSpec>, run_context: &mut RunContext, runner: &dyn CommandRunner) -> Vec<StepResult> {
    let mut results = Vec::with_capacity(steps.len());
    let mut statuses: HashMap<String, StepStatus> = HashMap::new();

    for step in &steps {
        debug!(step_id = %step.id, run = %step.run, "step execution started");

        if let Some(blocked_result) = dependency_block(step.id.as_str(), &step.depends_on, &statuses) {
            info!(step_id = %step.id, run = %step.run, "step execution skipped due to dependency");
            statuses.insert(step.id.clone(), blocked_result.status);
            results.push(blocked_result);
            continue;
        }

        if let Some(skipped_result) = condition_skip_result(step, run_context) {
            info!(step_id = %step.id, run = %step.run, "step execution skipped by condition");
            statuses.insert(step.id.clone(), skipped_result.status);
            results.push(skipped_result);
            continue;
        }

        let unresolved_templates = collect_unresolved_step_templates(step, run_context);
        if !unresolved_templates.is_empty() {
            let failure_result = unresolved_template_failure_result(step.id.as_str(), unresolved_templates);
            warn!(step_id = %step.id, "step execution failed due to unresolved templates");
            statuses.insert(step.id.clone(), failure_result.status);
            results.push(failure_result);
            continue;
        }

        let prepared_step = prepare_step(step, run_context);

        let result = if step.repeat.is_some() {
            run_step_repeating_with(&prepared_step, run_context, runner)
        } else {
            let single_result = run_step_with(&prepared_step, run_context, runner);
            run_context.steps.insert(step.id.clone(), single_result.output.clone());
            single_result
        };

        match result.status {
            StepStatus::Succeeded => debug!(step_id = %step.id, attempts = result.attempts, "step execution succeeded"),
            StepStatus::Failed => warn!(step_id = %step.id, attempts = result.attempts, "step execution failed"),
            StepStatus::Skipped => info!(step_id = %step.id, attempts = result.attempts, "step execution skipped"),
        }

        statuses.insert(step.id.clone(), result.status);
        results.push(result);
    }

    results
}

fn dependency_block(step_id: &str, dependencies: &[String], statuses: &HashMap<String, StepStatus>) -> Option<StepResult> {
    for dependency in dependencies {
        match statuses.get(dependency) {
            Some(StepStatus::Succeeded) => {}
            Some(StepStatus::Failed) => return Some(blocked_result(step_id, dependency, "failed earlier in the run")),
            Some(StepStatus::Skipped) => return Some(blocked_result(step_id, dependency, "did not execute successfully")),
            None => return Some(blocked_result(step_id, dependency, "has not executed yet")),
        }
    }
    None
}

fn condition_skip_result(step: &StepSpec, run_context: &RunContext) -> Option<StepResult> {
    let condition = step.r#if.as_ref()?;
    if eval_condition(condition, run_context) {
        return None;
    }

    let unresolved_references = find_unresolved_references_in_condition(condition, run_context);
    let logs = if unresolved_references.is_empty() {
        vec![format!("step '{}' skipped by condition", step.id)]
    } else {
        vec![format!(
            "step '{}' skipped by unresolved condition references: {}",
            step.id,
            unresolved_references.join(", ")
        )]
    };

    Some(StepResult {
        id: step.id.clone(),
        status: StepStatus::Skipped,
        output: Value::Null,
        logs,
        attempts: 0,
    })
}

fn blocked_result(step_id: &str, dependency: &str, detail: &str) -> StepResult {
    StepResult {
        id: step_id.to_string(),
        status: StepStatus::Skipped,
        output: Value::Null,
        logs: vec![format!("step '{}' skipped because dependency '{}' {}", step_id, dependency, detail)],
        attempts: 0,
    }
}

fn unresolved_template_failure_result(step_id: &str, unresolved_templates: Vec<crate::templates::UnresolvedTemplateRef>) -> StepResult {
    let mut logs = vec![format!(
        "step '{}' failed before execution: unresolved template references in with/body",
        step_id
    )];
    for unresolved_template in unresolved_templates {
        logs.push(format!(
            "unresolved template at {}: ${{{{ {} }}}}",
            unresolved_template.source_path, unresolved_template.expression
        ));
    }
    StepResult {
        id: step_id.to_string(),
        status: StepStatus::Failed,
        output: Value::Null,
        logs,
        attempts: 0,
    }
}

struct StepStatusSummary {
    succeeded: usize,
    failed: usize,
    skipped: usize,
}

fn summarize_step_statuses(results: &[StepResult]) -> StepStatusSummary {
    let mut summary = StepStatusSummary {
        succeeded: 0,
        failed: 0,
        skipped: 0,
    };

    for result in results {
        match result.status {
            StepStatus::Succeeded => summary.succeeded += 1,
            StepStatus::Failed => summary.failed += 1,
            StepStatus::Skipped => summary.skipped += 1,
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::{Value, json};

    use crate::{
        executor::{StepStatus, execute_workflow_with_runner, runner::CommandRunner},
        model::{StepSpec, WorkflowSpec},
        resolve::RunContext,
    };

    struct FailRunner;

    impl CommandRunner for FailRunner {
        fn run(&self, run: &str, _with: Option<&Value>, _body: Option<&Value>, _ctx: &RunContext) -> Result<Value> {
            if run == "fail" {
                bail!("boom");
            }
            Ok(json!({ "status": "ok" }))
        }
    }

    #[test]
    fn dependent_steps_skip_when_prerequisite_fails() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![
                StepSpec {
                    id: "first".into(),
                    depends_on: vec![],
                    run: "fail".into(),
                    ..Default::default()
                },
                StepSpec {
                    id: "second".into(),
                    depends_on: vec!["first".into()],
                    run: "echo".into(),
                    ..Default::default()
                },
            ],
        };

        let mut run_context = RunContext::default();
        let results = execute_workflow_with_runner(&spec, &mut run_context, &FailRunner).expect("execute");

        assert_eq!(results.len(), 2);
        assert!(matches!(results[0].status, StepStatus::Failed));
        assert!(matches!(results[1].status, StepStatus::Skipped));
        assert!(
            results[1].logs.iter().any(|log| log.contains("dependency 'first'")),
            "skip log missing dependency reason: {:?}",
            results[1].logs
        );
    }

    #[test]
    fn unresolved_templates_fail_step_before_runner_invocation() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![StepSpec {
                id: "delete".into(),
                depends_on: vec![],
                run: "apps:delete".into(),
                with: Some(json!({"id": "${{ steps.lookup.value }}"}).as_object().expect("object").clone()),
                ..Default::default()
            }],
        };

        let mut run_context = RunContext::default();
        let results = execute_workflow_with_runner(&spec, &mut run_context, &FailRunner).expect("execute");
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].status, StepStatus::Failed));
        assert!(results[0].logs.iter().any(|entry| entry.contains("unresolved template")));
        assert_eq!(results[0].attempts, 0);
    }

    #[test]
    fn unresolved_if_skips_before_unresolved_with_validation() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![StepSpec {
                id: "delete".into(),
                depends_on: vec![],
                run: "apps:delete".into(),
                r#if: Some("steps.find.value != null".into()),
                with: Some(json!({"id": "${{ steps.find.value }}"}).as_object().expect("object").clone()),
                ..Default::default()
            }],
        };

        let mut run_context = RunContext::default();
        run_context.steps.insert("find".into(), json!([]));
        let results = execute_workflow_with_runner(&spec, &mut run_context, &FailRunner).expect("execute");
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].status, StepStatus::Skipped));
        assert!(
            results[0]
                .logs
                .iter()
                .any(|entry| entry.contains("unresolved condition references"))
        );
        assert!(!results[0].logs.iter().any(|entry| entry.contains("unresolved template at with")));
    }
}
