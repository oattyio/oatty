//! Workflow runtime state management.
//!
//! The workflow engine evaluates provider-backed inputs iteratively as users
//! progress through a run. This module tracks the current workflow, maintains
//! the mutable [`RunContext`](RunContext), and persists the
//! outcome of each provider argument resolution, including manual overrides
//! supplied through the UI.

use std::collections::HashSet;

use crate::{
    RunContext,
    executor::{self, CommandRunner, StepResult, StepStatus, runner::NoopRunner},
    workflow::{
        bindings::{ProviderArgumentResolver, ProviderBindingOutcome},
        runtime::workflow_spec_from_runtime,
    },
};
use anyhow::Result;
use heroku_types::workflow::RuntimeWorkflow;
use indexmap::{IndexMap, map::Entry as IndexMapEntry};
use serde_json::Value;

/// Captures the outcome of resolving a single provider argument.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderOutcomeState {
    /// Resolution outcome (resolved value, prompt, skip, or error).
    pub outcome: ProviderBindingOutcome,
    /// Indicates whether the user manually supplied this outcome.
    pub locked_by_user: bool,
}

impl ProviderOutcomeState {
    fn from_outcome(outcome: ProviderBindingOutcome, locked_by_user: bool) -> Self {
        Self { outcome, locked_by_user }
    }
}

/// Aggregated provider outcomes for a single workflow input.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputProviderState {
    /// Latest outcome per provider argument.
    pub argument_outcomes: IndexMap<String, ProviderOutcomeState>,
}

/// Mutable runtime state for an executing workflow.
#[derive(Debug, Clone)]
pub struct WorkflowRunState {
    /// Immutable workflow metadata and steps.
    pub workflow: RuntimeWorkflow,
    /// Mutable execution context shared with the resolver and executor.
    pub run_context: RunContext,
    /// Cached provider argument outcomes keyed by input name.
    input_provider_states: IndexMap<String, InputProviderState>,
    /// Tracks arguments that have received manual overrides.
    locked_arguments: HashSet<(String, String)>,
    /// Telemetry collected during the run.
    telemetry: WorkflowTelemetry,
}

impl WorkflowRunState {
    /// Creates a new workflow run state with an empty execution context.
    pub fn new(workflow: RuntimeWorkflow) -> Self {
        Self {
            workflow,
            run_context: RunContext::default(),
            input_provider_states: IndexMap::new(),
            locked_arguments: HashSet::new(),
            telemetry: WorkflowTelemetry::default(),
        }
    }

    /// Returns an immutable view of the provider state for a given input.
    pub fn provider_state_for(&self, input_name: &str) -> Option<&InputProviderState> {
        self.input_provider_states.get(input_name)
    }

    /// Returns a mutable reference to the underlying execution context.
    pub fn run_context_mut(&mut self) -> &mut RunContext {
        &mut self.run_context
    }

    /// Evaluates provider arguments for all inputs, preserving any user overrides.
    pub fn evaluate_input_providers(&mut self) -> Result<()> {
        let resolver = ProviderArgumentResolver::new(&self.run_context);

        for (input_name, definition) in &self.workflow.inputs {
            if definition.provider_args.is_empty() {
                continue;
            }

            let outcomes = resolver.resolve_arguments(&definition.provider_args);
            let state = self.input_provider_states.entry(input_name.clone()).or_default();

            for (argument_name, outcome) in outcomes {
                let key = (input_name.clone(), argument_name.clone());
                let is_locked = self.locked_arguments.contains(&key);

                match state.argument_outcomes.entry(argument_name.clone()) {
                    IndexMapEntry::Occupied(mut entry) => {
                        if entry.get().locked_by_user {
                            continue;
                        }
                        if entry.get().outcome != outcome {
                            entry.insert(ProviderOutcomeState::from_outcome(outcome.clone(), false));
                            self.telemetry.record_provider_resolution(ProviderResolutionEvent {
                                input: input_name.clone(),
                                argument: argument_name,
                                outcome,
                                source: ProviderResolutionSource::Automatic,
                            });
                        }
                    }
                    IndexMapEntry::Vacant(entry) => {
                        entry.insert(ProviderOutcomeState::from_outcome(outcome.clone(), is_locked));
                        self.telemetry.record_provider_resolution(ProviderResolutionEvent {
                            input: input_name.clone(),
                            argument: argument_name,
                            outcome,
                            source: ProviderResolutionSource::Automatic,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Persists a user-supplied outcome for a specific provider argument.
    pub fn persist_provider_outcome(&mut self, input_name: &str, argument_name: &str, outcome: ProviderBindingOutcome) {
        let state = self.input_provider_states.entry(input_name.to_string()).or_default();

        state
            .argument_outcomes
            .insert(argument_name.to_string(), ProviderOutcomeState::from_outcome(outcome.clone(), true));
        self.locked_arguments.insert((input_name.to_string(), argument_name.to_string()));
        self.telemetry.record_provider_resolution(ProviderResolutionEvent {
            input: input_name.to_string(),
            argument: argument_name.to_string(),
            outcome,
            source: ProviderResolutionSource::Manual,
        });
    }

    /// Applies a manual value to the `RunContext` inputs table.
    pub fn set_input_value(&mut self, input_name: &str, value: Value) {
        self.run_context.inputs.insert(input_name.to_string(), value);
    }

    /// Executes the workflow using the provided runner and records telemetry.
    pub fn execute_with_runner(&mut self, runner: &dyn CommandRunner) -> Vec<StepResult> {
        let spec = workflow_spec_from_runtime(&self.workflow);
        let results = executor::execute_workflow_with_runner(&spec, &mut self.run_context, runner);

        for result in &results {
            self.telemetry.record_step_event(StepTelemetryEvent {
                step_id: result.id.clone(),
                status: result.status,
            });
        }

        results
    }

    /// Executes the workflow using the no-op runner (useful for previews and tests).
    pub fn execute(&mut self) -> Vec<StepResult> {
        let runner = NoopRunner;
        self.execute_with_runner(&runner)
    }

    /// Persists a step output and records a placeholder telemetry event.
    pub fn record_step_result(&mut self, step_id: &str, status: StepStatus, output: Value) {
        self.run_context.steps.insert(step_id.to_string(), output);
        self.telemetry.record_step_event(StepTelemetryEvent {
            step_id: step_id.to_string(),
            status,
        });
    }

    /// Returns a read-only view of the accumulated telemetry.
    pub fn telemetry(&self) -> &WorkflowTelemetry {
        &self.telemetry
    }
}

/// Aggregated telemetry emitted during a workflow run.
#[derive(Debug, Default, Clone)]
pub struct WorkflowTelemetry {
    provider_resolutions: Vec<ProviderResolutionEvent>,
    step_events: Vec<StepTelemetryEvent>,
}

impl WorkflowTelemetry {
    fn record_provider_resolution(&mut self, event: ProviderResolutionEvent) {
        self.provider_resolutions.push(event);
    }

    fn record_step_event(&mut self, event: StepTelemetryEvent) {
        self.step_events.push(event);
    }

    /// Returns recorded provider resolution events.
    pub fn provider_resolution_events(&self) -> &[ProviderResolutionEvent] {
        &self.provider_resolutions
    }

    /// Returns recorded step execution events.
    pub fn step_events(&self) -> &[StepTelemetryEvent] {
        &self.step_events
    }
}

/// Describes the origin of a provider resolution event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderResolutionSource {
    /// Automatically resolved based on the current run context.
    Automatic,
    /// Supplied manually by the user through the UI.
    Manual,
}

/// Structured provider resolution telemetry event.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderResolutionEvent {
    /// Input that triggered the resolution.
    pub input: String,
    /// Specific argument inside the provider payload.
    pub argument: String,
    /// Resolution outcome in effect after the event.
    pub outcome: ProviderBindingOutcome,
    /// Origin marker identifying whether this was automatic or manual.
    pub source: ProviderResolutionSource,
}

/// Placeholder step execution telemetry event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepTelemetryEvent {
    /// Identifier of the step that executed.
    pub step_id: String,
    /// Final status emitted by the executor.
    pub status: StepStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::runner::NoopRunner;
    use heroku_types::workflow::{
        WorkflowInputDefinition, WorkflowJoinConfiguration, WorkflowProviderArgumentBinding, WorkflowProviderArgumentValue,
        WorkflowProviderErrorPolicy, WorkflowStepDefinition, WorkflowValueProvider,
    };
    use indexmap::indexmap;

    fn demo_workflow() -> RuntimeWorkflow {
        let mut input_definition = WorkflowInputDefinition::default();
        input_definition.provider = Some(WorkflowValueProvider::Id("apps:list".into()));
        input_definition.join = Some(WorkflowJoinConfiguration {
            separator: ",".into(),
            wrap_each: None,
        });
        input_definition.on_error = Some(WorkflowProviderErrorPolicy::Manual);
        input_definition.provider_args = indexmap! {
            "app".into() => WorkflowProviderArgumentValue::Literal("demo-app".into())
        };

        RuntimeWorkflow {
            identifier: "demo".into(),
            title: None,
            description: None,
            inputs: indexmap! {"target".into() => input_definition},
            steps: vec![WorkflowStepDefinition {
                id: "noop".into(),
                run: "apps:list".into(),
                description: None,
                depends_on: Vec::new(),
                with: IndexMap::new(),
                body: Value::Null,
                repeat: None,
                output_contract: None,
            }],
        }
    }

    #[test]
    fn evaluates_provider_arguments() {
        let workflow = demo_workflow();
        let mut state = WorkflowRunState::new(workflow);

        state.evaluate_input_providers().expect("evaluate providers");

        let provider_state = state.provider_state_for("target").expect("missing provider state");
        let outcome = provider_state.argument_outcomes.get("app").expect("missing argument outcome");
        assert!(matches!(outcome.outcome, ProviderBindingOutcome::Resolved(_)));
        assert!(!outcome.locked_by_user);

        let telemetry = state.telemetry();
        assert_eq!(telemetry.provider_resolution_events().len(), 1);
        assert_eq!(telemetry.step_events().len(), 0);

        let telemetry = state.telemetry();
        assert_eq!(telemetry.provider_resolution_events().len(), 1);
        assert_eq!(telemetry.step_events().len(), 0);
    }

    #[test]
    fn preserves_manual_outcome() {
        let workflow = demo_workflow();
        let mut state = WorkflowRunState::new(workflow);

        state.evaluate_input_providers().expect("evaluate providers");
        state.persist_provider_outcome("target", "app", ProviderBindingOutcome::Resolved(Value::String("custom".into())));

        state
            .evaluate_input_providers()
            .expect("re-evaluate providers with manual overrides");

        let provider_state = state.provider_state_for("target").expect("missing provider state");
        let outcome = provider_state.argument_outcomes.get("app").expect("missing argument outcome");
        assert!(matches!(outcome.outcome, ProviderBindingOutcome::Resolved(Value::String(ref value)) if value == "custom"));
        assert!(outcome.locked_by_user);

        let telemetry = state.telemetry();
        assert_eq!(telemetry.provider_resolution_events().len(), 2);
        assert!(matches!(
            telemetry.provider_resolution_events()[1].source,
            ProviderResolutionSource::Manual
        ));

        let telemetry = state.telemetry();
        assert_eq!(telemetry.provider_resolution_events().len(), 2);
        assert!(matches!(
            telemetry.provider_resolution_events()[1].source,
            ProviderResolutionSource::Manual
        ));
    }

    #[test]
    fn resolves_binding_using_run_context_inputs() {
        let mut workflow = demo_workflow();
        // Replace literal with a binding referring to run context input.
        let binding = WorkflowProviderArgumentBinding {
            from_step: None,
            from_input: Some("source".into()),
            path: Some("name".into()),
            required: Some(true),
            on_missing: None,
        };
        workflow.inputs.get_mut("target").unwrap().provider_args = indexmap! {
            "app".into() => WorkflowProviderArgumentValue::Binding(binding)
        };

        let mut state = WorkflowRunState::new(workflow);
        let mut source_object = serde_json::Map::new();
        source_object.insert("name".into(), Value::String("context-app".into()));
        state.set_input_value("source", Value::Object(source_object));

        state.evaluate_input_providers().expect("evaluate providers");

        let provider_state = state.provider_state_for("target").expect("missing provider state");
        let outcome = provider_state.argument_outcomes.get("app").expect("missing argument outcome");
        assert!(matches!(outcome.outcome, ProviderBindingOutcome::Resolved(Value::String(ref value)) if value == "context-app"));
    }

    #[test]
    fn executes_workflow_with_runner() {
        let workflow = demo_workflow();
        let mut state = WorkflowRunState::new(workflow);

        let results = state.execute_with_runner(&NoopRunner);

        assert_eq!(results.len(), 1);
        let telemetry = state.telemetry();
        assert_eq!(telemetry.step_events().len(), 1);
        assert_eq!(telemetry.step_events()[0].step_id, "noop");
    }

    #[test]
    fn records_step_events() {
        let workflow = demo_workflow();
        let mut state = WorkflowRunState::new(workflow);

        let _ = state.execute();

        let telemetry = state.telemetry();
        assert_eq!(telemetry.step_events().len(), 1);
        assert_eq!(telemetry.step_events()[0].status, StepStatus::Succeeded);
    }
}
