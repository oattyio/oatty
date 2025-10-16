//! Strongly typed workflow schema definitions shared across the registry, engine, and TUI.
//!
//! The models defined here mirror the authoring semantics captured in
//! `specs/WORKFLOWS.md`, `specs/WORKFLOW_TUI.md`, and
//! `specs/WORKFLOW_VALUE_PROVIDERS_UX.md`. They intentionally preserve authoring order (via
//! `IndexMap`) so the guided experience can render inputs and steps in a predictable sequence.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Fully resolved workflow ready for runtime consumption.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RuntimeWorkflow {
    /// Canonical identifier used for lookups and telemetry.
    pub identifier: String,
    /// Optional title exposed in selection UI.
    pub title: Option<String>,
    /// Optional descriptive copy shown in detail panes.
    pub description: Option<String>,
    /// Declarative inputs keyed by authoring order.
    pub inputs: IndexMap<String, WorkflowInputDefinition>,
    /// Ordered execution steps.
    pub steps: Vec<WorkflowStepDefinition>,
}

/// Describes a fully authored workflow, including metadata, inputs, and sequential steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowDefinition {
    /// Canonical workflow identifier (for example, `app_with_db`).
    #[serde(default)]
    pub workflow: String,
    /// Optional human-readable title for menus.
    #[serde(default)]
    pub title: Option<String>,
    /// Optional descriptive copy surfaced in the picker or detail pane.
    #[serde(default)]
    pub description: Option<String>,
    /// Declarative input definitions keyed by input name, preserving author order.
    #[serde(default = "default_input_map")]
    pub inputs: IndexMap<String, WorkflowInputDefinition>,
    /// Ordered list of workflow steps executed sequentially.
    #[serde(default)]
    pub steps: Vec<WorkflowStepDefinition>,
}

/// Defines metadata for a single workflow input, including provider bindings and validation.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowInputDefinition {
    /// Descriptive text explaining the purpose of the input.
    #[serde(default)]
    pub description: Option<String>,
    /// Declares the primitive type (string, number, array, etc.).
    #[serde(default)]
    pub r#type: Option<String>,
    /// Provider configuration enumerating how dynamic values are populated.
    #[serde(default)]
    pub provider: Option<WorkflowValueProvider>,
    /// Select metadata describing which fields to display and submit from provider items.
    #[serde(default)]
    pub select: Option<WorkflowProviderSelect>,
    /// Selection mode (single vs. multiple) for the provider-backed UI.
    #[serde(default)]
    pub mode: WorkflowInputMode,
    /// Cache time-to-live for provider results in seconds.
    #[serde(default)]
    pub cache_ttl_sec: Option<u64>,
    /// Error handling policy when provider resolution fails.
    #[serde(default)]
    pub on_error: Option<WorkflowProviderErrorPolicy>,
    /// Default value sourcing strategy.
    #[serde(default)]
    pub default: Option<WorkflowInputDefault>,
    /// Provider argument bindings, keyed by argument name.
    #[serde(default = "default_provider_argument_map")]
    pub provider_args: IndexMap<String, WorkflowProviderArgumentValue>,
    /// Join configuration applied when `mode` is multiple and results must be concatenated.
    #[serde(default)]
    pub join: Option<WorkflowJoinConfiguration>,
    /// When true, this input does not block readiness. All inputs are required by default unless `optional: true`.
    #[serde(default)]
    pub optional: bool,
    /// Declarative validation metadata (required flags, enumerations, patterns).
    #[serde(default)]
    pub validate: Option<WorkflowInputValidation>,
    /// Placeholder text rendered when no selection is made.
    #[serde(default)]
    pub placeholder: Option<String>,
    /// Enumerated literals for manual authoring without providers.
    #[serde(rename = "enum")]
    #[serde(default)]
    pub enumerated_values: Vec<JsonValue>,
}

impl Default for WorkflowInputDefinition {
    fn default() -> Self {
        Self {
            description: None,
            r#type: None,
            provider: None,
            select: None,
            mode: WorkflowInputMode::Single,
            cache_ttl_sec: None,
            on_error: None,
            default: None,
            provider_args: default_provider_argument_map(),
            join: None,
            optional: false,
            validate: None,
            placeholder: None,
            enumerated_values: Vec::new(),
        }
    }
}

/// Selection metadata describing how to extract values from provider results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkflowProviderSelect {
    /// Field name inserted into flags or arguments when a value is chosen.
    #[serde(default)]
    pub value_field: Option<String>,
    /// Field name displayed as the primary label in the UI.
    #[serde(default)]
    pub display_field: Option<String>,
    /// Optional stable identifier to support caching and analytics.
    #[serde(default)]
    pub id_field: Option<String>,
}

/// Lists selection modes for provider-backed inputs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowInputMode {
    /// A single value must be selected.
    #[default]
    Single,
    /// Multiple values may be selected.
    Multiple,
}

/// Error handling policy applied when provider fetching fails.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowProviderErrorPolicy {
    /// Prompt the user to provide a manual value.
    Manual,
    /// Surface cached results instead of blocking the run.
    Cached,
    /// Fail the workflow initialization immediately.
    Fail,
}

/// Defines a provider configuration either by identifier or by an embedded object.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum WorkflowValueProvider {
    /// Shorthand string identifier (for example, `apps:list`).
    Id(String),
    /// Structured provider configuration with additional metadata.
    Detailed(WorkflowValueProviderDetailed),
}

/// Structured provider configuration matching the richer syntax in the specs.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowValueProviderDetailed {
    /// Identifier of the provider (for example, `apps:list` or `workflow`).
    pub id: String,
    /// Optional default field to read from provider results.
    #[serde(default)]
    pub field: Option<String>,
}

/// Declares how default values are derived for a workflow input.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowInputDefault {
    /// Source describing where the default originates.
    pub from: WorkflowDefaultSource,
    /// Optional literal or templated value associated with the source.
    #[serde(default)]
    pub value: Option<JsonValue>,
}

/// Enumerates supported default value sources.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDefaultSource {
    /// Use command history for the user.
    History,
    /// Use a literal value provided in the workflow document.
    Literal,
    /// Load from an environment variable resolved at runtime.
    Env,
    /// Reference a previous workflow task's output.
    WorkflowOutput,
}

/// Describes how multiple selected items should be concatenated into a single value.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowJoinConfiguration {
    /// Separator inserted between values.
    pub separator: String,
    /// Optional wrapper applied to each value before joining.
    #[serde(default)]
    pub wrap_each: Option<String>,
}

/// Declarative validation settings attached to an input.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowInputValidation {
    /// Whether a value must be provided.
    #[serde(default)]
    pub required: bool,
    /// Enumerated set of allowed values, if constrained.
    #[serde(rename = "enum")]
    #[serde(default)]
    pub allowed_values: Vec<JsonValue>,
    /// Regular expression pattern the value must match, when provided.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Minimum length for string inputs, when specified.
    #[serde(default)]
    pub min_length: Option<usize>,
    /// Maximum length for string inputs, when specified.
    #[serde(default)]
    pub max_length: Option<usize>,
}

/// Value assigned to a provider argument, either as a literal or as a structured binding.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum WorkflowProviderArgumentValue {
    /// Literal string or templated expression.
    Literal(String),
    /// Structured binding referencing prior inputs or step outputs.
    Binding(WorkflowProviderArgumentBinding),
}

/// Structured provider argument binding following the dependent provider spec.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowProviderArgumentBinding {
    /// Step identifier referenced by the binding.
    #[serde(default)]
    pub from_step: Option<String>,
    /// Input identifier referenced by the binding.
    #[serde(default)]
    pub from_input: Option<String>,
    /// Relative JSON path resolved from the chosen source.
    #[serde(default)]
    pub path: Option<String>,
    /// Whether the binding is required for provider execution.
    #[serde(default)]
    pub required: Option<bool>,
    /// Behavior when the referenced field is missing.
    #[serde(default)]
    pub on_missing: Option<WorkflowMissingBehavior>,
}

/// Behavior applied when a dependent value cannot be resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMissingBehavior {
    /// Prompt the user to resolve the ambiguity via the field picker.
    Prompt,
    /// Skip binding and allow manual entry.
    Skip,
    /// Fail the workflow immediately.
    Fail,
}

/// Describes a single step within a workflow.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowStepDefinition {
    /// Unique step identifier referenced by later bindings.
    pub id: String,
    /// Command to execute for the step (for example, `apps:create`).
    pub run: String,
    /// Optional descriptive copy surfaced in the UI timeline.
    #[serde(default)]
    pub description: Option<String>,
    /// Dependency list ensuring steps run after their prerequisites.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Structured parameters bound to positional arguments or flags.
    #[serde(default = "default_value_map")]
    pub with: IndexMap<String, JsonValue>,
    /// Request body payload, when applicable.
    #[serde(default = "default_json_null")]
    pub body: JsonValue,
    /// Optional repeat configuration for polling or retry loops.
    #[serde(default)]
    pub repeat: Option<WorkflowRepeat>,
    /// Output contract emitting schema tags for downstream bindings.
    #[serde(default)]
    pub output_contract: Option<WorkflowOutputContract>,
}

/// Repeat configuration enabling polling until a condition is met.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowRepeat {
    /// Exit condition expressed as a templated expression.
    #[serde(default)]
    pub until: Option<String>,
    /// Interval between iterations (for example, `10s`).
    #[serde(default)]
    pub every: Option<String>,
    /// Optional timeout duration after which the loop aborts.
    #[serde(default)]
    pub timeout: Option<String>,
    /// Maximum attempt count.
    #[serde(default)]
    pub max_attempts: Option<u32>,
}

/// Output contract advertised by a workflow step for downstream consumers.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowOutputContract {
    /// Structured field descriptors annotated with semantic tags.
    #[serde(default)]
    pub fields: Vec<WorkflowOutputField>,
}

/// Describes a single field made available by a workflow step output.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkflowOutputField {
    /// Field name exposed from the output payload.
    pub name: String,
    /// Semantic tags (for example, `app_id`, `pipeline_slug`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional description enhancing picker UX.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional JSON type hint (object, array<uuid>, etc.).
    #[serde(default)]
    pub r#type: Option<String>,
}

const fn default_json_null() -> JsonValue {
    JsonValue::Null
}

fn default_input_map() -> IndexMap<String, WorkflowInputDefinition> {
    IndexMap::new()
}

fn default_provider_argument_map() -> IndexMap<String, WorkflowProviderArgumentValue> {
    IndexMap::new()
}

fn default_value_map() -> IndexMap<String, JsonValue> {
    IndexMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deserializes_basic_workflow() {
        let yaml_text = r#"
workflow: app_with_db
inputs:
  app:
    provider: apps:list
    select:
      value_field: name
      display_field: name
steps:
  - id: create_app
    run: apps:create
    body:
      name: ${{ inputs.app }}
"#;

        let definition: WorkflowDefinition = serde_yaml::from_str(yaml_text).expect("deserialize workflow");

        assert_eq!(definition.workflow, "app_with_db");
        assert!(definition.inputs.contains_key("app"));
        assert_eq!(definition.steps.len(), 1);
        assert_eq!(definition.steps[0].id, "create_app");
    }

    #[test]
    fn repository_sample_workflow_parses() {
        let yaml_text = include_str!("../../../workflows/create_app_and_db.yaml");
        let definition: WorkflowDefinition = serde_yaml::from_str(yaml_text).expect("parse sample workflow");
        assert_eq!(definition.workflow, "app_with_db");
        assert!(definition.inputs.contains_key("app_name"));
        assert_eq!(definition.steps.len(), 3);
    }
}

impl WorkflowInputDefinition {
    /// Returns true when this input should not block readiness.
    ///
    /// New authoring semantics: inputs are required by default unless `optional: true` is set.
    /// Legacy `validate.required` is ignored for readiness; it may still be used by
    /// future per-field validation, but does not affect required/optional at the
    /// workflow level.
    pub fn is_optional(&self) -> bool {
        self.optional
    }

    /// Returns true when a value must be supplied before running the workflow.
    pub fn is_required(&self) -> bool {
        !self.optional
    }
}
