use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use bincode::config;
use heroku_types::ServiceId;

use crate::openapi::transform_openapi_to_links;
use crate::schema::{derive_commands_from_schema, generate_commands};

pub struct ManifestInput {
    pub input: PathBuf,
    pub service_id: ServiceId,
}
/// Writes the command manifest to a file.
///
/// This function reads a JSON schema from the input path, generates commands,
/// encodes them using bincode, and writes the output to the specified path.
///
/// # Arguments
///
/// * `input` - Path to the input JSON schema file.
/// * `output` - Path to write the bincode-encoded manifest.
///
/// # Errors
///
/// Returns an error if file reading, directory creation, command generation,
/// encoding, or writing fails.
pub fn write_manifest(inputs: Vec<ManifestInput>, output: PathBuf) -> Result<()> {
    let mut all_commands = Vec::new();
    for input in inputs {
        let ManifestInput { input, service_id } = input;
        let commands = if is_yaml(&input) {
            let text = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            let doc: serde_json::Value = serde_yaml::from_str(&text).context("parse yaml as json value")?;
            let transformed = transform_to_links_if_openapi(&doc)?;
            derive_commands_from_schema(&transformed, service_id)?
        } else {
            let schema = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            // If JSON OpenAPI, transform; else assume hyper-schema JSON
            if looks_like_openapi_json(&schema) {
                let doc: serde_json::Value = serde_json::from_str(&schema).context("parse json")?;
                let transformed = transform_to_links_if_openapi(&doc)?;
                derive_commands_from_schema(&transformed, service_id)?
            } else {
                generate_commands(&schema, service_id)?
            }
        };
        all_commands.extend(commands);
    }
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    all_commands.sort_by_key(|c| (c.group.clone(), c.name.clone()));
    let config = config::standard();
    let bytes = bincode::encode_to_vec(all_commands, config)?;
    fs::write(&output, &bytes)?;
    println!("wrote {} bytes to {}", bytes.len(), output.display());

    Ok(())
}

/// Writes the command manifest as JSON to a file.
///
/// This function mirrors `write_manifest` but serializes the generated commands
/// as JSON instead of bincode.
///
/// # Arguments
///
/// * `input` - Path to the input JSON schema file.
/// * `output` - Path to write the JSON manifest.
///
/// # Errors
///
/// Returns an error if file reading, directory creation, command generation,
/// encoding, or writing fails.
pub fn write_manifest_json(inputs: Vec<ManifestInput>, output: PathBuf) -> Result<()> {
    let mut all_commands = Vec::new();
    for input in inputs {
        let ManifestInput { input, service_id } = input;
        let commands = if is_yaml(&input) {
            let text = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            let doc: serde_json::Value = serde_yaml::from_str(&text).context("parse yaml as json value")?;
            let transformed = transform_to_links_if_openapi(&doc)?;
            derive_commands_from_schema(&transformed, service_id)?
        } else {
            let schema = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            if looks_like_openapi_json(&schema) {
                let doc: serde_json::Value = serde_json::from_str(&schema).context("parse json")?;
                let transformed = transform_to_links_if_openapi(&doc)?;
                derive_commands_from_schema(&transformed, service_id)?
            } else {
                generate_commands(&schema, service_id)?
            }
        };
        all_commands.extend(commands);
    }
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    all_commands.sort_by_key(|c| (c.group.clone(), c.name.clone()));
    let json = serde_json::to_vec_pretty(&all_commands).context("serialize commands to json")?;
    fs::write(&output, &json)?;
    println!("wrote {} bytes (json) to {}", json.len(), output.display());
    Ok(())
}

fn is_yaml(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(ext, "yaml" | "yml"),
        None => false,
    }
}

fn looks_like_openapi_json(s: &str) -> bool {
    // Lightweight detection to avoid parsing twice
    s.contains("\"openapi\"") || s.contains("\"swagger\"")
}

fn transform_to_links_if_openapi(doc: &serde_json::Value) -> Result<serde_json::Value> {
    // For now we support OpenAPI v3 only; v2 can be added later
    if doc.get("openapi").is_some() {
        transform_openapi_to_links(doc)
    } else {
        // Not OpenAPI; assume it's already hyper-schema-like
        Ok(doc.clone())
    }
}
