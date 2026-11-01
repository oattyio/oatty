//! Prepare the local development environment for the Oatty workspace.
//!
//! Ported from `scripts/dev-setup.sh`. The setup verifies the Rust toolchain
//! and required components, creates configuration files from templates, and
//! runs the first workspace build, test, and lint passes. Toolchain problems
//! fail the setup; test and lint findings warn without failing, matching the
//! original script's behavior.

use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::context::project_root;

/// Check that a program is on `PATH` and report its version.
///
/// # Arguments
///
/// * `name` - The program name, for example `cargo`.
///
/// # Returns
///
/// Returns the program's `--version` first line.
fn program_version(name: &str) -> Result<String> {
    let output = Command::new(name)
        .arg("--version")
        .output()
        .with_context(|| format!("{name} is required; it was not found on PATH"))?;
    if !output.status.success() {
        bail!("{name} --version failed");
    }
    let version_text = String::from_utf8_lossy(&output.stdout).to_string();
    let version = version_text.lines().next().unwrap_or_default();
    anyhow::Ok(version.trim().to_string())
}

/// Read the toolchain channel requested by `rust-toolchain.toml`.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns the channel name, for example `stable`.
fn requested_toolchain_channel(root: &std::path::Path) -> Result<Option<String>> {
    let manifest = std::fs::read_to_string(root.join("rust-toolchain.toml")).unwrap_or_default();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if let Some(channel) = trimmed.strip_prefix("channel =") {
            return Ok(Some(channel.trim().trim_matches('"').to_string()));
        }
    }
    Ok(None)
}

/// Ensure the toolchain requested by `rust-toolchain.toml` is installed.
///
/// # Arguments
///
/// * `channel` - The channel name, for example `stable`.
fn ensure_toolchain(channel: &str) -> Result<()> {
    let installed = Command::new("rustup").args(["toolchain", "list"]).output()?;
    let listing = String::from_utf8_lossy(&installed.stdout);
    if listing.lines().any(|line| line.starts_with(channel)) {
        println!("Toolchain installed: {channel}");
        return Ok(());
    }
    println!("Installing toolchain: {channel}...");
    let outcome = Command::new("rustup").args(["toolchain", "install", channel]).status()?;
    if !outcome.success() {
        bail!("Toolchain installation failed: {channel}");
    }
    println!("Toolchain installed: {channel}");
    Ok(())
}

/// Ensure a rustup component is installed, installing it when missing.
///
/// # Arguments
///
/// * `component` - The component name, for example `clippy`.
fn ensure_component(component: &str) -> Result<()> {
    let installed = Command::new("rustup").args(["component", "list", "--installed"]).output()?;
    let listing = String::from_utf8_lossy(&installed.stdout);
    if listing.lines().any(|line| line.starts_with(component)) {
        println!("{component} is installed");
        return Ok(());
    }
    println!("Installing {component}...");
    let outcome = Command::new("rustup").args(["component", "add", component]).status()?;
    if !outcome.success() {
        bail!("Failed to install component: {component}");
    }
    Ok(())
}

/// Create the local `.env` file from the template when it is missing.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
fn ensure_local_environment(root: &std::path::Path) -> Result<()> {
    let env_path = root.join(".env");
    if env_path.is_file() {
        println!(".env file already exists");
        return Ok(());
    }
    std::fs::copy(root.join(".env.example"), &env_path).context("failed to create .env from .env.example")?;
    println!("Created .env from .env.example (informational only;");
    println!("nothing loads .env automatically).");
    println!("Configure API credentials via catalog header interpolation");
    println!("(e.g. an Authorization header using ${{env:...}} or ${{secret:...}})");
    Ok(())
}

/// Create the MCP configuration directory in the user's home when missing.
fn ensure_mcp_config_directory() -> Result<()> {
    let home = dirs_next::home_dir().context("cannot determine the home directory")?;
    let config_directory = home.join(".config/oatty");
    if config_directory.is_dir() {
        println!("MCP config directory exists");
        return Ok(());
    }
    std::fs::create_dir_all(&config_directory).with_context(|| format!("failed to create {}", config_directory.display()))?;
    println!("Created {}", config_directory.display());
    Ok(())
}

/// Run one workspace check command, returning whether it succeeded.
///
/// # Arguments
///
/// * `description` - The human-readable step name.
/// * `arguments` - The full cargo argument list, for example
///   `["build", "--workspace"]`.
fn run_workspace_check(description: &str, arguments: &[&str]) -> Result<bool> {
    println!("==> {description}");
    let outcome = Command::new("cargo").args(arguments).status()?;
    if outcome.success() {
        println!("{description} succeeded");
        return Ok(true);
    }
    println!("{description} failed");
    Ok(false)
}

/// Print the post-setup guidance shown by the original setup script.
fn print_next_steps() {
    println!();
    println!("Next steps:");
    println!("  1. Configure API credentials via catalog header");
    println!("     interpolation (e.g. an Authorization header)");
    println!("  2. Run the TUI: cargo run -p oatty-cli");
    println!("  3. Or run CLI commands: make run-cli ARGS=\"apps list\"");
    println!("Documentation: DEVELOPMENT.md, ARCHITECTURE.md, README.md");
}

/// Run the development environment setup.
///
/// Verify the Rust toolchain and required components, create configuration
/// files from templates, then build, test, and lint the workspace.
///
/// # Returns
///
/// Returns `Ok(())` when prerequisites and the workspace build pass.
///
/// # Errors
///
/// Returns an error when Rust/Cargo are missing, the configured toolchain
/// cannot be installed, or the workspace build fails. Test and clippy
/// failures warn without failing, as in the original setup script.
pub(crate) fn run() -> Result<()> {
    let root = project_root()?;
    println!("Oatty CLI (Rust) - Development Setup");
    println!();
    program_version("rustc").context("Rust is not installed. Please install from https://rustup.rs/")?;
    println!("Cargo found: {}", program_version("cargo")?);
    if let Some(channel) = requested_toolchain_channel(&root)? {
        ensure_toolchain(&channel)?;
    }
    ensure_component("clippy")?;
    ensure_component("rustfmt")?;
    ensure_local_environment(&root)?;
    ensure_mcp_config_directory()?;
    if !run_workspace_check("Building the workspace (this may take a few minutes...", &["build", "--workspace"])? {
        bail!("Workspace build failed");
    }
    run_workspace_check("Testing the workspace", &["test", "--workspace", "--quiet"])?;
    run_workspace_check("Running clippy", &["clippy", "--workspace", "--", "-D", "warnings"])?;
    print_next_steps();
    Ok(())
}
