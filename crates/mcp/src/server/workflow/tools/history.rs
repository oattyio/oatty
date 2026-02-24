//! Workflow history management MCP tools.

use crate::server::workflow::errors::{execution_error, invalid_params_error};
use crate::server::workflow::services::history::purge_history;
use crate::server::workflow::tools::types::WorkflowPurgeHistoryRequest;
use rmcp::model::ErrorData;
use serde_json::Value;

/// Purge persisted workflow run history entries by workflow id and/or input keys.
pub fn purge_workflow_history(request: &WorkflowPurgeHistoryRequest) -> Result<Value, ErrorData> {
    if request.workflow_id.is_none() && request.input_keys.as_ref().is_none_or(Vec::is_empty) {
        return Err(invalid_params_error(
            "WORKFLOW_PURGE_FILTER_REQUIRED",
            "workflow_purge_history requires workflow_id or input_keys",
            serde_json::json!({}),
            "Provide workflow_id, input_keys, or both.",
        ));
    }

    let summary = purge_history(request.workflow_id.as_deref(), request.input_keys.as_deref().unwrap_or_default()).map_err(|error| {
        execution_error(
            "WORKFLOW_PURGE_HISTORY_FAILED",
            error.to_string(),
            serde_json::json!({
                "workflow_id": request.workflow_id,
                "input_keys": request.input_keys,
            }),
            true,
            "Retry purge after verifying workflow history directory permissions.",
        )
    })?;

    Ok(serde_json::json!({
        "purged": true,
        "workflow_id": request.workflow_id,
        "input_keys": request.input_keys,
        "removed_entries": summary.removed_entries,
        "removed_files": summary.removed_files,
    }))
}
