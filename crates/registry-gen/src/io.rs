use anyhow::{Context, Result};
use bincode::config;
use std::fs;
use std::path::PathBuf;

use crate::schema::generate_commands;

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
pub fn write_manifest(input: PathBuf, output: PathBuf) -> Result<()> {
    let schema = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
    let commands = generate_commands(&schema)?;
    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
        }
    }
    let config = config::standard();
    let bytes = bincode::encode_to_vec(commands, config)?;
    fs::write(&output, &bytes)?;
    println!("wrote {} bytes to {}", bytes.len(), output.display());
    Ok(())
}
