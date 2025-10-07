use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use heroku_registry_gen::{io::ManifestInput, write_manifest_json};
use heroku_types::ServiceId;
/// The entry point of the program.
///
/// The `main` function performs the following tasks:
/// 1. Retrieves the output directory path and constructs the path for the Heroku manifest file.
/// 2. Defines paths for required schema files (`heroku-schema.enhanced.json` and `data-schema.yaml`) and ensures they are watched for changes.
/// 3. Registers a directory of workflows to trigger rebuilds upon changes.
/// 4. Creates the output directory if it does not already exist.
/// 5. Writes a JSON manifest file containing input schemas and workflow information.
///
/// # Returns
/// * `Ok(())` on successful execution.
/// * An error of type `std::io::Error` or `env::VarError` if issues occur with file operations or environment variable handling.
///
/// # Details:
/// - Schemas:
///   - `heroku-schema.enhanced.json`: This schema provides configurations for the Core API service.
///   - `data-schema.yaml`: This schema provides configurations for the Data API service.
/// - The paths for schemas and workflows are constructed relative to the Cargo manifest directory.
///
/// # Environment Variables:
/// - `OUT_DIR`: Determines the output directory for the build artifacts.
/// - `CARGO_MANIFEST_DIR`: Points to the root directory of the current package.
///
/// # Steps in Workflow:
/// - All paths specified (`schemas` and `workflows`) are set to trigger rebuilds if files change (`cargo:rerun-if-changed`).
/// - Ensures that the output directory exists before proceeding with generating the manifest file.
///
/// # Manifest Generation:
/// - Invokes `write_manifest_json` to generate a JSON manifest file.
/// - The generated manifest links input schemas to their respective services via `ServiceId` enums, supporting workflows.
///
/// # Errors:
/// - This function will return an error if:
///   - Required environment variables (`OUT_DIR` or `CARGO_MANIFEST_DIR`) are missing or invalid.
///   - Reading or writing to files, or creating directories, fails.
///   - The `register_workflow_rerun` process encounters issues.
///
/// # Dependencies:
/// - Requires auxiliary functions `register_workflow_rerun` and `write_manifest_json`.
/// - Uses paths from standard library: `PathBuf` for file path manipulations.
///
/// # Example:
/// ```rust
/// fn main() -> Result<()> {
///     // Runs the main program flow to generate a manifest file.
/// }
/// ```
///
/// # Development Notes:
/// - Ensure the paths to schemas and workflows are updated correctly in case of project restructuring.
/// - This function is typically invoked during a build script process in a Rust project (`build.rs`).
fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_path = out_dir.join("heroku-manifest.json");

    // Source schema: top-level schemas/heroku-schema.json
    let heroku_schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("schemas")
        .join("heroku-schema.enhanced.json");
    let data_schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("schemas")
        .join("data-schema.yaml");

    let workflows_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("workflows");

    println!("cargo:rerun-if-changed={}", heroku_schema_path.display());
    println!("cargo:rerun-if-changed={}", data_schema_path.display());
    register_workflow_rerun(&workflows_dir)?;

    // Ensure output directory exists
    if let Some(parent) = manifest_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    write_manifest_json(
        vec![
            ManifestInput {
                input: heroku_schema_path,
                service_id: ServiceId::CoreApi,
            },
            ManifestInput {
                input: data_schema_path,
                service_id: ServiceId::DataApi,
            },
        ],
        Some(workflows_dir),
        manifest_path,
    )
}
/// Registers a Cargo build script directive to rerun the build script whenever any files in the specified workflows
/// directory or its subdirectories are modified.
///
/// This function is typically used in a `build.rs` script to dynamically register the `cargo:rerun-if-changed`
/// directive for all files in a specific directory tree. If the workflows directory doesn't exist, a warning
/// is emitted and the function exits without registering any directives.
///
/// # Arguments
///
/// * `workflows_dir` - A reference to the path of the workflows directory to monitor for changes.
///
/// # Returns
///
/// * `Result<()>` - Returns `Ok(())` if all files and subdirectories are successfully registered.
///   Returns an error if any I/O or path-related issues occur during processing.
///
/// # Behavior
///
/// * If `workflows_dir` does not exist, a warning is printed, and the function exits early without error.
/// * Registers `cargo:rerun-if-changed` for the root workflows directory and all files within it, recursively.
/// * Skips directories or files that encounter errors during processing.
///
/// # Errors
///
/// This function will return an error if:
///
/// 1. The directory cannot be read (e.g., due to insufficient permissions or if the path is invalid).
/// 2. Metadata for a file or directory entry cannot be accessed.
///
/// These errors will typically include an error message with context about the failing operation.
///
/// # Example
/// ```
/// use std::path::Path;
/// use std::fs;
/// use anyhow::Result;
///
/// fn main() -> Result<()> {
///     let workflows_dir = Path::new("path/to/workflows");
///     register_workflow_rerun(workflows_dir)?;
///     Ok(())
/// }
/// ```
///
/// # Notes
/// * The function prints `cargo:warning` if the workflows directory is missing, providing a helpful diagnostic
///   message during the build process.
/// * The recursion allows monitoring of nested directories within the specified workflows directory.
fn register_workflow_rerun(workflows_dir: &Path) -> Result<()> {
    if !workflows_dir.exists() {
        println!(
            "cargo:warning=workflows directory not found at {} (skipping bundling workflows)",
            workflows_dir.display()
        );
        return Ok(());
    }
    println!("cargo:rerun-if-changed={}", workflows_dir.display());
    for entry in fs::read_dir(workflows_dir).with_context(|| format!("read workflows directory {}", workflows_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            register_workflow_rerun(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}
