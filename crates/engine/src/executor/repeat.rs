//! Repeat step execution helpers.
//!
//! This module owns polling/repeat behavior for workflow steps and keeps the
//! orchestration surface in `executor::mod` focused on planning and step flow.

use std::{thread, time::Duration};

use tracing::{info, warn};

use crate::resolve::{RunContext, eval_condition, find_unresolved_references_in_condition};

use super::{CommandRunner, PreparedStep, StepResult, StepStatus, run_step_with};

/// Max attempts for repeat/until steps to prevent infinite loops.
const MAX_REPEAT_ATTEMPTS: u32 = 100;
/// Default polling interval when repeat `every` is invalid or missing.
const DEFAULT_REPEAT_INTERVAL: Duration = Duration::from_secs(1);

/// Execute a prepared step with repeat/until semantics.
///
/// Persists the latest output into `ctx.steps[step.id]` after each attempt.
/// A failed command attempt is terminal and exits immediately.
pub(crate) fn run_step_repeating_with(step: &PreparedStep, ctx: &mut RunContext, runner: &dyn CommandRunner) -> StepResult {
    run_step_repeating_with_observer(step, ctx, runner, |_| {})
}

/// Execute a repeating step and notify observers on each attempt.
pub(crate) fn run_step_repeating_with_observer<F>(
    step: &PreparedStep,
    ctx: &mut RunContext,
    runner: &dyn CommandRunner,
    mut observer: F,
) -> StepResult
where
    F: FnMut(u32),
{
    if let Some(condition) = &step.r#if
        && !eval_condition(condition, ctx)
    {
        let unresolved_references = find_unresolved_references_in_condition(condition, ctx);
        let mut skipped = StepResult {
            id: step.id.clone(),
            ..Default::default()
        };
        skipped.status = StepStatus::Skipped;
        if unresolved_references.is_empty() {
            skipped.logs.push(format!("step '{}' skipped by condition", step.id));
            info!(step_id = %step.id, "repeat step skipped by condition");
        } else {
            skipped.logs.push(format!(
                "step '{}' skipped by unresolved condition references: {}",
                step.id,
                unresolved_references.join(", ")
            ));
            info!(step_id = %step.id, "repeat step skipped by unresolved condition");
        }
        return skipped;
    }

    info!(
        step_id = %step.id,
        has_until = step.repeat.as_ref().is_some_and(|repeat| !repeat.until.trim().is_empty()),
        "repeat step started"
    );

    let max_attempts = step
        .repeat
        .as_ref()
        .and_then(|repeat| repeat.max_attempts)
        .unwrap_or(MAX_REPEAT_ATTEMPTS)
        .clamp(1, MAX_REPEAT_ATTEMPTS);
    let sleep_duration = step
        .repeat
        .as_ref()
        .and_then(|repeat| parse_repeat_interval(repeat.every.as_str()))
        .unwrap_or(DEFAULT_REPEAT_INTERVAL);
    let until_expression = step.repeat.as_ref().map(|repeat| repeat.until.clone());

    let mut attempts = 0u32;
    let result: StepResult = loop {
        attempts += 1;
        observer(attempts);
        let single_attempt_result = run_step_with(step, ctx, runner);
        ctx.steps.insert(step.id.clone(), single_attempt_result.output.clone());

        if matches!(single_attempt_result.status, StepStatus::Failed) {
            let mut terminal_result = single_attempt_result;
            terminal_result.attempts = attempts;
            break terminal_result;
        }

        if let Some(until_expression) = until_expression.as_deref() {
            let unresolved_references = find_unresolved_references_in_condition(until_expression, ctx);
            if !unresolved_references.is_empty() {
                let mut terminal_result = single_attempt_result;
                terminal_result.status = StepStatus::Failed;
                terminal_result
                    .logs
                    .push(format!("repeat.until unresolved references: {}", unresolved_references.join(", ")));
                terminal_result.attempts = attempts;
                warn!(step_id = %step.id, attempts, "repeat step failed due to unresolved until reference");
                break terminal_result;
            }
        }

        if until_expression
            .as_deref()
            .map(|expression| eval_condition(expression, ctx))
            .unwrap_or(true)
        {
            let mut terminal_result = single_attempt_result;
            terminal_result.attempts = attempts;
            break terminal_result;
        }

        if attempts >= max_attempts {
            let mut terminal_result = single_attempt_result;
            terminal_result.status = StepStatus::Failed;
            terminal_result
                .logs
                .push(format!("repeat guard tripped at {} attempts; stopping", attempts));
            terminal_result.attempts = attempts;
            warn!(step_id = %step.id, attempts, "repeat guard tripped");
            break terminal_result;
        }

        thread::sleep(sleep_duration);
    };

    match result.status {
        StepStatus::Succeeded => info!(step_id = %step.id, attempts = result.attempts, "repeat step succeeded"),
        StepStatus::Failed => warn!(step_id = %step.id, attempts = result.attempts, "repeat step failed"),
        StepStatus::Skipped => info!(step_id = %step.id, attempts = result.attempts, "repeat step skipped"),
    }

    result
}

fn parse_repeat_interval(raw_interval: &str) -> Option<Duration> {
    let trimmed = raw_interval.trim();
    if trimmed.is_empty() {
        return None;
    }
    let last_character = trimmed.chars().last()?;
    if last_character.is_ascii_alphabetic() {
        let number = &trimmed[..trimmed.len() - 1];
        let value: u64 = number.parse().ok()?;
        return match last_character {
            's' | 'S' => Some(Duration::from_secs(value)),
            'm' | 'M' => Some(Duration::from_secs(value * 60)),
            _ => None,
        };
    }
    let value: u64 = trimmed.parse().ok()?;
    Some(Duration::from_secs(value))
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::{Value, json};

    use crate::{
        executor::{PreparedStep, StepStatus, run_step_repeating_with, runner::CommandRunner},
        model::StepRepeat,
        resolve::RunContext,
    };

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
    fn repeat_until_stops_and_updates_context() {
        let step = PreparedStep {
            id: "s1".into(),
            depends_on: vec![],
            run: "echo".into(),
            with: None,
            body: None,
            r#if: None,
            repeat: Some(StepRepeat {
                until: "steps.s1.status == \"ok\"".into(),
                every: "1s".into(),
                ..Default::default()
            }),
        };

        let runner = EchoRunner;
        let mut run_context = RunContext::default();
        let result = run_step_repeating_with(&step, &mut run_context, &runner);

        assert_eq!(result.status, StepStatus::Succeeded);
        assert!(run_context.steps.contains_key("s1"));
        assert!(result.attempts >= 1);
    }

    #[test]
    fn repeat_respects_configured_max_attempts() {
        let step = PreparedStep {
            id: "s1".into(),
            depends_on: vec![],
            run: "echo".into(),
            with: None,
            body: None,
            r#if: None,
            repeat: Some(StepRepeat {
                until: "steps.s1.status == \"ready\"".into(),
                every: "1s".into(),
                max_attempts: Some(2),
                ..Default::default()
            }),
        };

        let runner = EchoRunner;
        let mut run_context = RunContext::default();
        let result = run_step_repeating_with(&step, &mut run_context, &runner);

        assert_eq!(result.status, StepStatus::Failed);
        assert_eq!(result.attempts, 2);
        assert!(result.logs.iter().any(|line| line.contains("repeat guard tripped")));
    }

    #[test]
    fn repeat_stops_immediately_when_attempt_fails() {
        let step = PreparedStep {
            id: "s1".into(),
            depends_on: vec![],
            run: "fail".into(),
            with: None,
            body: None,
            r#if: None,
            repeat: Some(StepRepeat {
                until: "steps.s1.status == \"live\"".into(),
                every: "1s".into(),
                max_attempts: Some(5),
                ..Default::default()
            }),
        };

        let runner = FailRunner;
        let mut run_context = RunContext::default();
        let result = run_step_repeating_with(&step, &mut run_context, &runner);

        assert_eq!(result.status, StepStatus::Failed);
        assert_eq!(result.attempts, 1);
        assert!(!result.logs.iter().any(|line| line.contains("repeat guard tripped")));
    }

    #[test]
    fn repeat_until_fails_when_until_expression_references_unresolved_path() {
        let step = PreparedStep {
            id: "wait".into(),
            depends_on: vec![],
            run: "echo".into(),
            with: None,
            body: None,
            r#if: None,
            repeat: Some(StepRepeat {
                until: "steps.wait.value == \"ready\"".into(),
                every: "1s".into(),
                max_attempts: Some(3),
                ..Default::default()
            }),
        };

        let runner = EchoRunner;
        let mut run_context = RunContext::default();
        let result = run_step_repeating_with(&step, &mut run_context, &runner);
        assert_eq!(result.status, StepStatus::Failed);
        assert_eq!(result.attempts, 1);
        assert!(result.logs.iter().any(|line| line.contains("repeat.until unresolved references")));
    }
}
