//! Upstream MCP server management tools.
//!
//! These tools allow agent-driven configuration, listing, and lifecycle
//! management of the upstream MCP servers that provide command capabilities.

use crate::config::{
    ClearField, ConfigureMode, ErrorCategory, ErrorStage, McpConfigMutation, McpConfigMutationError, McpConfigMutationOutcome,
    McpServerPatch, ReloadPolicy, apply_config_mutation, default_config_path, load_raw_config_document_from_path,
};
use crate::plugin::PluginEngineError;
use crate::server::schemas::{McpServerConfigureRequest, McpServerSetRunningRequest};
use rmcp::model::ErrorData;
use serde_json::{Map, Value};
use std::sync::Arc;

/// Log only fixed field names and non-secret request metadata. User-supplied
/// strings can contain credentials regardless of which field carries them.
pub(crate) fn configure_request_log_projection(request: &McpServerConfigureRequest) -> Value {
    serde_json::json!({
        "command_present": request.command.is_some(),
        "argument_count": request.args.as_ref().map(Vec::len),
        "base_url_present": request.base_url.is_some(),
        "enabled": request.enabled,
        "connect": request.connect,
        "tag_count": request.tags.as_ref().map(Vec::len),
    })
}

fn configure_error(code: &'static str) -> ErrorData {
    ErrorData::internal_error(
        "MCP server configuration failed",
        Some(serde_json::json!({ "error_code": code, "next_step": "Review the server configuration and retry." })),
    )
}

/// Project lifecycle failures into stable categories.
///
/// Engine error displays can embed interpolated configuration values (for
/// example, a resolved URL in a connection error), so only the category is
/// exposed to callers.
fn plugin_engine_error_code(error: &PluginEngineError) -> &'static str {
    match error {
        PluginEngineError::PluginNotFound { .. } => "plugin_not_found",
        PluginEngineError::ClientManagerError(_) => "client_manager_failed",
        PluginEngineError::RegistryError(_) => "registry_failed",
        PluginEngineError::LifecycleError(_) => "lifecycle_failed",
        PluginEngineError::ConfigurationError { .. } => "configuration_failed",
        PluginEngineError::MaxRestartAttemptsExceeded { .. } => "max_restart_attempts_exceeded",
    }
}

fn set_running_error(code: &'static str) -> ErrorData {
    ErrorData::internal_error(
        "MCP server lifecycle change failed",
        Some(serde_json::json!({
            "error_code": code,
            "next_step": "Inspect server status and configuration, then retry; enable disabled servers through configuration before starting them."
        })),
    )
}

fn list_error(code: &'static str) -> ErrorData {
    ErrorData::internal_error(
        "MCP server listing failed",
        Some(serde_json::json!({ "error_code": code, "next_step": "Verify the MCP configuration file and retry." })),
    )
}

/// Log only fixed request metadata. Server names can contain arbitrary
/// user-supplied text but carry no diagnostic value here.
pub(crate) fn set_running_request_log_projection(request: &McpServerSetRunningRequest) -> Value {
    serde_json::json!({ "running": request.running })
}

fn mutation_error_code(error: &McpConfigMutationError) -> &'static str {
    match error {
        McpConfigMutationError::ReadFailed(_) => "read_failed",
        McpConfigMutationError::ParseFailed(_) | McpConfigMutationError::InvalidRootShape => "parse_failed",
        McpConfigMutationError::ServerNotFound(_) => "server_not_found",
        McpConfigMutationError::PatchConflict(_) => "patch_conflict",
        McpConfigMutationError::ValidationFailed(_) => "validation_failed",
        McpConfigMutationError::TokenizationFailed(_) => "tokenization_failed",
        McpConfigMutationError::SaveFailed(_) => "save_failed",
        McpConfigMutationError::RuntimeLoadFailed(_) => "runtime_load_failed",
        McpConfigMutationError::RuntimeApplyFailed(_) => "runtime_apply_failed",
    }
}

fn error_category_label(category: &ErrorCategory) -> &'static str {
    match category {
        ErrorCategory::ReadFailed => "read_failed",
        ErrorCategory::ParseFailed => "parse_failed",
        ErrorCategory::PatchConflict => "patch_conflict",
        ErrorCategory::ValidationFailed => "validation_failed",
        ErrorCategory::TokenizationFailed => "tokenization_failed",
        ErrorCategory::SaveFailed => "save_failed",
        ErrorCategory::RuntimeLoadFailed => "runtime_load_failed",
        ErrorCategory::RuntimeApplyFailed => "runtime_apply_failed",
        ErrorCategory::ConnectionFailed => "connection_failed",
    }
}

fn error_stage_label(stage: &ErrorStage) -> &'static str {
    match stage {
        ErrorStage::Read => "read",
        ErrorStage::Parse => "parse",
        ErrorStage::Patch => "patch",
        ErrorStage::Save => "save",
        ErrorStage::RuntimeLoad => "runtime_load",
        ErrorStage::RuntimeApply => "runtime_apply",
        ErrorStage::Connect => "connect",
    }
}

fn configure_outcome_json(outcome: McpConfigMutationOutcome) -> Value {
    serde_json::json!({
        "name": outcome.server_name,
        "operation": outcome.operation,
        "changed_fields": outcome.changed_fields,
        "persisted": outcome.persisted,
        "runtime_applied": outcome.runtime_applied,
        "connection_attempted": outcome.connection_attempted,
        "status": format!("{:?}", outcome.status).to_lowercase(),
        "error": outcome.error.map(|error| serde_json::json!({
            "error_code": error_category_label(&error.error_code),
            "stage": error_stage_label(&error.stage),
            "next_step": error.next_step,
        })),
    })
}

/// Configure an upstream MCP server.
pub(crate) async fn mcp_server_configure(
    plugin_engine: &Arc<crate::PluginEngine>,
    request: &McpServerConfigureRequest,
) -> Result<Value, ErrorData> {
    let path = default_config_path();

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
        _ => {
            return Err(configure_error("invalid_transport"));
        }
    }

    // The shared service derives add-vs-edit from the freshly read document
    // under the operation guard and resolves connect against the resulting
    // server's disabled state.
    let mutation = McpConfigMutation::ConfigureServer {
        name: request.name.clone(),
        patch,
        mode: ConfigureMode::Derive,
    };
    let policy = ReloadPolicy::ConfigureTarget {
        name: request.name.clone(),
        connect: request.connect.unwrap_or(true),
    };

    let outcome = apply_config_mutation(Arc::clone(plugin_engine), path, mutation, policy)
        .await
        .map_err(|e| configure_error(mutation_error_code(&e)))?;

    Ok(configure_outcome_json(outcome))
}

/// List all configured upstream MCP servers with runtime status.
pub(crate) async fn mcp_servers_list(plugin_engine: &Arc<crate::PluginEngine>, limit: Option<usize>) -> Result<Value, ErrorData> {
    let path = crate::config::default_config_path();

    let _guard = plugin_engine.acquire_operation_guard().await;

    let doc = load_raw_config_document_from_path(&path).map_err(|e| list_error(mutation_error_code(&e)))?;

    Ok(build_inventory(plugin_engine, &doc, limit).await)
}

async fn build_inventory(plugin_engine: &Arc<crate::PluginEngine>, doc: &Value, limit: Option<usize>) -> Value {
    let mcp_servers = doc.get("mcpServers").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let runtime_config = plugin_engine.config_snapshot().await;

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
        let runtime_server = runtime_config.mcp_servers.get(name);
        let cached_detail = plugin_engine.cached_plugin_detail(name).await;
        let runtime_available = runtime_server.is_some() && cached_detail.is_some();
        let runtime_status = cached_detail
            .as_ref()
            .filter(|_| runtime_available)
            .map(|detail| format!("{:?}", detail.status).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        let effective_disabled = runtime_server.filter(|_| runtime_available).map(|server| server.disabled);
        let status_detail = server_status_detail(runtime_server);
        let tool_count = if runtime_available {
            plugin_engine.plugin_tools(name).await.map(|tools| tools.len())
        } else {
            None
        };
        servers.push(serde_json::json!({
            "name": name,
            "transport": transport,
            "disabled": config.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false),
            "effective_disabled": effective_disabled,
            "status_detail": status_detail,
            "runtime_available": runtime_available,
            "status": runtime_status,
            "tool_count": tool_count,
            "masked_config": masked,
        }));
    }

    // Report the number of configured matches before applying the response
    // limit so callers can detect truncated output.
    let total = servers.len();
    if let Some(lim) = limit {
        servers.truncate(lim);
    }

    serde_json::json!({
        "servers": servers,
        "total": total,
    })
}

/// Start or stop a specific upstream MCP server.
pub(crate) async fn mcp_server_set_running(
    plugin_engine: &Arc<crate::PluginEngine>,
    request: &McpServerSetRunningRequest,
) -> Result<Value, ErrorData> {
    // Re-applying the current state is a successful no-op. In particular,
    // starting an already-running server must not reach the lifecycle state
    // machine, which would flip a healthy server to Starting/Error.
    if request.running
        && matches!(
            plugin_engine.get_plugin_status(&request.name).await,
            Ok(crate::types::PluginStatus::Running)
        )
    {
        return Ok(serde_json::json!({
            "name": request.name,
            "running": true,
            "status": "running",
            "already_in_state": true,
        }));
    }

    let result = if request.running {
        plugin_engine.start_plugin(&request.name).await
    } else {
        plugin_engine.stop_plugin(&request.name).await
    };
    result.map_err(|error| set_running_error(plugin_engine_error_code(&error)))?;

    let status = plugin_engine
        .get_plugin_status(&request.name)
        .await
        .map(|status| format!("{status:?}").to_lowercase())
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(serde_json::json!({
        "name": request.name,
        "running": request.running,
        "status": status,
    }))
}

/// Map runtime diagnostics to a stable, non-sensitive label.
///
/// `McpServer::err` currently records interpolation failures and can contain
/// configuration values, so the raw message is never exposed.
fn server_status_detail(server: Option<&crate::config::McpServer>) -> Option<&'static str> {
    server
        .and_then(|server| server.err.as_deref())
        .filter(|message| !message.trim().is_empty())
        .map(|_| "interpolation_failed")
}

/// Mask sensitive values in server configuration for display.
fn mask_server_config(config: &Map<String, Value>) -> Value {
    const MASKED: &str = "<masked>";
    let mut masked = Map::new();

    for field in ["command", "cwd", "baseUrl"] {
        if config.contains_key(field) {
            masked.insert(field.to_string(), Value::String(MASKED.to_string()));
        }
    }
    if let Some(args) = config.get("args").and_then(|v| v.as_array()) {
        masked.insert(
            "args".to_string(),
            Value::Array(args.iter().map(|_| Value::String(MASKED.to_string())).collect()),
        );
        masked.insert("argument_count".to_string(), serde_json::json!(args.len()));
    }
    if let Some(origin) = config
        .get("baseUrl")
        .and_then(Value::as_str)
        .and_then(|url| url::Url::parse(url).ok())
        .and_then(|url| match url.origin() {
            url::Origin::Tuple(..) => Some(url.origin().ascii_serialization()),
            _ => None,
        })
    {
        masked.insert("connection_origin".to_string(), Value::String(origin));
    }
    for field in ["env", "headers"] {
        if let Some(values) = config.get(field) {
            if let Some(entries) = values.as_object() {
                let masked_entries = entries.keys().map(|key| (key.clone(), Value::String(MASKED.to_string()))).collect();
                masked.insert(field.to_string(), Value::Object(masked_entries));
            } else if let Some(entries) = values.as_array() {
                let masked_entries = entries
                    .iter()
                    .filter_map(Value::as_object)
                    .map(|entry| {
                        let mut masked_entry = Map::new();
                        if let Some(key) = entry.get("key") {
                            masked_entry.insert("key".to_string(), key.clone());
                        }
                        if entry.contains_key("value") {
                            masked_entry.insert("value".to_string(), Value::String(MASKED.to_string()));
                        }
                        Value::Object(masked_entry)
                    })
                    .collect();
                masked.insert(field.to_string(), Value::Array(masked_entries));
            }
        }
    }
    if let Some(auth) = config.get("auth").and_then(Value::as_object) {
        let masked_auth = auth.keys().map(|key| (key.clone(), Value::String(MASKED.to_string()))).collect();
        masked.insert("auth".to_string(), Value::Object(masked_auth));
    }

    serde_json::Value::Object(masked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::gateway::{ClientGatewayEvent, McpClientGateway};
    use crate::config::{McpConfig, McpServer, SafeError};
    use crate::types::McpToolMetadata;
    use oatty_registry::CommandRegistry;
    use std::sync::Mutex;
    use std::time::Duration;

    #[test]
    fn inventory_mask_covers_map_and_array_credentials() {
        let config = serde_json::json!({
            "command": "secret-command", "args": ["secret-arg"], "cwd": "/secret/path",
            "baseUrl": "https://user:password@example.com:8443/secret?token=secret-query",
            "env": {"TOKEN": "secret-env"},
            "headers": [{"key": "Authorization", "value": "secret-header"}],
            "auth": {"scheme": "secret-scheme", "username": "secret-user", "password": "secret-password"}
        });
        let masked = mask_server_config(config.as_object().unwrap());
        let serialized = masked.to_string();
        for secret in [
            "secret-command",
            "secret-arg",
            "/secret/path",
            "secret-query",
            "secret-env",
            "secret-header",
            "secret-scheme",
            "secret-user",
            "secret-password",
        ] {
            assert!(!serialized.contains(secret), "masked inventory leaked a credential");
        }
        assert_eq!(masked["connection_origin"], "https://example.com:8443");
        assert_eq!(masked["argument_count"], 1);
        assert_eq!(masked["env"]["TOKEN"], "<masked>");
        assert_eq!(masked["headers"][0]["value"], "<masked>");
        assert_eq!(masked["auth"]["scheme"], "<masked>");
    }

    #[test]
    fn list_error_response_omits_raw_failure_values() {
        let source = McpConfigMutationError::ReadFailed(std::io::Error::new(std::io::ErrorKind::NotFound, "secret-directory/mcp.json"));
        let safe = list_error(mutation_error_code(&source));
        let serialized = serde_json::to_string(&safe).unwrap();
        assert_eq!(safe.data.as_ref().unwrap()["error_code"], "read_failed");
        assert!(!serialized.contains("secret-directory"));
    }

    #[test]
    fn every_plugin_engine_error_variant_has_a_snake_case_label() {
        for (error, expected) in [
            (
                crate::plugin::PluginEngineError::PluginNotFound {
                    name: "secret-detail".into(),
                },
                "plugin_not_found",
            ),
            (
                crate::plugin::PluginEngineError::ClientManagerError("secret-detail".into()),
                "client_manager_failed",
            ),
            (
                crate::plugin::PluginEngineError::RegistryError(crate::plugin::RegistryError::OperationFailed {
                    reason: "secret-detail".into(),
                }),
                "registry_failed",
            ),
            (
                crate::plugin::PluginEngineError::LifecycleError(crate::plugin::LifecycleError::OperationFailed {
                    reason: "secret-detail".into(),
                }),
                "lifecycle_failed",
            ),
            (
                crate::plugin::PluginEngineError::ConfigurationError {
                    message: "secret-detail".into(),
                },
                "configuration_failed",
            ),
            (
                crate::plugin::PluginEngineError::MaxRestartAttemptsExceeded {
                    name: "secret-detail".into(),
                },
                "max_restart_attempts_exceeded",
            ),
        ] {
            assert_eq!(plugin_engine_error_code(&error), expected);
            let serialized = serde_json::to_string(&set_running_error(plugin_engine_error_code(&error))).unwrap();
            assert!(!serialized.contains("secret-detail"), "set_running error leaked raw failure values");
        }
    }

    #[test]
    fn configure_adapter_derives_add_and_edit_under_the_guard() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        let seed = serde_json::json!({
            "mcpServers": {
                "existing": {"command": "node", "tags": ["old"]}
            }
        });
        std::fs::write(&path, serde_json::to_string(&seed).unwrap()).unwrap();
        let path_string = path.to_string_lossy().to_string();

        temp_env::with_var("MCP_CONFIG_PATH", Some(path_string.as_str()), || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async {
                let engine =
                    Arc::new(crate::PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

                for (name, tags) in [("existing", vec!["updated"]), ("fresh", vec!["new"])] {
                    let request = McpServerConfigureRequest {
                        name: name.into(),
                        transport: "stdio".into(),
                        command: Some("node".into()),
                        args: None,
                        base_url: None,
                        enabled: Some(true),
                        tags: Some(tags.into_iter().map(str::to_string).collect()),
                        connect: Some(false),
                    };
                    let outcome = mcp_server_configure(&engine, &request).await.unwrap();
                    assert_eq!(outcome["operation"], "configure");
                    assert_eq!(outcome["persisted"], true);
                    assert_eq!(outcome["connection_attempted"], false);
                }

                let persisted = load_raw_config_document_from_path(&path).unwrap();
                assert_eq!(persisted["mcpServers"]["existing"]["command"], "node");
                assert_eq!(persisted["mcpServers"]["existing"]["tags"], serde_json::json!(["updated"]));
                assert_eq!(persisted["mcpServers"]["fresh"]["command"], "node");
            });
        });
    }

    #[test]
    fn configure_error_response_omits_raw_failure_values() {
        let source = McpConfigMutationError::ValidationFailed("secret-command failed".into());
        let safe = configure_error(mutation_error_code(&source));
        let serialized = serde_json::to_string(&safe).unwrap();
        assert_eq!(safe.data.as_ref().unwrap()["error_code"], "validation_failed");
        assert!(!serialized.contains("secret-command"));
    }

    #[test]
    fn configure_outcome_uses_stable_error_wire_labels() {
        for (category, stage, expected_category, expected_stage, runtime_applied) in [
            (
                ErrorCategory::RuntimeApplyFailed,
                ErrorStage::RuntimeApply,
                "runtime_apply_failed",
                "runtime_apply",
                false,
            ),
            (
                ErrorCategory::ConnectionFailed,
                ErrorStage::Connect,
                "connection_failed",
                "connect",
                true,
            ),
        ] {
            let outcome = McpConfigMutationOutcome {
                server_name: "demo".into(),
                operation: "configure".into(),
                changed_fields: vec!["command".into()],
                persisted: true,
                runtime_applied,
                connection_attempted: true,
                status: if runtime_applied {
                    crate::types::PluginStatus::Error
                } else {
                    crate::types::PluginStatus::Running
                },
                error: Some(SafeError {
                    error_code: category,
                    stage,
                    next_step: "Retry safely".into(),
                }),
            };
            let serialized = configure_outcome_json(outcome);
            assert_eq!(serialized["persisted"], true);
            assert_eq!(serialized["connection_attempted"], true);
            assert_eq!(serialized["runtime_applied"], runtime_applied);
            assert_eq!(serialized["error"]["error_code"], expected_category);
            assert_eq!(serialized["error"]["stage"], expected_stage);
        }
    }

    #[test]
    fn every_configure_error_variant_has_a_snake_case_label() {
        for (category, expected) in [
            (ErrorCategory::ReadFailed, "read_failed"),
            (ErrorCategory::ParseFailed, "parse_failed"),
            (ErrorCategory::PatchConflict, "patch_conflict"),
            (ErrorCategory::ValidationFailed, "validation_failed"),
            (ErrorCategory::TokenizationFailed, "tokenization_failed"),
            (ErrorCategory::SaveFailed, "save_failed"),
            (ErrorCategory::RuntimeLoadFailed, "runtime_load_failed"),
            (ErrorCategory::RuntimeApplyFailed, "runtime_apply_failed"),
            (ErrorCategory::ConnectionFailed, "connection_failed"),
        ] {
            assert_eq!(error_category_label(&category), expected);
        }
        for (stage, expected) in [
            (ErrorStage::Read, "read"),
            (ErrorStage::Parse, "parse"),
            (ErrorStage::Patch, "patch"),
            (ErrorStage::Save, "save"),
            (ErrorStage::RuntimeLoad, "runtime_load"),
            (ErrorStage::RuntimeApply, "runtime_apply"),
            (ErrorStage::Connect, "connect"),
        ] {
            assert_eq!(error_stage_label(&stage), expected);
        }
    }

    #[tokio::test]
    async fn set_running_start_on_running_server_is_a_successful_no_op() {
        let mut config = McpConfig::default();
        config.mcp_servers.insert(
            "live".into(),
            McpServer {
                command: Some("/nonexistent/mcp-server".into()),
                ..Default::default()
            },
        );
        let engine = Arc::new(crate::PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());
        engine.prepare_registry().await.unwrap();

        engine
            .client_manager()
            .guarded_emit_lifecycle(ClientGatewayEvent::Started { name: "live".into() });
        let ack = engine.client_manager().emit_barrier().unwrap();
        McpClientGateway::wait_for_barrier_ack(ack, Duration::from_secs(1)).await.unwrap();
        assert_eq!(engine.get_plugin_status("live").await.unwrap(), crate::types::PluginStatus::Running);

        let detail_before = engine.get_plugin_detail("live").await.unwrap();
        let mut events = engine.client_manager().subscribe();

        let request = McpServerSetRunningRequest {
            name: "live".into(),
            running: true,
        };
        let response = mcp_server_set_running(&engine, &request).await.unwrap();
        assert_eq!(response["name"], "live");
        assert_eq!(response["running"], true);
        assert_eq!(response["status"], "running");
        assert_eq!(response["already_in_state"], true);

        // A no-op must not drive Start/StartFailed lifecycle transitions that
        // would poison the lifecycle state machine and health projection.
        assert!(matches!(events.try_recv(), Err(tokio::sync::broadcast::error::TryRecvError::Empty)));

        let detail_after = engine.get_plugin_detail("live").await.unwrap();
        assert_eq!(detail_after.status, crate::types::PluginStatus::Running);
        assert_eq!(detail_after.health.failure_count, detail_before.health.failure_count);
        assert_eq!(detail_after.health.last_error, detail_before.health.last_error);
    }

    #[tokio::test]
    async fn inventory_uses_runtime_disabled_and_cached_tool_counts() {
        let mut config = McpConfig::default();
        config.mcp_servers.insert(
            "forced-disabled".into(),
            McpServer {
                command: Some("node".into()),
                disabled: true,
                ..Default::default()
            },
        );
        config.mcp_servers.insert(
            "known-empty".into(),
            McpServer {
                command: Some("node".into()),
                ..Default::default()
            },
        );
        config.mcp_servers.insert(
            "live".into(),
            McpServer {
                command: Some("node".into()),
                ..Default::default()
            },
        );
        config.mcp_servers.insert(
            "running-no-snapshot".into(),
            McpServer {
                command: Some("node".into()),
                ..Default::default()
            },
        );
        config.mcp_servers.insert(
            "interpolation-failed".into(),
            McpServer {
                command: Some("node".into()),
                disabled: true,
                err: Some("Missing secret: secret-name - secret-detail".into()),
                ..Default::default()
            },
        );
        let mut updated_config = config.clone();
        updated_config.mcp_servers.get_mut("known-empty").unwrap().tags = Some(vec!["changed".into()]);
        let engine = Arc::new(crate::PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());
        engine.prepare_registry().await.unwrap();
        let tools = Arc::new(
            (0..3)
                .map(|index| McpToolMetadata {
                    name: format!("tool_{index}"),
                    title: None,
                    description: None,
                    input_schema: serde_json::json!({"type": "object"}),
                    output_schema: None,
                    annotations: None,
                    auth_summary: None,
                })
                .collect(),
        );
        engine.client_manager().guarded_emit_lifecycle(ClientGatewayEvent::ToolsUpdated {
            name: "live".into(),
            tools,
        });
        for name in ["forced-disabled", "known-empty"] {
            engine.client_manager().guarded_emit_lifecycle(ClientGatewayEvent::ToolsUpdated {
                name: name.into(),
                tools: Arc::new(Vec::new()),
            });
        }
        engine
            .client_manager()
            .guarded_emit_lifecycle(ClientGatewayEvent::Started { name: "live".into() });
        engine.client_manager().guarded_emit_lifecycle(ClientGatewayEvent::Started {
            name: "running-no-snapshot".into(),
        });
        let ack = engine.client_manager().emit_barrier().unwrap();
        McpClientGateway::wait_for_barrier_ack(ack, Duration::from_secs(1)).await.unwrap();
        engine.update_config(updated_config).await.unwrap();
        let persisted = serde_json::json!({"mcpServers": {
            "forced-disabled": {"command": "node", "disabled": false},
            "known-empty": {"command": "node", "disabled": false},
            "live": {"command": "node", "disabled": false},
            "running-no-snapshot": {"command": "node", "disabled": false},
            "interpolation-failed": {"command": "node", "disabled": false},
            "configured-only": {"command": "node", "disabled": false}
        }});
        let inventory = build_inventory(&engine, &persisted, None).await;
        let servers = inventory["servers"].as_array().unwrap();
        let forced = servers.iter().find(|item| item["name"] == "forced-disabled").unwrap();
        assert_eq!(forced["disabled"], false);
        assert_eq!(forced["effective_disabled"], true);
        assert_eq!(forced["tool_count"], 0);
        let live = servers.iter().find(|item| item["name"] == "live").unwrap();
        assert_eq!(live["status"], "running");
        assert_eq!(live["tool_count"], 3);
        let empty = servers.iter().find(|item| item["name"] == "known-empty").unwrap();
        assert_eq!(empty["tool_count"], 0);
        let unknown_tools = servers.iter().find(|item| item["name"] == "running-no-snapshot").unwrap();
        assert_eq!(unknown_tools["status"], "running");
        assert!(unknown_tools["tool_count"].is_null());
        let absent = servers.iter().find(|item| item["name"] == "configured-only").unwrap();
        assert_eq!(absent["runtime_available"], false);
        assert!(absent["effective_disabled"].is_null());
        assert!(absent["tool_count"].is_null());
        let interpolation_failed = servers.iter().find(|item| item["name"] == "interpolation-failed").unwrap();
        assert_eq!(interpolation_failed["status_detail"], "interpolation_failed");
        assert!(forced["status_detail"].is_null());
        let serialized = inventory.to_string();
        assert!(
            !serialized.contains("secret-detail"),
            "inventory leaked a raw interpolation diagnostic"
        );
        assert!(
            !serialized.contains("secret-name"),
            "inventory leaked a raw interpolation diagnostic"
        );

        let limited = build_inventory(&engine, &persisted, Some(2)).await;
        assert_eq!(limited["servers"].as_array().unwrap().len(), 2);
        assert_eq!(limited["total"], 6);
    }
}
