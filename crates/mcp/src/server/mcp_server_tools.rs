//! Upstream MCP server management tools.
//!
//! These tools allow agent-driven configuration, listing, and lifecycle
//! management of the upstream MCP servers that provide command capabilities.

use crate::config::{
    ClearField, ConfigureMode, McpConfigMutation, McpServerPatch, ReloadPolicy, apply_config_mutation, default_config_path,
    load_raw_config_document_from_path,
};
use crate::server::schemas::{McpServerConfigureRequest, McpServerSetRunningRequest};
use rmcp::model::ErrorData;
use serde_json::{Map, Value};
use std::sync::Arc;

fn internal_error<T: std::fmt::Display>(message: T) -> ErrorData {
    ErrorData::internal_error(message.to_string(), None)
}

/// Configure an upstream MCP server.
pub(crate) async fn mcp_server_configure(
    plugin_engine: &Arc<crate::PluginEngine>,
    request: &McpServerConfigureRequest,
) -> Result<Value, ErrorData> {
    let path = default_config_path();

    // Read the persisted document once to determine add-vs-edit mode and the
    // effective enabled state so `connect` honors "disabled servers remain
    // disconnected". The shared service re-reads under the operation guard.
    let doc = load_raw_config_document_from_path(&path).map_err(|e| internal_error(format!("Failed to load config: {e}")))?;
    let existing = doc
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .and_then(|servers| servers.get(&request.name))
        .cloned();

    let mode = if existing.is_some() {
        ConfigureMode::Edit
    } else {
        ConfigureMode::Add
    };

    let enabled = request.enabled.unwrap_or_else(|| {
        existing
            .as_ref()
            .and_then(|v| v.get("disabled"))
            .and_then(|v| v.as_bool())
            .map(|disabled| !disabled)
            .unwrap_or(true)
    });
    let connect = request.connect.unwrap_or(true) && enabled;

    let mut patch = McpServerPatch::default();
    patch.configuration.disabled = request.enabled.map(|enabled| !enabled);
    patch.configuration.tags = request.tags.clone();

    match request.transport.as_str() {
        "stdio" => {
            patch.configuration.command = request.command.clone();
            patch.configuration.args = request.args.clone();
            patch.clear_fields = vec![ClearField::BaseUrl, ClearField::Headers, ClearField::Auth];
        }
        "http" => {
            patch.configuration.base_url = request.base_url.clone();
            patch.clear_fields = vec![ClearField::Command, ClearField::Args, ClearField::Cwd, ClearField::Env];
        }
        other => {
            return Err(internal_error(format!("invalid transport type: {other}")));
        }
    }

    let mutation = McpConfigMutation::ConfigureServer {
        name: request.name.clone(),
        patch,
        mode,
    };
    let policy = ReloadPolicy::ConfigureTarget {
        name: request.name.clone(),
        connect,
    };

    let outcome = apply_config_mutation(Arc::clone(plugin_engine), path, mutation, policy)
        .await
        .map_err(|e| internal_error(e.to_string()))?;

    Ok(serde_json::json!({
        "name": outcome.server_name,
        "operation": outcome.operation,
        "changed_fields": outcome.changed_fields,
        "persisted": outcome.persisted,
        "runtime_applied": outcome.runtime_applied,
        "connection_attempted": outcome.connection_attempted,
        "status": format!("{:?}", outcome.status).to_lowercase(),
        "error": outcome.error.map(|e| serde_json::json!({
            "error_code": format!("{:?}", e.error_code).to_lowercase(),
            "stage": format!("{:?}", e.stage).to_lowercase(),
            "next_step": e.next_step,
        })),
    }))
}

/// List all configured upstream MCP servers with runtime status.
pub(crate) async fn mcp_servers_list(plugin_engine: &Arc<crate::PluginEngine>, limit: Option<usize>) -> Result<Value, ErrorData> {
    let path = oatty_registry::default_config_path();

    let doc = match load_raw_config_document_from_path(&path) {
        Ok(doc) => doc,
        Err(e) => {
            return Err(internal_error(format!("Failed to load config: {e}")));
        }
    };

    let mcp_servers = doc.get("mcpServers").and_then(|v| v.as_object()).cloned().unwrap_or_default();

    let mut servers = Vec::new();

    // Sort by name for deterministic output
    let mut sorted_names: Vec<&String> = mcp_servers.keys().collect();
    sorted_names.sort();

    for name in sorted_names {
        let config = &mcp_servers[name];
        let transport = if config.get("command").is_some() {
            "stdio"
        } else if config.get("baseUrl").is_some() {
            "http"
        } else {
            "unknown"
        };

        let masked = mask_server_config(config.as_object().unwrap_or(&Map::new()));
        // Look up runtime status from the engine
        let engine_status = plugin_engine.get_plugin_status(name).await;
        let runtime_status = match &engine_status {
            Ok(s) => format!("{s:?}").to_lowercase(),
            Err(_) => "unknown".to_string(),
        };
        let runtime_available = engine_status.is_ok();
        servers.push(serde_json::json!({
            "name": name,
            "transport": transport,
            "disabled": config.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false),
            "effective_disabled": config.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false),
            "runtime_available": runtime_available,
            "status": runtime_status,
            "tool_count": 0,
            "masked_config": masked,
        }));
    }

    if let Some(lim) = limit {
        servers.truncate(lim);
    }

    Ok(serde_json::json!({
        "servers": servers,
        "total": servers.len(),
    }))
}

/// Start or stop a specific upstream MCP server.
pub(crate) async fn mcp_server_set_running(
    plugin_engine: &Arc<crate::PluginEngine>,
    request: &McpServerSetRunningRequest,
) -> Result<Value, ErrorData> {
    if request.running {
        plugin_engine.start_plugin(&request.name).await.map_err(internal_error)?;
    } else {
        plugin_engine.stop_plugin(&request.name).await.map_err(internal_error)?;
    }

    let status = plugin_engine
        .get_plugin_status(&request.name)
        .await
        .map(|s| format!("{s:?}").to_lowercase())
        .unwrap_or_else(|e| format!("error: {e}"));

    Ok(serde_json::json!({
        "name": request.name,
        "running": request.running,
        "status": status,
    }))
}

/// Mask sensitive values in server configuration for display.
fn mask_server_config(config: &Map<String, Value>) -> Value {
    let mut masked = Map::new();

    if config.contains_key("command") {
        masked.insert("command".to_string(), serde_json::json!("***"));
    }
    if let Some(args) = config.get("args").and_then(|v| v.as_array()) {
        masked.insert("argument_count".to_string(), serde_json::json!(args.len()));
    }
    if config.contains_key("baseUrl") {
        masked.insert("baseUrl".to_string(), serde_json::json!("***"));
    }
    if let Some(headers) = config.get("headers").and_then(|v| v.as_array()) {
        let mut masked_headers = Vec::new();
        for header in headers {
            if let Some(obj) = header.as_object() {
                let mut masked_header = Map::new();
                if let Some(key) = obj.get("key") {
                    masked_header.insert("key".to_string(), key.clone());
                }
                if let Some(value) = obj.get("value") {
                    if let Some(val_str) = value.as_str() {
                        if val_str.len() > 8 {
                            masked_header.insert("value".to_string(), serde_json::json!("***"));
                        } else {
                            masked_header.insert("value".to_string(), value.clone());
                        }
                    } else {
                        masked_header.insert("value".to_string(), value.clone());
                    }
                }
                masked_headers.push(serde_json::Value::Object(masked_header));
            }
        }
        masked.insert("headers".to_string(), serde_json::json!(masked_headers));
    }

    serde_json::Value::Object(masked)
}
