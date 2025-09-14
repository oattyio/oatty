//! Simple stdio MCP client example using `rmcp`.
//!
//! Usage:
//!   cargo run -p heroku-mcp --example stdio_client -- <command> [args...] [--list] [--tool <name>] [--json '{...}']
//!
//! Examples:
//!   # Just list tools exposed by the stdio server
//!   cargo run -p heroku-mcp --example stdio_client -- uvx mcp-server-git --list
//!
//!   # Call a tool with JSON arguments
//!   cargo run -p heroku-mcp --example stdio_client -- uvx mcp-server-git --tool git_status --json '{"repo_path":"."}'

use anyhow::{Context, Result};
use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt as _,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::env;
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: stdio_client <command> [args...] [--list] [--tool <name>] [--json '{...}']");
        std::process::exit(2);
    }

    let (cmd, cmd_args, flags) = split_args(&args);
    let list_only = flags.list;
    let tool_name = flags.tool_name;
    let tool_args = flags.tool_args;

    // Build and spawn the stdio transport
    let mut command = Command::new(&cmd);
    if !cmd_args.is_empty() {
        command.args(&cmd_args);
    }
    let transport = TokioChildProcess::new(command.configure(|_cmd| {}))
        .with_context(|| format!("failed to spawn command: {}", cmd))?;

    // Connect the client service (no-op service type `()`)
    let running = ().serve(transport).await?;
    let peer = running.peer().clone();

    // List tools
    let tools = peer.list_all_tools().await.context("failed to list tools")?;
    println!("Discovered {} tool(s):", tools.len());
    for t in &tools {
        println!("- {}: {}", t.name, t.description.clone().unwrap_or_default());
    }

    if list_only {
        return Ok(());
    }

    // Optionally call a tool
    if let Some(name) = tool_name {
        let arguments = tool_args
            .and_then(|s| serde_json::from_str::<JsonValue>(&s).ok())
            .and_then(|v| v.as_object().cloned());

        let result = peer
            .call_tool(CallToolRequestParam { name, arguments })
            .await
            .context("tool invocation failed")?;
        println!("\nTool result:\n{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

struct Flags {
    list: bool,
    tool_name: Option<String>,
    tool_args: Option<String>,
}

fn split_args(args: &[String]) -> (String, Vec<String>, Flags) {
    // Everything until the first flag (starting with '--') is command and args
    let mut idx = 0;
    while idx < args.len() && !args[idx].starts_with("--") {
        idx += 1;
    }
    let (head, tail) = args.split_at(idx);
    let cmd = head.first().cloned().unwrap();
    let cmd_args = head.iter().skip(1).cloned().collect::<Vec<_>>();

    // Parse flags: --list, --tool <name>, --json <json>
    let mut list = false;
    let mut tool_name = None;
    let mut tool_args = None;
    let mut i = 0;
    while i < tail.len() {
        match tail[i].as_str() {
            "--list" => {
                list = true;
                i += 1;
            }
            "--tool" => {
                if i + 1 < tail.len() {
                    tool_name = Some(tail[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--json" => {
                if i + 1 < tail.len() {
                    tool_args = Some(tail[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    (
        cmd,
        cmd_args,
        Flags {
            list,
            tool_name,
            tool_args,
        },
    )
}
