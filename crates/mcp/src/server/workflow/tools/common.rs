//! Shared helpers for resolving runtime workflows from tool request payloads.

use crate::server::workflow::services::storage::{find_manifest_record, parse_manifest_content};
use anyhow::Result;
use oatty_types::workflow::RuntimeWorkflow;

pub fn resolve_runtime_workflow(
    workflow_identifier: Option<&str>,
    manifest_content: Option<&str>,
    format_hint: Option<&str>,
) -> Result<RuntimeWorkflow> {
    if let Some(workflow_identifier) = workflow_identifier {
        let Some(record) = find_manifest_record(workflow_identifier)? else {
            return Err(anyhow::anyhow!("workflow '{}' was not found", workflow_identifier));
        };
        return oatty_engine::workflow::document::runtime_workflow_from_definition(&record.definition);
    }

    let manifest_content = manifest_content.ok_or_else(|| anyhow::anyhow!("either workflow_id or manifest_content must be provided"))?;
    let (definition, _) = parse_manifest_content(manifest_content, format_hint)?;
    oatty_engine::workflow::document::runtime_workflow_from_definition(&definition)
}
