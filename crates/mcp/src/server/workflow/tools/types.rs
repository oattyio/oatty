//! Workflow MCP tool request payload types.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Deserializes a manifest document from JSON, coercing non-string values to a
/// pretty-printed JSON string representation.
fn deserialize_manifest_document<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let document = Value::deserialize(deserializer)?;
    match document {
        Value::String(text) => Ok(text),
        non_string_document => {
            serde_json::to_string_pretty(&non_string_document).map_err(serde::de::Error::custom)
        }
    }
}

/// Deserializes an optional manifest document, returning `None` when absent.
fn deserialize_optional_manifest_document<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let document = Option::<Value>::deserialize(deserializer)?;
    document
        .map(deserialize_manifest_document)
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
    #[schemars(
        description = "Inline workflow manifest document to persist. Provide exactly one of manifest_content or input_path.",
        with = "Option<serde_json::Value>"
    )]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Absolute path to a workflow manifest to persist. Provide exactly one of manifest_content or input_path.")]
    pub input_path: Option<String>,
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

/// Selects the one compact, read-only preview representation to return.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPreviewView {
    /// Describe defaults, missing required values, and optional input details.
    Inputs,
    /// Describe ordered steps, dependencies, and condition evaluation.
    Plan,
    /// Render step payloads and final output with candidate input values.
    Rendered,
}

/// Request one read-only workflow preview from an identifier or inline manifest.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowPreviewRequest {
    #[schemars(description = "Required preview representation: inputs, plan, or rendered.")]
    pub view: WorkflowPreviewView,
    #[schemars(description = "Optional existing workflow identifier. Provide exactly one of workflow_id or manifest_content.")]
    pub workflow_id: Option<String>,
    #[schemars(
        description = "Optional inline workflow manifest document. Provide exactly one of workflow_id or manifest_content.",
        with = "Option<serde_json::Value>"
    )]
    #[serde(default, deserialize_with = "deserialize_optional_manifest_document")]
    pub manifest_content: Option<String>,
    #[schemars(description = "Optional format hint for inline manifest: yaml or json.")]
    pub format: Option<String>,
    #[schemars(description = "Optional candidate input values keyed by input name.")]
    pub inputs: Option<HashMap<String, Value>>,
    #[schemars(description = "Include per-input detail rows. Valid only when view is inputs.")]
    pub include_details: Option<bool>,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct WorkflowPurgeHistoryRequest {
    #[schemars(description = "Optional workflow identifier filter.")]
    pub workflow_id: Option<String>,
    #[schemars(description = "Optional input keys used to select entries for purge.")]
    pub input_keys: Option<Vec<String>>,
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

        assert!(
            request
                .manifest_content
                .expect("manifest content")
                .contains("\"workflow\": \"save_from_object\"")
        );
    }
}
