#![feature(associated_type_defaults)]
//! # Heroku CLI TUI Library
//!
//! This library provides a terminal user interface (TUI) for the Heroku CLI.
//! It implements a modern, interactive command-line interface using the Ratatui
//! framework with support for command execution, real-time logs, and interactive
//! command building.
//!
//! ## Key Features
//!
//! - Interactive command palette with autocomplete
//! - Real-time command execution and log streaming
//! - Command browser with inline help
//! - Tabular data display with pagination
//! - Focus management and keyboard navigation
//! - Asynchronous command execution
//!
//! ## Architecture
//!
//! The TUI follows a component-based architecture where each UI element
//! (palette, logs, browser, table, help) is implemented as a separate
//! component that can handle events and render itself.

mod app;
mod cmd;
mod ui;

use anyhow::Result;
use heroku_mcp::PluginEngine;
use std::sync::{Arc, Mutex};

// Runtime moved to ui::runtime

/// Runs the main TUI application loop.
///
/// This function initializes the terminal interface, sets up all UI components,
/// and runs the main event loop that handles user input, command execution,
/// and UI rendering.
///
/// # Arguments
///
/// * `registry` - The Heroku command registry containing all available commands
///
/// # Returns
///
/// Returns `Ok(())` if the application exits cleanly, or an error if there's
/// a terminal setup or runtime issue.
///
/// # Errors
///
/// This function can return errors for:
/// - Terminal setup failures (raw mode, alternate screen)
/// - Component initialization failures
/// - Event loop runtime errors
///
/// # Example
///
/// ```no_run
/// use heroku_registry::Registry;
/// use heroku_tui::run;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let registry = Registry::new();
///     run(registry).await
/// }
/// ```
pub async fn run(registry: Arc<Mutex<heroku_registry::Registry>>, plugin_engine: Arc<PluginEngine>) -> Result<()> {
    ui::runtime::run_app(registry, plugin_engine).await
}
