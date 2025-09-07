//! Execution engine: builds a plan from a workflow, executes steps (with optional
//! repeat/until semantics), and persists results back into the runtime context.
//!
//! - Plan preparation interpolates inputs/environment into step parameters
//! - `runner::CommandRunner` abstracts how a command is executed
//! - `runner::RegistryCommandRunner` issues HTTP requests using the command registry
//! - Helpers run steps sequentially and update `RunContext.steps` as they go

use std::{thread, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    model::{StepRepeat, StepSpec, WorkflowSpec},
    resolve::{RunContext, eval_condition, interpolate_value},
};

pub mod runner;
pub use runner::{CommandRunner, NoopRunner, RegistryCommandRunner};

/// Max attempts for repeat/until steps to prevent infinite loops.
const MAX_REPEAT_ATTEMPTS: u32 = 100;
/// Default polling interval when repeat `every` is invalid or missing.
const DEFAULT_REPEAT_INTERVAL: Duration = Duration::from_secs(1);

/// Prepared step with inputs/body interpolated against the provided context.
///
/// This is the unit executed by the engine. Each `PreparedStep` is derived from a
/// `StepSpec` by applying string interpolation based on a `RunContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedStep {
    /// Unique identifier for this step within a workflow.
    pub id: String,
    /// Command identifier, e.g. "apps:create" or registry-backed "addons:attach".
    pub run: String,
    /// Named input arguments passed to the command as query/body or positional path parts.
    #[serde(default)]
    pub with: Option<serde_json::Map<String, Value>>,
    /// Optional JSON body provided to the command.
    #[serde(default)]
    pub body: Option<Value>,
    /// Optional conditional expression; when false the step is skipped.
    #[serde(default, rename = "if")]
    pub r#if: Option<String>,
    /// Optional repeat specification to poll until a condition is met.
    #[serde(default)]
    pub repeat: Option<StepRepeat>,
}

/// A plan is an ordered list of prepared steps.
///
/// The plan is created by `prepare_plan` and executed by the workflow helpers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Plan {
    /// The ordered steps to run.
    pub steps: Vec<PreparedStep>,
}

/// Status of an executed step.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    /// Step did not run due to failing condition.
    Skipped,
    /// Step executed and returned successfully.
    Succeeded,
    /// Step attempted but returned an error.
    Failed,
}

/// Result of running a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step identifier.
    pub id: String,
    /// Final status of this step execution.
    pub status: StepStatus,
    /// Arbitrary JSON returned by the runner.
    pub output: Value,
    /// Log lines captured while running the step.
    pub logs: Vec<String>,
    /// Number of attempts when `repeat` is used (>= 1 if executed).
    pub attempts: u32,
}

impl Default for StepResult {
    fn default() -> Self {
        Self {
            id: String::new(),
            status: StepStatus::Skipped,
            output: Value::Null,
            logs: vec![],
            attempts: 0,
        }
    }
}

/// Prepare a plan from a workflow spec by interpolating inputs/env into step parameters.
///
/// References to prior `steps.<id>` are not resolved during planning; they are
/// evaluated at execution time when the step is actually run.
///
/// Example:
/// ```rust
/// use heroku_engine::resolve::RunContext;
/// use heroku_engine::model::{WorkflowSpec, StepSpec};
/// use serde_json::json;
///
/// let spec = WorkflowSpec {
///     workflow: Some("demo".into()),
///     inputs: Default::default(),
///     steps: vec![StepSpec {
///         id: "s1".into(),
///         run: "echo".into(),
///         with: Some(json!({"name": "${{ inputs.app }}"}).as_object().unwrap().clone()),
///         body: None,
///         repeat: None,
///         r#if: None,
///         output_contract: None,
///     }],
/// };
/// let mut ctx = RunContext::default();
/// ctx.inputs.insert("app".into(), json!("myapp"));
/// let plan = heroku_engine::executor::prepare_plan(&spec, &ctx);
/// assert_eq!(plan.steps[0].with.as_ref().unwrap()["name"], "myapp");
/// ```
pub fn prepare_plan(spec: &WorkflowSpec, run_context: &RunContext) -> Plan {
    let steps = spec
        .steps
        .iter()
        .map(|s: &StepSpec| PreparedStep {
            id: s.id.clone(),
            run: s.run.clone(),
            with: s.with.as_ref().map(|m| {
                // Interpolate by wrapping in a JSON object, then unwrap back to a map
                let v = Value::Object(m.clone());
                match interpolate_value(&v, run_context) {
                    Value::Object(obj) => obj,
                    _ => m.clone(),
                }
            }),
            body: s.body.as_ref().map(|v| interpolate_value(v, run_context)),
            r#if: s.r#if.clone(),
            repeat: s.repeat.clone(),
        })
        .collect();

    Plan { steps }
}

/// Execute a prepared step once using the provided runner.
///
/// Returns a `StepResult` with `attempts = 1` on success or failure, or `Skipped`
/// if the step's condition evaluates to false.
pub fn run_step_with(step: &PreparedStep, ctx: &RunContext, runner: &dyn CommandRunner) -> StepResult {
    let mut result = StepResult {
        id: step.id.clone(),
        ..Default::default()
    };

    // Evaluate conditional
    if let Some(cond) = &step.r#if
        && !eval_condition(cond, ctx)
    {
        result.status = StepStatus::Skipped;
        result.logs.push(format!("step '{}' skipped by condition", step.id));
        return result;
    }

    // Execute
    let with_v = step.with.as_ref().map(|m| Value::Object(m.clone()));
    match runner.run(&step.run, with_v.as_ref(), step.body.as_ref(), ctx) {
        Ok(output) => {
            result.status = StepStatus::Succeeded;
            result.output = output;
            result.logs.push(format!("step '{}' executed", step.id));
            result.attempts = 1;
        }
        Err(err) => {
            result.status = StepStatus::Failed;
            result.logs.push(format!("step '{}' failed: {}", step.id, err));
            result.attempts = 1;
        }
    }

    result
}

/// Execute a prepared step with repeat/until semantics. Persists the latest output
/// into `ctx.steps[step.id]` after each attempt using the provided context mutably.
///
/// Guard rails:
/// - Max attempts constrained by `MAX_REPEAT_ATTEMPTS` to prevent infinite loops.
/// - `every` string supports "Xs" seconds or "Xm" minutes; defaults to 1s when invalid.
///
/// On success, returns the last `StepResult` that satisfied the `until` condition with
/// an accurate `attempts` count. If the guard trips, returns `Failed` with logs.
pub fn run_step_repeating_with(step: &PreparedStep, ctx: &mut RunContext, runner: &dyn CommandRunner) -> StepResult {
    // If condition fails up-front, skip without attempts
    if let Some(cond) = &step.r#if
        && !eval_condition(cond, ctx)
    {
        let mut skipped = StepResult {
            id: step.id.clone(),
            ..Default::default()
        };
        skipped.status = StepStatus::Skipped;
        skipped.logs.push(format!("step '{}' skipped by condition", step.id));
        return skipped;
    }

    let max_attempts = MAX_REPEAT_ATTEMPTS;
    let sleep_dur = step
        .repeat
        .as_ref()
        .and_then(|r| parse_every(&r.every))
        .unwrap_or(DEFAULT_REPEAT_INTERVAL);
    let until_expr = step.repeat.as_ref().map(|r| r.until.clone());

    let mut attempts = 0u32;
    let result: StepResult = loop {
        attempts += 1;
        let single = run_step_with(step, ctx, runner);
        // Persist output into context
        ctx.steps.insert(step.id.clone(), single.output.clone());

        if until_expr.as_deref().map(|e| eval_condition(e, ctx)).unwrap_or(true) {
            let mut sr = single;
            sr.attempts = attempts;
            break sr;
        }

        if attempts >= max_attempts {
            let mut sr = single;
            sr.status = StepStatus::Failed;
            sr.logs
                .push(format!("repeat guard tripped at {} attempts; stopping", attempts));
            sr.attempts = attempts;
            break sr;
        }

        thread::sleep(sleep_dur);
    };

    result
}

/// Convenience wrapper using the default `NoopRunner`.
///
/// This is useful for previews and for unit tests that do not need to hit the
/// real registry or network.
pub fn run_step(step: &PreparedStep, ctx: &mut RunContext) -> StepResult {
    let runner = NoopRunner;
    if step.repeat.is_some() {
        run_step_repeating_with(step, ctx, &runner)
    } else {
        let res = run_step_with(step, ctx, &runner);
        // Persist once for consistency
        ctx.steps.insert(step.id.clone(), res.output.clone());
        res
    }
}

/// Execute all steps sequentially, updating the context after each.
///
/// Each step's output is persisted under `ctx.steps[step.id]` after it runs.
pub fn execute_workflow(spec: &WorkflowSpec, ctx: &mut RunContext) -> Vec<StepResult> {
    let plan = prepare_plan(spec, ctx);
    let mut results = Vec::with_capacity(plan.steps.len());
    for step in plan.steps.iter() {
        let res = run_step(step, ctx);
        results.push(res);
    }
    results
}

/// Execute all steps using a custom command runner.
///
/// Use this to run real commands via `RegistryCommandRunner` or a custom implementation.
pub fn execute_workflow_with_runner(
    spec: &WorkflowSpec,
    ctx: &mut RunContext,
    runner: &dyn CommandRunner,
) -> Vec<StepResult> {
    let plan = prepare_plan(spec, ctx);
    let mut results = Vec::with_capacity(plan.steps.len());
    for step in plan.steps.iter() {
        let res = if step.repeat.is_some() {
            run_step_repeating_with(step, ctx, runner)
        } else {
            let single = run_step_with(step, ctx, runner);
            ctx.steps.insert(step.id.clone(), single.output.clone());
            single
        };
        results.push(res);
    }
    results
}

fn parse_every(s: &str) -> Option<Duration> {
    // Accept formats like "10s" or "2m". If only a number is given, treat as seconds.
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let last = trimmed.chars().last()?;
    if last.is_ascii_alphabetic() {
        let num = &trimmed[..trimmed.len() - 1];
        let n: u64 = num.parse().ok()?;
        return match last {
            's' | 'S' => Some(Duration::from_secs(n)),
            'm' | 'M' => Some(Duration::from_secs(n * 60)),
            _ => None,
        };
    }
    // No suffix: seconds
    let n: u64 = trimmed.parse().ok()?;
    Some(Duration::from_secs(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct EchoRunner;
    impl CommandRunner for EchoRunner {
        fn run(
            &self,
            run: &str,
            with: Option<&Value>,
            body: Option<&Value>,
            _ctx: &RunContext,
        ) -> anyhow::Result<Value> {
            Ok(json!({
                "cmd": run,
                "with": with.cloned().unwrap_or(Value::Null),
                "body": body.cloned().unwrap_or(Value::Null),
                "status": "ok"
            }))
        }
    }

    #[test]
    fn prepare_plan_interpolates_inputs() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            inputs: Default::default(),
            steps: vec![StepSpec {
                id: "s1".into(),
                run: "echo".into(),
                with: Some(json!({"name": "${{ inputs.app }}"}).as_object().unwrap().clone()),
                body: None,
                repeat: None,
                r#if: None,
                output_contract: None,
            }],
        };
        let mut ctx = RunContext::default();
        ctx.inputs.insert("app".into(), json!("myapp"));
        let plan = prepare_plan(&spec, &ctx);
        let step = &plan.steps[0];
        assert_eq!(step.with.as_ref().unwrap()["name"], "myapp");
    }

    #[test]
    fn run_step_persists_output_and_respects_condition() {
        let step = PreparedStep {
            id: "s1".into(),
            run: "do".into(),
            with: None,
            body: None,
            r#if: Some("inputs.enabled == \"true\"".into()),
            repeat: None,
        };
        let runner = EchoRunner;
        let mut ctx = RunContext::default();
        ctx.inputs.insert("enabled".into(), json!("true"));
        let res = run_step_with(&step, &ctx, &runner);
        assert_eq!(res.status, StepStatus::Succeeded);

        let mut ctx2 = RunContext::default();
        ctx2.inputs.insert("enabled".into(), json!("false"));
        let res2 = run_step_with(&step, &ctx2, &runner);
        assert_eq!(res2.status, StepStatus::Skipped);
    }

    #[test]
    fn repeat_until_stops_and_updates_context() {
        // until: steps.s1.status == "ok" (true immediately), guard avoids loops
        let step = PreparedStep {
            id: "s1".into(),
            run: "echo".into(),
            with: None,
            body: None,
            r#if: None,
            repeat: Some(StepRepeat {
                until: "steps.s1.status == \"ok\"".into(),
                every: "1s".into(),
            }),
        };
        let runner = EchoRunner;
        let mut ctx = RunContext::default();
        let res = run_step_repeating_with(&step, &mut ctx, &runner);
        assert_eq!(res.status, StepStatus::Succeeded);
        assert!(ctx.steps.contains_key("s1"));
        assert!(res.attempts >= 1);
    }
}
