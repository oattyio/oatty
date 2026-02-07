//! Workflow MCP tool request payload types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunExecutionMode {
    /// Execute immediately in the current tool invocation.
    Sync,
    /// Choose execution strategy automatically and include recommendations.
    Auto,
    /// Prefer task-backed execution semantics for long-running workflows.
    Task,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowGetRequest {
    #[schemars(description = "Canonical workflow identifier.")]
    pub workflow_id: String,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowValidateRequest {
    #[schemars(description = "Workflow manifest document to validate.")]
    pub manifest_content: String,
    #[schemars(description = "Optional format hint: yaml or json.")]
    pub format: Option<String>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowSaveRequest {
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Workflow manifest document to persist.")]
    pub manifest_content: String,
    #[schemars(description = "Optional format hint for persisted content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when the target file already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow.get.")]
    pub expected_version: Option<String>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowDeleteRequest {
    #[schemars(description = "Canonical workflow identifier.")]
    pub workflow_id: String,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowRenameRequest {
    #[schemars(description = "Existing canonical workflow identifier.")]
    pub workflow_id: String,
    #[schemars(description = "New canonical workflow identifier.")]
    pub new_id: String,
    #[schemars(description = "Whether to overwrite if destination exists.")]
    pub overwrite: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowCancelRequest {
    #[schemars(description = "Workflow run identifier.")]
    pub run_id: String,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowRunRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional input overrides keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
    #[schemars(description = "Execution preference: sync, auto, or task.")]
    pub execution_mode: Option<WorkflowRunExecutionMode>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowResolveInputsRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional partial input values keyed by input name.")]
    pub partial_inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowPreviewInputsRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional partial input values keyed by input name.")]
    pub partial_inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowPreviewRenderedRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional input values keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowStepPlanRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional input values keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowPurgeHistoryRequest {
    #[schemars(description = "Optional workflow identifier filter.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional input keys used to select entries for purge.")]
    pub input_keys: Option<Vec<String>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowAuthorAndRunRequest {
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Draft workflow manifest document to persist and execute.")]
    pub manifest_content: String,
    #[schemars(description = "Optional format hint for persisted content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when the target file already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow.get.")]
    pub expected_version: Option<String>,
    #[schemars(description = "Optional input overrides keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowRepairAndRerunRequest {
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Current workflow manifest document.")]
    pub manifest_content: String,
    #[schemars(description = "Optional repaired workflow manifest document to use instead of manifest_content.")]
    pub repaired_manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for persisted content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when the target file already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow.get.")]
    pub expected_version: Option<String>,
    #[schemars(description = "Optional input overrides keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowExportRequest {
    #[schemars(description = "Canonical workflow identifier to export from runtime storage.")]
    pub workflow_id: String,
    #[schemars(description = "Project-relative output path for the exported manifest.")]
    pub output_path: String,
    #[schemars(description = "Optional output format override: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite destination if it already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Whether to create destination parent directories when missing.")]
    pub create_directories: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowImportRequest {
    #[schemars(description = "Project-relative input path for the manifest to import.")]
    pub input_path: String,
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional format hint for import content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when workflow already exists in runtime storage.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow.get.")]
    pub expected_version: Option<String>,
}
