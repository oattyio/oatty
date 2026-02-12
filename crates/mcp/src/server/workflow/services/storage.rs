//! Runtime filesystem-backed workflow storage adapter.

use anyhow::{Context, Result, anyhow, bail};
use oatty_registry::default_workflows_path;
use oatty_types::workflow::WorkflowDefinition;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Supported manifest serialization formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowManifestFormat {
    Yaml,
    Json,
}

impl WorkflowManifestFormat {
    /// Resolve a format from an optional user hint.
    pub fn from_hint(format_hint: Option<&str>) -> Result<Self> {
        let Some(hint) = format_hint else {
            return Ok(Self::Yaml);
        };
        match hint.trim().to_ascii_lowercase().as_str() {
            "yaml" | "yml" => Ok(Self::Yaml),
            "json" => Ok(Self::Json),
            other => bail!("unsupported workflow format '{other}'"),
        }
    }

    /// Infer the format from a path extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("yaml") | Some("yml") => Some(Self::Yaml),
            Some("json") => Some(Self::Json),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Json => "json",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Json => "json",
        }
    }
}

/// Filesystem manifest entry with parsed definition and metadata.
#[derive(Debug, Clone)]
pub struct WorkflowManifestRecord {
    pub definition: WorkflowDefinition,
    pub path: PathBuf,
    pub format: WorkflowManifestFormat,
    pub content: String,
    pub version: String,
}

pub fn workflow_root_directory() -> PathBuf {
    default_workflows_path()
}

pub fn parse_manifest_content(content: &str, format_hint: Option<&str>) -> Result<(WorkflowDefinition, WorkflowManifestFormat)> {
    match format_hint {
        Some(hint) => {
            let format = WorkflowManifestFormat::from_hint(Some(hint))?;
            let definition = parse_definition_with_format(content, format)?;
            Ok((definition, format))
        }
        None => {
            if let Ok(definition) = parse_definition_with_format(content, WorkflowManifestFormat::Yaml) {
                return Ok((definition, WorkflowManifestFormat::Yaml));
            }
            let definition = parse_definition_with_format(content, WorkflowManifestFormat::Json)?;
            Ok((definition, WorkflowManifestFormat::Json))
        }
    }
}

pub fn serialize_definition(definition: &WorkflowDefinition, format: WorkflowManifestFormat) -> Result<String> {
    match format {
        WorkflowManifestFormat::Yaml => serde_yaml::to_string(definition).context("serialize workflow to yaml"),
        WorkflowManifestFormat::Json => serde_json::to_string_pretty(definition).context("serialize workflow to json"),
    }
}

pub fn list_manifest_records() -> Result<Vec<WorkflowManifestRecord>> {
    let root = workflow_root_directory();
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    collect_workflow_files(&root, &mut paths)?;
    paths.sort();

    let mut records = Vec::with_capacity(paths.len());
    for path in paths {
        let Some(format) = WorkflowManifestFormat::from_path(&path) else {
            continue;
        };
        let content = fs::read_to_string(&path).with_context(|| format!("read workflow {}", path.display()))?;
        let definition =
            parse_definition_with_format(&content, format).with_context(|| format!("parse workflow from {}", path.display()))?;
        let version = compute_version(&content);
        records.push(WorkflowManifestRecord {
            definition,
            path,
            format,
            content,
            version,
        });
    }

    records.sort_by(|left, right| left.definition.workflow.cmp(&right.definition.workflow));
    Ok(records)
}

pub fn find_manifest_record(workflow_identifier: &str) -> Result<Option<WorkflowManifestRecord>> {
    let records = list_manifest_records()?;
    Ok(records.into_iter().find(|record| record.definition.workflow == workflow_identifier))
}

pub fn write_manifest(workflow_identifier: &str, content: &str, format: WorkflowManifestFormat) -> Result<PathBuf> {
    let root = workflow_root_directory();
    fs::create_dir_all(&root).with_context(|| format!("create workflow directory {}", root.display()))?;

    let file_path = manifest_path_for_identifier(workflow_identifier, format)?;
    write_atomic(&file_path, content)?;
    Ok(file_path)
}

pub fn remove_manifest(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("remove workflow {}", path.display()))?;
    }
    Ok(())
}

pub fn manifest_path_for_identifier(workflow_identifier: &str, format: WorkflowManifestFormat) -> Result<PathBuf> {
    let sanitized_identifier = sanitize_workflow_identifier(workflow_identifier)?;
    let root = workflow_root_directory();
    Ok(root.join(format!("{sanitized_identifier}.{}", format.extension())))
}

pub fn sanitize_workflow_identifier(identifier: &str) -> Result<String> {
    let trimmed = identifier.trim();
    if trimmed.is_empty() {
        bail!("workflow identifier cannot be empty");
    }
    if trimmed
        .chars()
        .any(|character| !(character.is_ascii_alphanumeric() || character == '_' || character == '-'))
    {
        bail!("workflow identifier contains unsupported characters: '{trimmed}'");
    }
    Ok(trimmed.to_string())
}

pub fn compute_version(content: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn parse_definition_with_format(content: &str, format: WorkflowManifestFormat) -> Result<WorkflowDefinition> {
    let manifest_value = parse_manifest_value_with_format(content, format)?;
    validate_manifest_shape(&manifest_value)?;

    match format {
        WorkflowManifestFormat::Yaml => serde_yaml::from_str(content).map_err(|error| anyhow!(format_yaml_error(error))),
        WorkflowManifestFormat::Json => serde_json::from_str(content).map_err(|error| anyhow!(format_json_error(error))),
    }
}

fn parse_manifest_value_with_format(content: &str, format: WorkflowManifestFormat) -> Result<serde_json::Value> {
    match format {
        WorkflowManifestFormat::Yaml => serde_yaml::from_str(content).map_err(|error| anyhow!(format_yaml_error(error))),
        WorkflowManifestFormat::Json => serde_json::from_str(content).map_err(|error| anyhow!(format_json_error(error))),
    }
}

fn validate_manifest_shape(manifest_value: &serde_json::Value) -> Result<()> {
    let root_object = manifest_value
        .as_object()
        .ok_or_else(|| anyhow!("workflow manifest root must be an object"))?;

    validate_input_defaults_shape(root_object)?;
    validate_step_key_shape(root_object)?;

    Ok(())
}

fn validate_input_defaults_shape(root_object: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    let Some(inputs_value) = root_object.get("inputs") else {
        return Ok(());
    };
    let Some(inputs_object) = inputs_value.as_object() else {
        return Ok(());
    };

    for (input_name, definition_value) in inputs_object {
        let Some(definition_object) = definition_value.as_object() else {
            continue;
        };
        let Some(default_value) = definition_object.get("default") else {
            continue;
        };
        if default_value.is_null() {
            continue;
        }
        let Some(default_object) = default_value.as_object() else {
            bail!(
                "workflow input '{}.default' must be an object like '{{ from: literal, value: ... }}'",
                input_name
            );
        };
        if !default_object.contains_key("from") {
            bail!("workflow input '{}.default' must include a 'from' field", input_name);
        }
    }

    Ok(())
}

fn validate_step_key_shape(root_object: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    let Some(steps_value) = root_object.get("steps") else {
        return Ok(());
    };
    let Some(step_list) = steps_value.as_array() else {
        return Ok(());
    };

    for (step_index, step_value) in step_list.iter().enumerate() {
        let Some(step_object) = step_value.as_object() else {
            continue;
        };
        let step_identifier = step_object.get("id").and_then(serde_json::Value::as_str).unwrap_or("<missing-id>");

        if step_object.contains_key("flags") || step_object.contains_key("positional_args") {
            bail!(
                "workflow step '{}'(index {}) must place command arguments under 'with', not 'flags' or 'positional_args'",
                step_identifier,
                step_index
            );
        }
        if step_object.contains_key("condition") {
            bail!(
                "workflow step '{}'(index {}) uses unsupported key 'condition'; use 'if' or 'when'",
                step_identifier,
                step_index
            );
        }
    }

    Ok(())
}

fn format_yaml_error(error: serde_yaml::Error) -> String {
    if let Some(location) = error.location() {
        return format!(
            "parse yaml workflow at line {}, column {}: {}",
            location.line(),
            location.column(),
            error
        );
    }
    format!("parse yaml workflow: {error}")
}

fn format_json_error(error: serde_json::Error) -> String {
    format!("parse json workflow at line {}, column {}: {}", error.line(), error.column(), error)
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let temporary_path = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|extension| extension.to_str()).unwrap_or("tmp")
    ));
    fs::write(&temporary_path, content).with_context(|| format!("write temporary workflow {}", temporary_path.display()))?;
    fs::rename(&temporary_path, path).with_context(|| format!("persist workflow {} -> {}", temporary_path.display(), path.display()))?;
    Ok(())
}

fn collect_workflow_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read workflow directory {}", root.display()))? {
        let entry = entry.with_context(|| format!("walk workflow directory {}", root.display()))?;
        let path = entry.path();
        if entry.file_type().with_context(|| format!("inspect {}", path.display()))?.is_dir() {
            collect_workflow_files(&path, files)?;
        } else if WorkflowManifestFormat::from_path(&path).is_some() {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_hint_parses_expected_variants() {
        assert_eq!(
            WorkflowManifestFormat::from_hint(Some("yaml")).expect("yaml format"),
            WorkflowManifestFormat::Yaml
        );
        assert_eq!(
            WorkflowManifestFormat::from_hint(Some("json")).expect("json format"),
            WorkflowManifestFormat::Json
        );
        assert!(WorkflowManifestFormat::from_hint(Some("toml")).is_err());
    }

    #[test]
    fn sanitize_identifier_rejects_invalid_values() {
        assert!(sanitize_workflow_identifier("valid_id-1").is_ok());
        assert!(sanitize_workflow_identifier("").is_err());
        assert!(sanitize_workflow_identifier("../escape").is_err());
        assert!(sanitize_workflow_identifier("name with space").is_err());
    }

    #[test]
    fn compute_version_is_stable_for_identical_content() {
        let left = compute_version("workflow: demo\nsteps: []\n");
        let right = compute_version("workflow: demo\nsteps: []\n");
        let different = compute_version("workflow: demo_two\nsteps: []\n");
        assert_eq!(left, right);
        assert_ne!(left, different);
    }

    #[test]
    fn rejects_scalar_input_default_shape() {
        let content = r#"
workflow: demo
inputs:
  environment:
    type: string
    default: production
steps:
  - id: one
    run: apps list
"#;

        let error = parse_manifest_content(content, Some("yaml")).expect_err("expected scalar default rejection");
        assert!(error.to_string().contains("default' must be an object"));
    }

    #[test]
    fn rejects_step_flags_and_positional_args_keys() {
        let content = r#"
workflow: demo
steps:
  - id: one
    run: apps list
    positional_args:
      - app
"#;

        let error = parse_manifest_content(content, Some("yaml")).expect_err("expected unsupported step key rejection");
        assert!(error.to_string().contains("not 'flags' or 'positional_args'"));
    }
}
