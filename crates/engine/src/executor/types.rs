//! Core executor data types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::StepRepeat;

/// Prepared step with inputs/body interpolated against the provided context.
///
/// This is the unit executed by the engine. Each `PreparedStep` is derived from a
/// `StepSpec` by applying string interpolation based on a `RunContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedStep {
    /// Unique identifier for this step within a workflow.
    pub id: String,
    /// List of step identifiers that must complete successfully before this step runs.
    #[serde(default)]
    pub depends_on: Vec<String>,
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
