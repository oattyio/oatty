//! stdio helpers for rmcp-backed MCP clients.

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::config::McpServer;
use crate::logging::LogManager;
use crate::types::McpLogEntry;
use crate::types::{LogLevel, LogSource};
use std::sync::Arc;

/// Build a configured `tokio::process::Command` for stdio transport.
pub(crate) fn build_stdio_command(server: &McpServer) -> Result<Command> {
    let command = server
        .command
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing command for stdio transport"))?;

    let mut cmd = Command::new(command);
    if let Some(args) = &server.args {
        cmd.args(args);
    }
    if let Some(env) = &server.env {
        for (key, value) in env {
            cmd.env(key, value);
        }
    }
    if let Some(cwd) = &server.cwd {
        cmd.current_dir(cwd);
    }
    Ok(cmd)
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
