//! # Workflow Model Definitions
//!
//! This module contains the core data structures that define workflow specifications,
//! including workflows, steps, inputs, and their associated metadata. These structures
//! form the foundation of the workflow engine and are designed to be serializable
//! from both YAML and JSON formats.
//!
//! ## Core Concepts
//!
//! - **WorkflowBundle**: A collection of named workflows that can be executed together
//! - **WorkflowSpec**: A complete workflow definition with inputs, steps, and metadata
//! - **StepSpec**: Individual execution units within a workflow
//! - **InputSpec**: Declarative input definitions with validation and provider support
//!
//! ## Usage
//!
//! ```rust
//! use oatty_engine::model::{WorkflowBundle, WorkflowSpec, StepSpec};
//! use serde_json::json;
//! use std::collections::HashMap;
//!
//! let workflow_spec = WorkflowSpec {
//!     workflow: Some("deploy-app".to_string()),
//!     name: Some("Deploy App".to_string()),
//!     inputs: HashMap::new(),
//!     steps: vec![
//!         StepSpec {
//!             id: "deploy".to_string(),
//!             depends_on: vec![],
//!             run: "apps:deploy".to_string(),
//!             with: None,
//!             body: None,
//!             repeat: None,
//!             r#if: None,
//!             output_contract: None,
//!         }
//!     ],
//! };
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A collection of named workflows that can be executed together.
///
/// This structure represents a workflow bundle file that may contain multiple
/// workflow definitions. It's the top-level container for workflow specifications
/// and supports both single-workflow and multi-workflow documents.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowBundle {
    /// Mapping of workflow names to their specifications
    pub workflows: HashMap<String, WorkflowSpec>,
}

/// Complete specification for a single workflow.
///
/// A workflow specification defines the complete structure of an automation
/// workflow, including its inputs, execution steps, and optional metadata.
/// This structure supports both declarative input validation and dynamic
/// step execution with conditional logic.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowSpec {
    /// Optional workflow name for identification and reference
    ///
    /// In single-workflow files, this field provides a human-readable
    /// identifier for the workflow. In multi-workflow bundles, this
    /// field is typically derived from the key in the workflows map.
    #[serde(default)]
    pub workflow: Option<String>,
    /// Optional human-friendly display name for the workflow.
    ///
    /// When present, user interfaces prefer this value instead of the identifier when
    /// presenting the workflow to an operator. Consumers should gracefully fall back to
    /// `workflow` should this value be missing or blank.
    #[serde(default)]
    pub name: Option<String>,

    /// Declarative input definitions for the workflow
    ///
    /// Inputs define the parameters that must be provided when executing
    /// the workflow. Each input can have validation rules, default values,
    /// and provider-based value resolution.
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,

    /// Ordered sequence of steps to execute
    ///
    /// Steps are executed in sequence and can reference inputs, environment
    /// variables, and outputs from previous steps. Each step represents
    /// a single unit of work within the workflow.
    #[serde(default)]
    pub steps: Vec<StepSpec>,
}

/// Specification for a workflow input parameter.
///
/// Input specifications define the structure, validation rules, and
/// value resolution for workflow parameters. They support various
/// input types including text, selections, and provider-based values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InputSpec {
    /// Human-readable description of the input parameter
    ///
    /// This description is used for documentation and user interface
    /// generation. It should clearly explain what the input represents
    /// and any constraints or requirements.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional human-friendly label for the input.
    ///
    /// Workflows can supply this value to present nicer labels than the raw identifier.
    /// Consumers must fall back to the identifier when this field is absent or blank.
    #[serde(default)]
    pub name: Option<String>,

    /// Data type of the input parameter
    ///
    /// Supported types include "string", "number", "boolean", and
    /// custom types defined by providers. This field is used for
    /// validation and UI generation.
    #[serde(default, rename = "type")]
    pub r#type: Option<String>,

    /// Provider identifier for dynamic value resolution
    ///
    /// Providers can supply dynamic values for inputs, such as
    /// available applications, regions, or other contextual data.
    /// The provider is responsible for resolving the actual values.
    #[serde(default)]
    pub provider: Option<String>,

    /// Additional arguments passed to the value provider
    ///
    /// These arguments configure how the provider resolves values,
    /// such as filtering criteria or authentication parameters.
    #[serde(default)]
    pub provider_args: Option<serde_json::Map<String, Value>>,

    /// Selection configuration for choice-based inputs
    ///
    /// When an input represents a choice from a predefined set,
    /// this field configures how the options are presented and
    /// how the selection is made.
    #[serde(default)]
    pub select: Option<SelectSpec>,

    /// Default value for the input parameter
    ///
    /// If no value is provided when executing the workflow, this
    /// default value will be used. The value must conform to the
    /// input's type and validation rules.
    #[serde(default)]
    pub default: Option<Value>,

    /// Predefined set of valid values for the input
    ///
    /// This field restricts the input to a specific set of values,
    /// typically used for enumerated types or when the provider
    /// returns a fixed set of options.
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<Value>>,

    /// Input mode for special behaviors
    ///
    /// Special modes like "multiple" for multi-select inputs
    /// or "secret" for sensitive data can be specified here.
    /// The exact behavior depends on the input type and provider.
    #[serde(default)]
    pub mode: Option<String>,
}

/// Configuration for selection-based input parameters.
///
/// Selection specifications define how choice-based inputs are
/// presented to users and how the selected values are processed.
/// They support both simple value lists and complex object selections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectSpec {
    /// Field name that contains the actual value
    ///
    /// When the input represents a selection from a list of objects,
    /// this field specifies which object property contains the
    /// actual value to be used in the workflow.
    pub value_field: String,

    /// Field name used for display purposes
    ///
    /// This field specifies which object property should be shown
    /// to users when presenting the selection options. It's used
    /// for user interface generation and should be human-readable.
    pub display_field: String,

    /// Optional field name for unique identification
    ///
    /// If the selection options have unique identifiers that differ
    /// from the value field, this field can specify where to find
    /// the ID. This is useful for tracking selections across
    /// workflow executions.
    #[serde(default)]
    pub id_field: Option<String>,
}

/// Specification for a single workflow execution step.
///
/// Each step represents a unit of work within the workflow and
/// can execute commands, make API calls, or perform other actions.
/// Steps can reference workflow inputs, environment variables,
/// and outputs from previous steps.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StepSpec {
    /// Unique identifier for the step within the workflow
    ///
    /// This identifier is used to reference the step's outputs
    /// in subsequent steps and conditional expressions. It should
    /// be descriptive and unique within the workflow.
    pub id: String,

    /// List of step identifiers that must complete successfully before this step runs.
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Command or action to execute
    ///
    /// The run field specifies what action to perform. This can be:
    /// - A registry command like "apps:create"
    /// - A provider action like "shell:curl_put"
    /// - A custom action defined by the workflow engine
    pub run: String,

    /// Parameters and configuration for the step
    ///
    /// The with field contains key-value pairs that configure
    /// how the step executes. These can include command-line
    /// arguments, API parameters, or other configuration data.
    /// Template expressions are supported for dynamic values.
    #[serde(default)]
    pub with: Option<serde_json::Map<String, Value>>,

    /// Request body for API-based steps
    ///
    /// When the step makes an API call, this field contains
    /// the request body. It's typically used for POST, PUT, or
    /// PATCH operations that require data payloads.
    #[serde(default)]
    pub body: Option<Value>,

    /// Configuration for repeating step execution
    ///
    /// Some steps may need to execute multiple times, such as
    /// polling for completion or retrying failed operations.
    /// This field configures the repetition behavior.
    #[serde(default)]
    pub repeat: Option<StepRepeat>,

    /// Conditional expression for step execution
    ///
    /// If specified, this expression must evaluate to true for
    /// the step to execute. The expression can reference inputs,
    /// environment variables, and outputs from previous steps.
    /// Uses the same syntax as template expressions.
    #[serde(default, rename = "if")]
    pub r#if: Option<String>,

    /// Output contract for automatic value mapping
    ///
    /// Output contracts define the structure and metadata of
    /// step outputs, enabling automatic mapping to subsequent
    /// steps and workflow outputs.
    #[serde(default)]
    pub output_contract: Option<OutputContract>,
}

/// Configuration for repeating step execution.
///
/// Repeat configurations enable steps to execute multiple times
/// based on conditions or time intervals. This is useful for
/// polling operations, retry logic, and iterative processing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StepRepeat {
    /// Expression that determines when to stop repeating
    ///
    /// This expression is evaluated against the step's output
    /// after each execution. When it evaluates to true, the
    /// repetition stops. The expression uses the same syntax
    /// as conditional expressions.
    pub until: String,

    /// Duration between repeat attempts
    ///
    /// This field specifies how long to wait between repeat
    /// executions. The format is a human-readable duration
    /// string like "10s", "1m", or "5m30s". The exact parsing
    /// is handled by the workflow executor.
    pub every: String,

    /// Optional timeout after which the repeating step aborts.
    ///
    /// Uses the same duration syntax as `every`.
    #[serde(default)]
    pub timeout: Option<String>,

    /// Optional maximum number of attempts before aborting.
    #[serde(default)]
    pub max_attempts: Option<u32>,
}

/// Definition of a step's output structure and metadata.
///
/// Output contracts enable automatic mapping of step outputs
/// to subsequent steps and workflow results. They provide
/// metadata about the output structure and can include tags
/// for intelligent value resolution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputContract {
    /// Fields that make up the step's output
    ///
    /// Each field defines a piece of output data with its
    /// name, type, and optional metadata. Fields can be
    /// tagged for automatic mapping and validation.
    #[serde(default)]
    pub fields: Vec<ContractField>,
}

/// Definition of a single output field within a step's output.
///
/// Contract fields define the structure and metadata of step
/// outputs, enabling automatic mapping and validation across
/// the workflow. They support tagging for intelligent value
/// resolution and type information for validation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractField {
    /// Name of the output field
    ///
    /// This name is used to reference the field in subsequent
    /// steps and template expressions. It should be descriptive
    /// and follow consistent naming conventions.
    pub name: String,

    /// Optional type information for the field
    ///
    /// Type information can be used for validation and UI
    /// generation. Supported types include basic JSON types
    /// and custom types defined by the workflow engine.
    #[serde(default)]
    pub r#type: Option<String>,

    /// Tags for automatic mapping and categorization
    ///
    /// Tags enable intelligent value resolution and automatic
    /// mapping between steps. Common tags include "id", "name",
    /// "url", and domain-specific identifiers.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_workflow_bundle_creation() {
        let mut bundle = WorkflowBundle::default();
        let workflow_spec = WorkflowSpec {
            workflow: Some("test-workflow".to_string()),
            name: Some("Test workflow".to_string()),
            inputs: HashMap::new(),
            steps: vec![],
        };

        bundle.workflows.insert("test".to_string(), workflow_spec);

        assert_eq!(bundle.workflows.len(), 1);
        assert!(bundle.workflows.contains_key("test"));
    }

    #[test]
    fn test_step_spec_with_conditional() {
        let step = StepSpec {
            id: "conditional-step".to_string(),
            depends_on: vec![],
            run: "test:command".to_string(),
            with: None,
            body: None,
            repeat: None,
            r#if: Some("inputs.environment == \"production\"".to_string()),
            output_contract: None,
        };

        assert_eq!(step.id, "conditional-step");
        assert_eq!(step.run, "test:command");
        assert!(step.r#if.is_some());
    }

    #[test]
    fn test_input_spec_with_provider() {
        let input = InputSpec {
            description: Some("Select an application".to_string()),
            name: Some("Application".to_string()),
            r#type: Some("string".to_string()),
            provider: Some("apps:list".to_string()),
            provider_args: Some(
                json!({
                    "team": "my-team"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
            select: None,
            default: None,
            enum_values: None,
            mode: None,
        };

        assert_eq!(input.description, Some("Select an application".to_string()));
        assert_eq!(input.r#type, Some("string".to_string()));
        assert_eq!(input.provider, Some("apps:list".to_string()));
    }

    #[test]
    fn test_select_spec_creation() {
        let select = SelectSpec {
            value_field: "id".to_string(),
            display_field: "name".to_string(),
            id_field: Some("uuid".to_string()),
        };

        assert_eq!(select.value_field, "id");
        assert_eq!(select.display_field, "name");
        assert_eq!(select.id_field, Some("uuid".to_string()));
    }

    #[test]
    fn test_step_repeat_configuration() {
        let repeat = StepRepeat {
            until: "output.status == \"completed\"".to_string(),
            every: "30s".to_string(),
            ..Default::default()
        };

        assert_eq!(repeat.until, "output.status == \"completed\"");
        assert_eq!(repeat.every, "30s");
    }

    #[test]
    fn test_output_contract_with_fields() {
        let contract = OutputContract {
            fields: vec![
                ContractField {
                    name: "application_id".to_string(),
                    r#type: Some("string".to_string()),
                    tags: vec!["id".to_string(), "app".to_string()],
                },
                ContractField {
                    name: "status".to_string(),
                    r#type: Some("string".to_string()),
                    tags: vec!["status".to_string()],
                },
            ],
        };

        assert_eq!(contract.fields.len(), 2);
        assert_eq!(contract.fields[0].name, "application_id");
        assert_eq!(contract.fields[0].tags, vec!["id", "app"]);
        assert_eq!(contract.fields[1].name, "status");
    }
}
