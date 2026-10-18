//! Upstream MCP server management tools.
//!
//! These tools allow agent-driven configuration, listing, and lifecycle
//! management of the upstream MCP servers that provide command capabilities.

use crate::config::{load_config_from_path, load_raw_config_document_from_path};
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
    let path = oatty_registry::default_config_path();

    // Load current config (sync call)
    let mut doc = match load_raw_config_document_from_path(&path) {
        Ok(doc) => doc,
        Err(e) => {
            return Err(internal_error(format!("Failed to load config: {e}")));
        }
    };

    // Determine if this is an add or edit
    let existing = doc
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .and_then(|servers| servers.get(&request.name))
        .cloned();

    // Build or update the server config
    let mut server_config = existing.clone().unwrap_or(Value::Object(Map::new()));

    match request.transport.as_str() {
        "stdio" => {
            if let Some(cmd) = &request.command {
                server_config["command"] = serde_json::json!(cmd);
            }
            if let Some(args) = &request.args {
                server_config["args"] = serde_json::json!(args);
            }
            if let Some(obj) = server_config.as_object_mut() {
                obj.remove("baseUrl");
            }
        }
        "http" => {
            if let Some(url) = &request.base_url {
                server_config["baseUrl"] = serde_json::json!(url);
            }
            if let Some(obj) = server_config.as_object_mut() {
                obj.remove("command");
                obj.remove("args");
            }
        }
        other => {
            return Err(internal_error(format!("invalid transport type: {other}")));
        }
    }

    if let Some(enabled) = request.enabled {
        server_config["disabled"] = serde_json::json!(!enabled);
    }
    if let Some(tags) = &request.tags {
        server_config["tags"] = serde_json::json!(tags);
    }

    // Save to config
    let mcp_servers = doc.get("mcpServers").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let mut mcp_servers = mcp_servers;
    mcp_servers.insert(request.name.clone(), server_config.clone());
    doc["mcpServers"] = serde_json::json!(mcp_servers);

    // Write the JSON directly
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = serde_json::to_string_pretty(&doc).map_err(internal_error)?;
    std::fs::write(&path, content).map_err(internal_error)?;

    // Reload the engine config
    let new_config = load_config_from_path(&path).map_err(internal_error)?;
    plugin_engine.update_config(new_config).await.map_err(internal_error)?;

    // If connect=true, start the server
    let should_connect = request.connect.unwrap_or(true) && !server_config.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);
    if should_connect {
        let _ = plugin_engine.start_plugin(&request.name).await;
    }

    Ok(serde_json::json!({
        "name": request.name,
        "operation": if existing.is_some() { "updated" } else { "created" },
        "persisted": true,
        "runtime_applied": true,
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
