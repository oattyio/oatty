//! Auxiliary development and release engineering tasks for Oatty.
//!
//! This crate hosts project-specific developer tooling that is not part of the
//! shipped application: licensing material generation and verification, npm
//! and native release packaging, npm publication, release metadata checks, and
//! environment setup. The conventional `cargo xtask` alias (see
//! `.cargo/config.toml`) runs it.
//!
//! Each module ports a previously independent script (`scripts/*.py`,
//! `scripts/dev-setup.sh`) and preserves its fail-closed behavior, including
//! the unit-level contracts that were covered by the former Python test suites.

mod context;
mod licenses;
mod npm_package;
mod npm_publish;
mod release_metadata;
mod release_package;
mod setup;

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};

use crate::context::project_root;

/// The command-line interface for auxiliary Oatty development tasks.
#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Auxiliary development tasks for the Oatty workspace")]
struct Xtask {
    /// The auxiliary task to run.
    #[command(subcommand)]
    command: Command,
}

/// The supported auxiliary development tasks.
#[derive(Debug, Subcommand)]
enum Command {
    /// Prepare or verify the deterministic third-party licensing materials.
    Licenses {
        /// The licensing task to run.
        #[command(subcommand)]
        command: LicensesCommand,
    },
    /// Package npm payloads from verified legal files and unchanged executables.
    Npm {
        /// The npm packaging or publication task to run.
        #[command(subcommand)]
        command: NpmCommand,
    },
    /// Package the signed native release archive with verified legal materials.
    ReleasePackage {
        /// The release executable to package.
        #[arg(long)]
        binary: PathBuf,
        /// The Rust target triple the executable was built for.
        #[arg(long)]
        target: String,
        /// The release version embedded in the archive name.
        #[arg(long)]
        version: String,
        /// The directory to write the archive into.
        #[arg(long, default_value = "dist")]
        output: PathBuf,
    },
    /// Validate Cargo, npm, and release tag versions for a release.
    ReleaseMetadata,
    /// Set up the local development environment.
    Setup,
}

/// The licensing preparation and verification subcommands.
#[derive(Debug, Subcommand)]
enum LicensesCommand {
    /// Generate licensing materials and the staleness manifest.
    Prepare,
    /// Reject missing, stale, edited, or extra generated materials.
    Verify,
}

/// The npm packaging and publication subcommands.
#[derive(Debug, Subcommand)]
enum NpmCommand {
    /// Pack script-free npm packages from verified inputs.
    Package {
        /// Pack the launcher-only umbrella package instead of a native payload.
        #[arg(long, default_value_t = false)]
        umbrella: bool,
        /// The executable to embed in a native payload package.
        #[arg(long)]
        binary: Option<PathBuf>,
        /// The Rust target triple of the executable being packed.
        #[arg(long)]
        target: Option<String>,
        /// The directory to write the packed archive into.
        #[arg(long, default_value = "dist")]
        output: PathBuf,
    },
    /// Publish prepared and locally validated npm release artifacts.
    Publish {
        /// The directory containing the validated release tarballs.
        #[arg(long)]
        directory: PathBuf,
    },
}

/// Validate the mutually exclusive native and umbrella npm packaging modes.
///
/// # Arguments
///
/// * `umbrella` - Whether the launcher-only umbrella package was requested.
/// * `binary` - The executable path supplied for a native payload, if any.
/// * `target` - The Rust target triple supplied for a native payload, if any.
///
/// # Returns
///
/// Returns a `(binary, target)` pair for native payloads, or `None` for the
/// umbrella package.
///
/// # Errors
///
/// Returns an error when the argument combination matches neither packaging
/// mode, mirrors `--umbrella OR --binary PATH --target TARGET`.
fn validate_npm_package_mode(umbrella: bool, binary: Option<PathBuf>, target: Option<String>) -> Result<Option<(PathBuf, String)>> {
    if umbrella {
        if binary.is_some() || target.is_some() {
            return Err(anyhow!("--target is only valid with --binary"));
        }
        return Ok(None);
    }
    match (binary, target) {
        (Some(binary_path), Some(target_triple)) => Ok(Some((binary_path, target_triple))),
        (Some(_), None) | (None, Some(_)) => Err(anyhow!("use --umbrella OR --binary PATH --target TARGET")),
        (None, None) => Err(anyhow!("use --umbrella OR --binary PATH --target TARGET")),
    }
}

/// Execute the auxiliary task selected on the command line.
///
/// # Arguments
///
/// * `application` - The parsed command-line arguments.
///
/// # Returns
///
/// Returns `Ok(())` when the task completes, or an error describing the
/// failure. Every error path is fail-closed: nothing is published or written
/// unless the required verification steps pass first.
///
/// # Errors
///
/// Returns the error produced by the selected task implementation.
fn execute(application: Xtask) -> Result<()> {
    let root = project_root()?;
    match application.command {
        Command::Licenses {
            command: LicensesCommand::Prepare,
        } => licenses::prepare(&root),
        Command::Licenses {
            command: LicensesCommand::Verify,
        } => {
            licenses::verify(&root)?;
            eprintln!("Distribution licensing materials are current and complete.");
            Ok(())
        }
        Command::Npm {
            command:
                NpmCommand::Package {
                    umbrella,
                    binary,
                    target,
                    output,
                },
        } => match validate_npm_package_mode(umbrella, binary, target)? {
            Some((binary_path, target_triple)) => {
                let archive = npm_package::package_binary(&root, &binary_path, &target_triple, &output)?;
                println!("{}", archive.display());
                Ok(())
            }
            None => {
                let archive = npm_package::package_umbrella(&root, &output)?;
                println!("{}", archive.display());
                Ok(())
            }
        },
        Command::Npm {
            command: NpmCommand::Publish { directory },
        } => npm_publish::publish(&root, &directory),
        Command::ReleasePackage {
            binary,
            target,
            version,
            output,
        } => {
            let archive = release_package::package_release(&root, &binary, &target, &version, &output)?;
            println!("{}", archive.display());
            Ok(())
        }
        Command::ReleaseMetadata => release_metadata::verify_release_versions(),
        Command::Setup => setup::run(),
    }
}

fn main() -> Result<()> {
    execute(Xtask::parse())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn umbrella_package_mode_requires_no_binary_arguments() {
        let mode = validate_npm_package_mode(true, Some(PathBuf::from("binary")), Some("target".into()));
        assert_eq!(mode.unwrap_err().to_string(), "--target is only valid with --binary");
    }

    #[test]
    fn native_package_mode_requires_both_binary_and_target() {
        let mode = validate_npm_package_mode(false, Some(PathBuf::from("binary")), None);
        assert_eq!(mode.unwrap_err().to_string(), "use --umbrella OR --binary PATH --target TARGET");
        let mode = validate_npm_package_mode(false, None, None);
        assert_eq!(mode.unwrap_err().to_string(), "use --umbrella OR --binary PATH --target TARGET");
    }

    #[test]
    fn package_mode_selections_are_preserved() {
        let native = validate_npm_package_mode(false, Some(PathBuf::from("binary")), Some("target".into())).unwrap();
        assert_eq!(native, Some((PathBuf::from("binary"), "target".into())));
        assert_eq!(validate_npm_package_mode(true, None, None).unwrap(), None);
    }
}
