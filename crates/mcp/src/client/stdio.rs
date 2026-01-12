//! stdio helpers for rmcp-backed MCP clients.

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::config::McpServer;
use crate::logging::LogManager;
use crate::types::McpLogEntry;
use crate::types::{LogLevel, LogSource};
use oatty_types::EnvVar;
use std::sync::Arc;

const DEFAULT_PARENT_ENVIRONMENT_ALLOWLIST: &[&str] = &["PATH", "SystemRoot", "SYSTEMROOT", "WINDIR", "COMSPEC"];

/// Build a configured `tokio::process::Command` for stdio transport.
///
/// The command inherits only the minimal parent environment required for
/// process discovery (`PATH` and Windows shell variables) and applies the
/// explicit environment defined in the MCP server configuration. This isolates
/// plugins from unrelated parent secrets while keeping configuration-driven
/// overrides intact.
pub(crate) fn build_stdio_command(server: &McpServer) -> Result<Command> {
    let command = server
        .command
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing command for stdio transport"))?;

    let mut cmd = Command::new(command);
    if let Some(args) = &server.args {
        cmd.args(args);
    }
    configure_isolated_environment(&mut cmd, server);
    if let Some(cwd) = &server.cwd {
        cmd.current_dir(cwd);
    }
    Ok(cmd)
}

/// Configure the child process environment to avoid leaking parent secrets while
/// preserving essential runtime variables and explicit MCP configuration.
fn configure_isolated_environment(command: &mut Command, server: &McpServer) {
    command.env_clear();

    for variable_name in DEFAULT_PARENT_ENVIRONMENT_ALLOWLIST {
        if let Some(value) = std::env::var_os(variable_name) {
            command.env(variable_name, value);
        }
    }

    for EnvVar { key, value, .. } in &server.env {
        command.env(key, value);
    }
}

/// Spawn a background task that forwards stderr lines to the log manager.
pub(crate) fn spawn_stderr_logger(plugin_name: String, log_manager: Arc<LogManager>, stderr: tokio::process::ChildStderr) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let entry = McpLogEntry::new(LogLevel::Info, line, LogSource::Stderr, plugin_name.clone());
            let _ = log_manager.add_log(&plugin_name, entry).await;
        }
    });
}
