//! Workflow run history persistence and retention operations.

use crate::server::workflow::services::storage::workflow_root_directory;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Durable execution history entry for workflow runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowHistoryEntry {
    pub workflow_id: String,
    pub run_id: String,
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub inputs: Value,
}

/// Summary describing removed workflow history records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowHistoryPurgeSummary {
    pub removed_entries: usize,
    pub removed_files: usize,
}

/// Append a run history entry for a workflow.
pub fn append_history_entry(entry: &WorkflowHistoryEntry) -> Result<()> {
    let history_directory = history_directory();
    fs::create_dir_all(&history_directory).with_context(|| format!("create history directory {}", history_directory.display()))?;

    let file_path = history_file_path(&entry.workflow_id);
    let serialized = serde_json::to_string(entry).context("serialize workflow history entry")?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .with_context(|| format!("open history file {}", file_path.display()))?;
    file.write_all(serialized.as_bytes())
        .with_context(|| format!("write history file {}", file_path.display()))?;
    file.write_all(b"\n")
        .with_context(|| format!("write history newline {}", file_path.display()))?;

    Ok(())
}

/// Purge workflow history entries by workflow id and/or referenced input keys.
pub fn purge_history(workflow_id: Option<&str>, input_keys: &[String]) -> Result<WorkflowHistoryPurgeSummary> {
    let history_directory = history_directory();
    if !history_directory.exists() {
        return Ok(WorkflowHistoryPurgeSummary {
            removed_entries: 0,
            removed_files: 0,
        });
    }

    let mut removed_entries = 0usize;
    let mut removed_files = 0usize;

    for file_path in list_history_files(workflow_id)? {
        let content = fs::read_to_string(&file_path).with_context(|| format!("read history file {}", file_path.display()))?;
        if content.trim().is_empty() {
            fs::remove_file(&file_path).with_context(|| format!("remove history file {}", file_path.display()))?;
            removed_files += 1;
            continue;
        }

        let mut retained = Vec::new();
        let mut removed_for_file = 0usize;

        for line in content.lines().filter(|line| !line.trim().is_empty()) {
            let parsed = serde_json::from_str::<WorkflowHistoryEntry>(line)
                .with_context(|| format!("parse history entry from {}", file_path.display()))?;
            let should_remove = if input_keys.is_empty() {
                true
            } else {
                input_keys.iter().any(|input_key| parsed.inputs.get(input_key).is_some())
            };
            if should_remove {
                removed_for_file += 1;
            } else {
                retained.push(line.to_string());
            }
        }

        if removed_for_file == 0 {
            continue;
        }

        removed_entries += removed_for_file;
        if retained.is_empty() {
            fs::remove_file(&file_path).with_context(|| format!("remove history file {}", file_path.display()))?;
            removed_files += 1;
        } else {
            fs::write(&file_path, format!("{}\n", retained.join("\n")))
                .with_context(|| format!("rewrite history file {}", file_path.display()))?;
        }
    }

    Ok(WorkflowHistoryPurgeSummary {
        removed_entries,
        removed_files,
    })
}

fn history_directory() -> PathBuf {
    workflow_root_directory().join(".history")
}

fn history_file_path(workflow_id: &str) -> PathBuf {
    history_directory().join(format!("{workflow_id}.jsonl"))
}

fn list_history_files(workflow_id: Option<&str>) -> Result<Vec<PathBuf>> {
    let history_directory = history_directory();
    if let Some(identifier) = workflow_id {
        return Ok(vec![history_file_path(identifier)]);
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(&history_directory).with_context(|| format!("read history directory {}", history_directory.display()))? {
        let entry = entry.with_context(|| format!("walk history directory {}", history_directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("jsonl") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(workflow_id: &str, run_id: &str, inputs: Value) -> WorkflowHistoryEntry {
        WorkflowHistoryEntry {
            workflow_id: workflow_id.to_string(),
            run_id: run_id.to_string(),
            status: "succeeded".to_string(),
            timestamp: Utc::now(),
            inputs,
        }
    }

    #[test]
    fn append_and_purge_history_by_workflow_identifier() {
        let temp_directory = tempfile::tempdir().expect("create temp dir");
        temp_env::with_var(
            "REGISTRY_WORKFLOWS_PATH",
            Some(temp_directory.path().to_string_lossy().to_string()),
            || {
                append_history_entry(&sample_entry("demo", "run-1", serde_json::json!({ "app": "demo" })))
                    .expect("append first history entry");
                append_history_entry(&sample_entry("demo", "run-2", serde_json::json!({ "app": "demo2" })))
                    .expect("append second history entry");

                let summary = purge_history(Some("demo"), &[]).expect("purge history");
                assert_eq!(summary.removed_entries, 2);
                assert_eq!(summary.removed_files, 1);
            },
        );
    }

    #[test]
    fn purge_history_by_input_key_preserves_non_matching_entries() {
        let temp_directory = tempfile::tempdir().expect("create temp dir");
        temp_env::with_var(
            "REGISTRY_WORKFLOWS_PATH",
            Some(temp_directory.path().to_string_lossy().to_string()),
            || {
                append_history_entry(&sample_entry("demo", "run-1", serde_json::json!({ "app": "demo", "region": "us" })))
                    .expect("append matching entry");
                append_history_entry(&sample_entry("demo", "run-2", serde_json::json!({ "team": "platform" })))
                    .expect("append non-matching entry");

                let summary = purge_history(Some("demo"), &["app".to_string()]).expect("purge by key");
                assert_eq!(summary.removed_entries, 1);
                assert_eq!(summary.removed_files, 0);
            },
        );
    }
}
