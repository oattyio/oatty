//! Registry synchronization after workflow mutations.

use anyhow::{Context, Result};
use oatty_registry::{CommandRegistry, workflows::load_runtime_workflows};
use std::sync::{Arc, Mutex};

/// Synchronization summary emitted after mutation operations.
#[derive(Debug, Clone, Copy)]
pub struct WorkflowSyncSummary {
    pub workflow_count: usize,
    pub synthetic_command_count: usize,
}

/// Reload filesystem-backed workflows and refresh the in-memory registry snapshot.
pub fn synchronize_runtime_workflows(command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<WorkflowSyncSummary> {
    let workflows = load_runtime_workflows().context("reload workflows from runtime storage")?;
    let workflow_count = workflows.len();

    let mut registry = command_registry
        .lock()
        .map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;
    registry.workflows = workflows;

    // Synthetic workflow commands are reserved for a follow-up implementation.
    let synthetic_command_count = 0usize;

    Ok(WorkflowSyncSummary {
        workflow_count,
        synthetic_command_count,
    })
}
