//! Workflow MCP tool request payload types.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
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

fn manifest_document_to_string(document: Value) -> Result<String, String> {
    match document {
        Value::String(text) => Ok(text),
        non_string_document => {
            serde_json::to_string_pretty(&non_string_document).map_err(|error| format!("failed to serialize manifest_content: {error}"))
        }
    }
}

fn deserialize_manifest_document<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let document = Value::deserialize(deserializer)?;
    manifest_document_to_string(document).map_err(serde::de::Error::custom)
}

fn deserialize_optional_manifest_document<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let document = Option::<Value>::deserialize(deserializer)?;
    document
        .map(manifest_document_to_string)
        .transpose()
        .map_err(serde::de::Error::custom)
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowGetRequest {
    #[schemars(description = "Canonical workflow identifier.")]
    pub workflow_id: String,
    #[schemars(description = "Include manifest content text in the response. Defaults to true.")]
    pub include_content: Option<bool>,
    #[schemars(description = "Include parsed manifest JSON in the response. Defaults to false.")]
    pub include_parsed: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowValidateRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(
        description = "Optional inline workflow manifest document to validate.",
        with = "Option<serde_json::Value>"
    )]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional absolute filesystem path to a workflow manifest file to validate.")]
    pub input_path: Option<String>,
    #[schemars(description = "Optional format hint: yaml or json.")]
    pub format: Option<String>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowSaveRequest {
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Workflow manifest document to persist.", with = "serde_json::Value")]
    #[serde(deserialize_with = "deserialize_manifest_document")]
    pub manifest_content: String,
    #[schemars(description = "Optional format hint for persisted content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when the target file already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow_get.")]
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
    #[schemars(description = "Task operation identifier returned for task-backed workflow execution.")]
    pub operation_id: String,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowRunRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.", with = "Option<serde_json::Value>")]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional input overrides keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
    #[schemars(description = "Execution preference: sync, auto, or task.")]
    pub execution_mode: Option<WorkflowRunExecutionMode>,
    #[schemars(description = "Include step result entries in the response. Defaults to true.")]
    pub include_results: Option<bool>,
    #[schemars(description = "Include aggregated step outputs in the response. Defaults to false.")]
    pub include_outputs: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowResolveInputsRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.", with = "Option<serde_json::Value>")]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional partial input values keyed by input name.")]
    pub partial_inputs: Option<HashMap<String, Value>>,
    #[schemars(description = "Include resolved input values in the response. Defaults to false.")]
    pub include_resolved_inputs: Option<bool>,
    #[schemars(description = "Include provider resolution events in the response. Defaults to false.")]
    pub include_provider_resolutions: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowPreviewInputsRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.", with = "Option<serde_json::Value>")]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional partial input values keyed by input name.")]
    pub partial_inputs: Option<HashMap<String, Value>>,
    #[schemars(description = "Include per-input detail rows in the response. Defaults to false.")]
    pub include_inputs: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowPreviewRenderedRequest {
    #[schemars(description = "Optional existing workflow identifier.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional inline workflow manifest document.", with = "Option<serde_json::Value>")]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
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
    #[schemars(description = "Optional inline workflow manifest document.", with = "Option<serde_json::Value>")]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
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
    #[schemars(
        description = "Draft workflow manifest document to persist and execute.",
        with = "serde_json::Value"
    )]
    #[serde(deserialize_with = "deserialize_manifest_document")]
    pub manifest_content: String,
    #[schemars(description = "Optional format hint for persisted content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when the target file already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow_get.")]
    pub expected_version: Option<String>,
    #[schemars(description = "Optional input overrides keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowRepairAndRerunRequest {
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Current workflow manifest document.", with = "serde_json::Value")]
    #[serde(deserialize_with = "deserialize_manifest_document")]
    pub manifest_content: String,
    #[schemars(
        description = "Optional repaired workflow manifest document to use instead of manifest_content.",
        with = "Option<serde_json::Value>"
    )]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub repaired_manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for persisted content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when the target file already exists.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow_get.")]
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
    #[schemars(description = "Absolute input path for the manifest to import.")]
    pub input_path: String,
    #[schemars(description = "Optional workflow identifier override.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional format hint for import content: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Whether to overwrite when workflow already exists in runtime storage.")]
    pub overwrite: Option<bool>,
    #[schemars(description = "Optional optimistic concurrency version from workflow_get.")]
    pub expected_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{WorkflowSaveRequest, WorkflowValidateRequest};

    #[test]
    fn workflow_validate_accepts_object_manifest_content() {
        let request: WorkflowValidateRequest = serde_json::from_value(serde_json::json!({
            "manifest_content": {
                "workflow": "object_manifest",
                "steps": [{"id": "a", "run": "apps list"}]
            },
            "format": "json"
        }))
        .expect("deserialize validate request");

        let manifest_content = request.manifest_content.expect("manifest content");
        assert!(manifest_content.contains("\"workflow\": \"object_manifest\""));
        assert!(manifest_content.contains("\"steps\""));
    }

    #[test]
    fn workflow_save_accepts_object_manifest_content() {
        let request: WorkflowSaveRequest = serde_json::from_value(serde_json::json!({
            "manifest_content": {
                "workflow": "save_from_object",
                "steps": [{"id": "a", "run": "apps list"}]
            },
            "overwrite": true
        }))
        .expect("deserialize save request");

        assert!(request.manifest_content.contains("\"workflow\": \"save_from_object\""));
    }
}
