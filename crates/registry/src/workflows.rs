//! Filesystem-backed workflow loading utilities.
//!
//! This module is the canonical runtime workflow source for the registry.
//! Workflow files are loaded from a directory on disk and parsed as either YAML
//! or JSON based on file extension.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use oatty_types::workflow::WorkflowDefinition;

use crate::config::default_workflows_path;

/// Load workflow definitions from the default runtime workflows directory.
///
/// The default directory is resolved by [`default_workflows_path`], which is
/// derived from registry configuration path conventions and optional
/// environment overrides.
pub fn load_runtime_workflows() -> Result<Vec<WorkflowDefinition>> {
    load_workflows_from_directory(&default_workflows_path())
}

/// Load workflow definitions from the provided directory path.
///
/// Returns an empty vector when the directory does not exist.
pub fn load_workflows_from_directory(root: &Path) -> Result<Vec<WorkflowDefinition>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_workflow_files(root, &mut files)?;
    files.sort();

    let mut workflows = Vec::with_capacity(files.len());
    for path in files {
        let content = fs::read_to_string(&path).with_context(|| format!("read workflow {}", path.display()))?;
        let definition = parse_workflow_definition(&path, &content)?;
        workflows.push(definition);
    }

    workflows.sort_by(|left, right| left.workflow.cmp(&right.workflow));
    workflows.dedup_by(|left, right| left.workflow == right.workflow);
    Ok(workflows)
}

fn parse_workflow_definition(path: &Path, content: &str) -> Result<WorkflowDefinition> {
    if is_json_path(path) {
        serde_json::from_str::<WorkflowDefinition>(content).with_context(|| format!("parse workflow json {}", path.display()))
    } else {
        serde_yaml::from_str::<WorkflowDefinition>(content).with_context(|| format!("parse workflow yaml {}", path.display()))
    }
}

fn collect_workflow_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read workflow dir {}", root.display()))? {
        let entry = entry.with_context(|| format!("walk {}", root.display()))?;
        let path = entry.path();
        if entry.file_type().with_context(|| format!("inspect {}", path.display()))?.is_dir() {
            collect_workflow_files(&path, files)?;
        } else if should_ingest_workflow(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn should_ingest_workflow(path: &Path) -> bool {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => matches!(extension, "yaml" | "yml" | "json"),
        None => false,
    }
}

fn is_json_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension) if extension.eq_ignore_ascii_case("json")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temporary_dir() -> PathBuf {
        let mut directory = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time moved backwards")
            .as_nanos();
        directory.push(format!("workflow_loader_tests_{nanos}"));
        fs::create_dir_all(&directory).expect("create temp directory");
        directory
    }

    #[test]
    fn load_workflows_reads_yaml_and_json_recursively() -> Result<()> {
        let root = create_temporary_dir();
        let nested = root.join("nested");
        fs::create_dir_all(&nested)?;

        let yaml_path = root.join("alpha.yaml");
        let mut yaml_file = fs::File::create(&yaml_path)?;
        writeln!(yaml_file, "workflow: alpha\nsteps: []")?;

        let json_path = nested.join("beta.json");
        let mut json_file = fs::File::create(&json_path)?;
        write!(json_file, "{}", serde_json::json!({ "workflow": "beta", "steps": [] }))?;

        let workflows = load_workflows_from_directory(&root)?;
        assert_eq!(workflows.len(), 2);
        assert_eq!(workflows[0].workflow, "alpha");
        assert_eq!(workflows[1].workflow, "beta");
        Ok(())
    }
}
