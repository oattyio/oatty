//! Plugin engine implementation.

use crate::client::{ClientGatewayEvent, McpClientGateway};
use crate::config::McpConfig;
use crate::logging::{AuditEntry, AuditResult, LogManager};
use crate::plugin::{LifecycleManager, PluginRegistry, RegistryError};
use crate::types::{AuthStatus, McpToolMetadata, PluginDetail, PluginStatus, PluginToolSummary};
use oatty_registry::{CommandRegistry, CommandSpec};
use oatty_types::{CommandFlag, ExecOutcome, McpCommandSpec, PositionalArgument};
use oatty_util::resolve_output_schema;
use serde_json::Value;
use std::sync::Mutex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{
    sync::{Mutex as TokioMutex, RwLock, broadcast::error::RecvError as TokioRecvError},
    task::JoinHandle,
};

/// Plugin engine that orchestrates all MCP plugin operations.
#[derive(Debug)]
pub struct PluginEngine {
    /// Client manager for handling MCP connections.
    client_manager: McpClientGateway,

    /// Log manager for plugin logs.
    log_manager: Arc<LogManager>,

    /// Plugin registry for metadata as an interior, thread
    /// safe mutable reference that's lockable across await points
    plugin_registry: Arc<TokioMutex<Option<PluginRegistry>>>,

    /// Cache of tool metadata discovered per plugin.
    tool_cache: Arc<TokioMutex<HashMap<String, Arc<Vec<McpToolMetadata>>>>>,

    /// Synthetic command specifications synthesized from MCP tools.
    synthetic_specs: Arc<TokioMutex<HashMap<String, Arc<[CommandSpec]>>>>,

    /// Lifecycle manager for plugin lifecycle.
    lifecycle_manager: LifecycleManager,

    /// Configuration guarded for concurrent updates.
    config: RwLock<McpConfig>,

    /// Shared vec containing all commands
    command_registry: Arc<Mutex<CommandRegistry>>,

    /// Background task that keeps the registry in sync with client status events.
    status_listener: TokioMutex<Option<JoinHandle<()>>>,
}

impl PluginEngine {
    /// Create a new plugin engine.
    pub fn new(config: McpConfig, command_registry: Arc<Mutex<CommandRegistry>>) -> anyhow::Result<Self> {
        let client_manager = McpClientGateway::new(config.clone())?;
        let log_manager = Arc::new(LogManager::new()?);
        let lifecycle_manager = LifecycleManager::new();

        Ok(Self {
            client_manager,
            log_manager,
            plugin_registry: Arc::new(TokioMutex::new(None)),
            tool_cache: Arc::new(TokioMutex::new(HashMap::new())),
            synthetic_specs: Arc::new(TokioMutex::new(HashMap::new())),
            lifecycle_manager,
            config: RwLock::new(config),
            command_registry,
            status_listener: TokioMutex::new(None),
        })
    }

    pub async fn prepare_registry(&self) -> Result<PluginRegistry, PluginEngineError> {
        let mut maybe_registry = self.plugin_registry.lock().await;
        if maybe_registry.is_some() {
            return Ok(maybe_registry.clone().unwrap());
        }

        let mut registry = PluginRegistry::new();

        let config_snapshot = self.config.read().await.clone();

        for (name, server) in &config_snapshot.mcp_servers {
            let mut plugin_detail = PluginDetail::new(
                name.clone(),
                if server.is_stdio() {
                    server.command.as_ref().unwrap().clone()
                } else {
                    server.base_url.as_ref().unwrap().to_string()
                },
                server.args.clone().map(|a| a.join(" ")),
            );
            plugin_detail.transport_type = server.transport_type().to_string();
            plugin_detail.tags = server.tags.clone().unwrap_or_default();
            plugin_detail.enabled = !server.is_disabled();
            plugin_detail.env = if server.is_stdio() {
                server.env.clone().unwrap_or_default()
            } else {
                server.headers.clone().unwrap_or_default()
            };

            registry.register_plugin(plugin_detail)?;
            self.lifecycle_manager.register_plugin(name.clone()).await;
        }

        self.ensure_status_listener(registry.clone()).await;
        maybe_registry.replace(registry);
        Ok(maybe_registry.clone().unwrap())
    }

    /// Start the plugin engine.
    pub async fn start(&self) -> Result<(), PluginEngineError> {
        // Start the client manager
        self.client_manager
            .start()
            .await
            .map_err(|e| PluginEngineError::ClientManagerError(e.to_string()))?;

        tracing::info!("Plugin engine started");
        Ok(())
    }

    /// Ensure the background status listener task is running so plugin status
    /// updates from the client manager are reflected in the registry.
    async fn ensure_status_listener(&self, mut registry: PluginRegistry) {
        let mut guard = self.status_listener.lock().await;
        if guard.is_some() {
            return;
        }

        let mut receiver = self.client_manager.subscribe();
        let tool_cache = Arc::clone(&self.tool_cache);
        let synthetic_specs = Arc::clone(&self.synthetic_specs);
        let command_registry = Arc::clone(&self.command_registry);

        let handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(ClientGatewayEvent::ToolsUpdated { name, tools }) => {
                        if let Err(update_err) = registry.set_plugin_tool_count(&name, tools.len()) {
                            tracing::warn!(plugin = %name, error = %update_err, "Failed to update tool count");
                        }

                        {
                            let mut cache = tool_cache.lock().await;
                            if tools.is_empty() {
                                cache.remove(&name);
                            } else {
                                cache.insert(name.clone(), Arc::clone(&tools));
                            }
                        }

                        let auth_message = registry
                            .get_plugin(&name)
                            .and_then(|detail| PluginEngine::format_auth_summary(detail.auth_status));

                        let synthesized = PluginEngine::synthesize_mcp_specs(&name, tools.as_ref(), auth_message.as_deref());
                        {
                            let mut synthetic_specs_lock = synthetic_specs.lock().await;
                            if let Ok(mut registry_lock) = command_registry.lock() {
                                if synthesized.is_empty() {
                                    if let Some(specs) = synthetic_specs_lock.remove(&name).as_ref() {
                                        let ids = specs.iter().map(|s| s.canonical_id()).collect();
                                        registry_lock.remove_commands(ids);
                                    }
                                } else {
                                    let syn_arc: Arc<[CommandSpec]> = Arc::from(synthesized);
                                    registry_lock.insert_commands(syn_arc.clone().as_ref());
                                    synthetic_specs_lock.insert(name.clone(), syn_arc);
                                }
                            }
                        }
                    }
                    Ok(event) => {
                        let (name, status) = match event {
                            ClientGatewayEvent::Starting { name } => (name, PluginStatus::Starting),
                            ClientGatewayEvent::Started { name } => (name, PluginStatus::Running),
                            ClientGatewayEvent::StartFailed { name, error } => {
                                tracing::warn!(plugin = %name, error = %error, "Plugin failed to start");
                                (name, PluginStatus::Error)
                            }
                            ClientGatewayEvent::Stopping { name } => (name, PluginStatus::Stopping),
                            ClientGatewayEvent::Stopped { name } => (name, PluginStatus::Stopped),
                            ClientGatewayEvent::ToolsUpdated { .. } => unreachable!("tools updates handled above"),
                        };
                        if let Err(update_err) = registry.set_plugin_status(&name, status) {
                            tracing::warn!(plugin = %name, error = %update_err, "Failed to update registry status");
                        }
                    }
                    Err(TokioRecvError::Closed) => break,
                    Err(TokioRecvError::Lagged(skipped)) => {
                        tracing::warn!("Plugin status listener lagged by {} events", skipped);
                    }
                }
            }
        });

        *guard = Some(handle);
    }

    /// Stop the plugin engine.
    pub async fn stop(&self) -> Result<(), PluginEngineError> {
        if let Some(handle) = self.status_listener.lock().await.take() {
            handle.abort();
            match handle.await {
                Err(join_error) if !join_error.is_cancelled() => {
                    tracing::warn!("Status listener task ended with error: {}", join_error);
                }
                _ => {}
            }
        }

        self.client_manager
            .stop()
            .await
            .map_err(|e| PluginEngineError::ClientManagerError(e.to_string()))?;

        tracing::info!("Plugin engine stopped");
        Ok(())
    }

    /// Start a plugin.
    pub async fn start_plugin(&self, name: &str) -> Result<(), PluginEngineError> {
        let Ok(mut registry) = self.prepare_registry().await else {
            return Err(PluginEngineError::RegistryError(RegistryError::OperationFailed {
                reason: "registry unavailable".into(),
            }));
        };
        // Check if plugin is registered
        if !registry.is_registered(name) {
            return Err(PluginEngineError::PluginNotFound { name: name.to_string() });
        }

        // Start the plugin using lifecycle management
        let start_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                async move { client_manager.start_plugin(&name).await.map_err(|e| e.to_string()) }
            }
        };

        self.lifecycle_manager.start_plugin(name, start_fn).await?;

        // Update registry
        registry.set_plugin_status(name, PluginStatus::Running)?;

        tracing::info!("Started plugin: {}", name);
        Ok(())
    }

    /// Stop a plugin.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), PluginEngineError> {
        let Ok(mut registry) = self.prepare_registry().await else {
            return Err(PluginEngineError::RegistryError(RegistryError::OperationFailed {
                reason: "registry unavailable".into(),
            }));
        };
        // Check if plugin is registered
        if !registry.is_registered(name) {
            return Err(PluginEngineError::PluginNotFound { name: name.to_string() });
        }

        // Stop the plugin using lifecycle management
        let stop_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                async move { client_manager.stop_plugin(&name).await.map_err(|e| e.to_string()) }
            }
        };

        self.lifecycle_manager.stop_plugin(name, stop_fn).await?;

        // Update registry
        registry.set_plugin_status(name, PluginStatus::Stopped)?;

        tracing::info!("Stopped plugin: {}", name);
        Ok(())
    }

    /// Restart a plugin.
    pub async fn restart_plugin(&self, name: &str) -> Result<(), PluginEngineError> {
        let Ok(mut registry) = self.prepare_registry().await else {
            return Err(PluginEngineError::RegistryError(RegistryError::OperationFailed {
                reason: "registry unavailable".into(),
            }));
        };
        // Check if plugin is registered
        if !registry.is_registered(name) {
            return Err(PluginEngineError::PluginNotFound { name: name.to_string() });
        }

        // Check if we can restart
        if !self.lifecycle_manager.can_restart(name).await {
            return Err(PluginEngineError::MaxRestartAttemptsExceeded { name: name.to_string() });
        }
        // Restart the plugin using lifecycle management
        let stop_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                async move { client_manager.stop_plugin(&name).await.map_err(|e| e.to_string()) }
            }
        };

        let start_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                async move { client_manager.start_plugin(&name).await.map_err(|e| e.to_string()) }
            }
        };

        self.lifecycle_manager.restart_plugin(name, stop_fn, start_fn).await?;

        // Update registry
        registry.set_plugin_status(name, PluginStatus::Running)?;

        tracing::info!("Restarted plugin: {}", name);
        Ok(())
    }

    /// Get plugin details.
    pub async fn get_plugin_detail(&self, name: &str) -> Result<PluginDetail, PluginEngineError> {
        let Ok(registry) = self.prepare_registry().await else {
            return Err(PluginEngineError::RegistryError(RegistryError::OperationFailed {
                reason: "registry unavailable".into(),
            }));
        };
        let mut plugin_detail = registry
            .get_plugin(name)
            .ok_or_else(|| PluginEngineError::PluginNotFound { name: name.to_string() })?;

        let status = registry.get_plugin_status(name).unwrap_or(PluginStatus::Stopped);
        let health = self.client_manager.get_plugin_health(name).await.unwrap_or_default();
        let logs = self.log_manager.get_recent_logs(name, 100).await;
        let tool_summaries = {
            let cache = self.tool_cache.lock().await;
            cache
                .get(name)
                .map(|tools| tools.iter().map(PluginEngine::summarize_tool).collect())
                .unwrap_or_default()
        };

        plugin_detail.status = status;
        plugin_detail.health = health;
        plugin_detail.logs = logs;
        plugin_detail.tools = tool_summaries;

        Ok(plugin_detail)
    }

    /// List all plugins.
    pub async fn list_plugins(&self) -> Vec<PluginDetail> {
        let Ok(registry) = self.prepare_registry().await else {
            return vec![];
        };
        let mut plugins = Vec::new();

        let names = registry.get_plugin_names();
        for name in names {
            if let Ok(detail) = self.get_plugin_detail(&name).await {
                plugins.push(detail);
            }
        }

        plugins
    }

    /// Get plugin status.
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, PluginEngineError> {
        let Ok(registry) = self.prepare_registry().await else {
            return Err(PluginEngineError::RegistryError(RegistryError::OperationFailed {
                reason: "registry unavailable".into(),
            }));
        };
        registry
            .get_plugin_status(name)
            .ok_or_else(|| PluginEngineError::PluginNotFound { name: name.to_string() })
    }

    /// Check if a plugin is running.
    pub async fn is_plugin_running(&self, name: &str) -> bool {
        matches!(self.get_plugin_status(name).await, Ok(PluginStatus::Running))
    }

    /// Get the client manager.
    pub fn client_manager(&self) -> &McpClientGateway {
        &self.client_manager
    }

    /// Get the log manager.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }

    /// Get the plugin registry.
    pub fn registry(&self) -> &TokioMutex<Option<PluginRegistry>> {
        self.plugin_registry.as_ref()
    }

    /// Get the lifecycle manager.
    pub fn lifecycle_manager(&self) -> &LifecycleManager {
        &self.lifecycle_manager
    }

    /// Update configuration.
    pub async fn update_config(&self, config: McpConfig) -> Result<(), PluginEngineError> {
        let Ok(mut registry) = self.prepare_registry().await else {
            return Err(PluginEngineError::RegistryError(RegistryError::OperationFailed {
                reason: "registry unavailable".into(),
            }));
        };
        // Capture which plugins should be restarted after reload.
        let mut restart_candidates = Vec::new();
        for name in registry.get_plugin_names() {
            let was_running = registry
                .get_plugin_status(&name)
                .map(|status| status == PluginStatus::Running)
                .unwrap_or(false);
            if was_running {
                restart_candidates.push(name.clone());
            }
            if let Err(e) = self.stop_plugin(&name).await {
                tracing::warn!("Failed to stop plugin {} during config update: {}", name, e);
            }
        }

        {
            let mut cache = self.tool_cache.lock().await;
            cache.clear();
        }

        {
            let mut overlay = self.synthetic_specs.lock().await;
            overlay.clear();
        }

        // Update client manager configuration
        self.client_manager
            .update_config(config.clone())
            .await
            .map_err(|e| PluginEngineError::ClientManagerError(e.to_string()))?;

        {
            let mut guard = self.config.write().await;
            *guard = config.clone();
        }

        // Clear and rebuild registry
        registry.clear()?;

        for (name, server) in &config.mcp_servers {
            let mut plugin_detail = PluginDetail::new(
                name.clone(),
                if server.is_stdio() {
                    server.command.as_ref().unwrap().clone()
                } else {
                    server.base_url.as_ref().unwrap().to_string()
                },
                server.args.clone().map(|a| a.join(" ")),
            );
            plugin_detail.transport_type = server.transport_type().to_string();
            plugin_detail.tags = server.tags.clone().unwrap_or_default();
            plugin_detail.enabled = !server.is_disabled();

            registry.register_plugin(plugin_detail)?;
            self.lifecycle_manager.register_plugin(name.clone()).await;
        }

        // Restart plugins that were previously running and still exist + enabled.
        for name in restart_candidates {
            let Some(entry) = config.mcp_servers.get(&name) else {
                continue;
            };
            if entry.is_disabled() {
                continue;
            }
            if let Err(error) = self.start_plugin(&name).await {
                tracing::warn!("Failed to restart plugin {} after config update: {}", name, error);
            }
        }

        tracing::info!("Plugin engine configuration updated");
        Ok(())
    }

    /// Return the current tool metadata snapshot for the requested plugin, if known.
    pub async fn plugin_tools(&self, name: &str) -> Option<Arc<Vec<McpToolMetadata>>> {
        let cache = self.tool_cache.lock().await;
        cache.get(name).cloned()
    }

    /// Execute an MCP-backed command specification with the provided arguments.
    pub async fn execute_tool(
        &self,
        spec: &CommandSpec,
        arguments: &serde_json::Map<String, Value>,
        request_id: u64,
    ) -> Result<ExecOutcome, PluginEngineError> {
        let mcp = spec.mcp().ok_or_else(|| PluginEngineError::ConfigurationError {
            message: format!("command '{}' is not MCP-backed", spec.name),
        })?;

        let plugin_name = mcp.plugin_name.clone();
        let tool_name = mcp.tool_name.clone();

        let call_result = self.client_manager.call_tool(&plugin_name, &tool_name, arguments).await;

        let (is_error, payload) = match call_result {
            Ok(result) => {
                let (is_error, payload) = Self::normalize_tool_result(result);
                let audit_result = if is_error { AuditResult::Failure } else { AuditResult::Success };
                let entry = AuditEntry::tool_invoke(plugin_name.clone(), tool_name.clone(), audit_result);
                let _ = self.log_manager.log_audit(entry).await;
                (is_error, payload)
            }
            Err(err) => {
                let mut entry = AuditEntry::tool_invoke(plugin_name.clone(), tool_name.clone(), AuditResult::Failure);
                entry.metadata.insert("error".to_string(), Value::String(err.to_string()));
                let _ = self.log_manager.log_audit(entry).await;
                return Err(PluginEngineError::ClientManagerError(err.to_string()));
            }
        };

        let mut log = format!(
            "MCP {}:{} {}",
            mcp.plugin_name,
            mcp.tool_name,
            if is_error { "failed" } else { "succeeded" }
        );

        if !payload.is_null()
            && let Ok(pretty) = serde_json::to_string_pretty(&payload)
        {
            log.push('\n');
            log.push_str(&pretty);
        }

        Ok(ExecOutcome::Mcp {
            log_entry: log,
            payload,
            request_id,
        })
    }

    /// Convert MCP tool metadata into synthetic CLI command specifications.
    ///
    /// When every tool name shares the same prefix (up to the first underscore), the prefix is
    /// treated as a common command group. The prefix is removed from each command name so that
    /// the resulting command identifiers focus on the actionable portion of the tool name. If
    /// the tools do not share a common prefix, the synthetic commands are grouped under the MCP
    /// server name provided by configuration.
    fn synthesize_mcp_specs(plugin_name: &str, tools: &[McpToolMetadata], auth_message: Option<&str>) -> Vec<CommandSpec> {
        if tools.is_empty() {
            return Vec::new();
        }

        let shared_group = Self::determine_shared_group(tools);

        let mut specs = Vec::with_capacity(tools.len());

        for tool in tools {
            let group = shared_group.clone().unwrap_or_else(|| plugin_name.to_string());
            let command_name = if let Some(shared_prefix) = &shared_group {
                let remainder = Self::trim_shared_prefix(&tool.name, shared_prefix);
                let formatted = Self::format_command_segments(&remainder);
                if formatted.is_empty() { shared_prefix.clone() } else { formatted }
            } else {
                let formatted = Self::format_command_segments(&tool.name);
                if formatted.is_empty() { tool.name.clone() } else { formatted }
            };

            let summary = Self::build_summary(tool, auth_message);
            let (positionals, flags) = Self::convert_schema_to_inputs(tool);
            let output_schema = tool
                .output_schema
                .as_ref()
                .and_then(|schema| resolve_output_schema(Some(schema), schema));
            let render_hint = tool.annotations.as_ref().and_then(|annotations| match annotations {
                Value::Object(map) => map
                    .get("render_hint")
                    .or_else(|| map.get("renderHint"))
                    .and_then(|value| value.as_str().map(String::from)),
                _ => None,
            });

            let mcp_spec = McpCommandSpec {
                plugin_name: plugin_name.to_string(),
                tool_name: tool.name.clone(),
                auth_summary: auth_message.map(String::from),
                output_schema,
                render_hint,
            };

            specs.push(CommandSpec::new_mcp(group, command_name, summary, positionals, flags, mcp_spec));
        }

        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    /// Determine whether all tools share the same prefix up to the first underscore.
    fn determine_shared_group(tools: &[McpToolMetadata]) -> Option<String> {
        let (first_prefix, _) = tools.iter().find_map(|tool| tool.name.split_once('_'))?;
        if first_prefix.is_empty() {
            return None;
        }

        tools
            .iter()
            .all(|tool| tool.name.split_once('_').map(|(prefix, _)| prefix) == Some(first_prefix))
            .then(|| first_prefix.to_string())
    }

    /// Remove the shared prefix (and following underscore) from a tool name.
    fn trim_shared_prefix(tool_name: &str, shared_prefix: &str) -> String {
        tool_name
            .strip_prefix(shared_prefix)
            .map(|remainder| remainder.strip_prefix('_').unwrap_or(remainder))
            .unwrap_or("")
            .to_string()
    }

    /// Convert underscore separated tool identifiers into colon delimited command names.
    fn format_command_segments(raw_name: &str) -> String {
        let segments: Vec<&str> = raw_name.split('_').filter(|segment| !segment.is_empty()).collect();
        if segments.is_empty() { String::new() } else { segments.join(":") }
    }

    fn summarize_tool(tool: &McpToolMetadata) -> PluginToolSummary {
        PluginToolSummary {
            name: tool.name.clone(),
            title: tool.title.clone(),
            description: tool.description.clone(),
            auth_summary: tool.auth_summary.clone(),
        }
    }

    fn build_summary(tool: &McpToolMetadata, auth_message: Option<&str>) -> String {
        let body = tool.description.as_deref().or(tool.title.as_deref()).unwrap_or(tool.name.as_str());

        match auth_message {
            Some(message) if !message.is_empty() => format!("{message} — {body}"),
            _ => body.to_string(),
        }
    }

    fn convert_schema_to_inputs(tool: &McpToolMetadata) -> (Vec<PositionalArgument>, Vec<CommandFlag>) {
        let mut positional_args = Vec::new();
        let mut flags = Vec::new();

        let schema = &tool.input_schema;
        let schema_object = schema.as_object();

        let required_fields: HashSet<String> = schema_object
            .and_then(|map| map.get("required"))
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect::<HashSet<String>>()
            })
            .unwrap_or_default();

        let properties = schema_object
            .and_then(|map| map.get("properties"))
            .and_then(|value| value.as_object());

        let mut fields = Vec::new();

        if let Some(props) = properties {
            for (name, definition) in props {
                let required = required_fields.contains(name);
                fields.push(Self::parse_field(name, definition, required));
            }
        }

        if fields.is_empty() {
            fields.push(FieldDescriptor::json_blob(
                "payload".to_string(),
                false,
                Some("Structured inputs collapsed into JSON.".to_string()),
            ));
        }

        for field in fields {
            match field.kind {
                FieldKind::Boolean => {
                    flags.push(field.into_flag());
                }
                FieldKind::String | FieldKind::Json | FieldKind::Number => {
                    if field.required {
                        positional_args.push(field.into_positional());
                    } else {
                        flags.push(field.into_flag());
                    }
                }
            }
        }

        (positional_args, flags)
    }

    fn parse_field(name: &str, definition: &Value, required: bool) -> FieldDescriptor {
        let mut descriptor = FieldDescriptor::new(name.to_string(), required);

        if let Some(description) = definition.get("description").and_then(|value| value.as_str()) {
            descriptor.description = Some(description.to_string());
        }

        if let Some(default_value) = definition.get("default") {
            descriptor.default_value = match default_value {
                Value::String(value) => Some(value.clone()),
                Value::Number(value) => Some(value.to_string()),
                Value::Bool(value) => Some(value.to_string()),
                _ => None,
            };
        }

        if let Some(enum_values) = definition.get("enum").and_then(|value| value.as_array()) {
            descriptor.enum_values = enum_values.iter().filter_map(|value| value.as_str().map(str::to_string)).collect();
        }

        let field_type = definition.get("type");
        descriptor.kind = match field_type {
            Some(Value::String(value)) if value == "boolean" => FieldKind::Boolean,
            Some(Value::String(value)) if value == "string" => FieldKind::String,
            Some(Value::String(value)) if value == "integer" => FieldKind::Number,
            Some(Value::Array(values)) if values.iter().any(|entry| entry == "boolean") => FieldKind::Boolean,
            Some(Value::Array(values)) if values.iter().any(|entry| entry == "string") => FieldKind::String,
            Some(Value::Array(values)) if values.iter().any(|entry| entry == "integer") => FieldKind::Number,
            _ => FieldKind::Json,
        };

        if matches!(descriptor.kind, FieldKind::Json) {
            descriptor.description = Some(FieldDescriptor::json_help(descriptor.description.take()));
        }

        descriptor
    }

    fn format_auth_summary(status: AuthStatus) -> Option<String> {
        match status {
            AuthStatus::Unknown => None,
            AuthStatus::Authorized => Some("Authenticated".to_string()),
            AuthStatus::Required => Some("Authentication required".to_string()),
            AuthStatus::Failed => Some("Authentication failed".to_string()),
        }
    }

    fn normalize_tool_result(result: rmcp::model::CallToolResult) -> (bool, Value) {
        let is_error = result.is_error.unwrap_or(false);

        if let Some(structured) = result.structured_content {
            return (is_error, structured);
        }

        match serde_json::to_value(&result.content) {
            Ok(value) => (is_error, Self::derive_tool_result(&value)),
            Err(_) => (is_error, Value::Null),
        }
    }

    fn derive_tool_result(value: &Value) -> Value {
        match value {
            Value::Null => Value::Null,
            Value::Bool(value) => Value::Bool(*value),
            Value::Number(value) => Value::Number(value.clone()),
            Value::String(value) => Value::String(value.clone()),
            Value::Array(value) => {
                // Special case: if the array contains a single object with a "text" field, return that field's value.
                if value.len() == 1 {
                    return Self::derive_tool_result(value.first().unwrap_or(&Value::Null));
                }
                value.iter().map(Self::derive_tool_result).collect()
            }
            Value::Object(value) => {
                if let Some(Value::String(text)) = value.get("text")
                    && let Ok(val) = serde_json::from_str::<Value>(text)
                {
                    return val;
                }
                Value::Object(value.clone())
            }
        }
    }
}

#[derive(Debug, Clone)]
struct FieldDescriptor {
    name: String,
    description: Option<String>,
    required: bool,
    kind: FieldKind,
    enum_values: Vec<String>,
    default_value: Option<String>,
}

impl FieldDescriptor {
    fn new(name: String, required: bool) -> Self {
        Self {
            name,
            description: None,
            required,
            kind: FieldKind::String,
            enum_values: Vec::new(),
            default_value: None,
        }
    }

    fn json_blob(name: String, required: bool, description: Option<String>) -> Self {
        let mut descriptor = Self::new(name, required);
        descriptor.kind = FieldKind::Json;
        descriptor.description = Some(Self::json_help(description));
        descriptor
    }

    fn into_flag(self) -> CommandFlag {
        CommandFlag {
            name: self.name,
            short_name: None,
            required: self.required,
            r#type: match self.kind {
                FieldKind::Boolean => "boolean".to_string(),
                FieldKind::String | FieldKind::Json => "string".to_string(),
                FieldKind::Number => "number".to_string(),
            },
            enum_values: self.enum_values,
            default_value: self.default_value,
            description: self.description,
            provider: None,
        }
    }

    fn into_positional(self) -> PositionalArgument {
        PositionalArgument {
            name: self.name,
            help: self.description,
            provider: None,
        }
    }

    /// Compose a descriptive help message for JSON-backed inputs, appending a concrete usage hint.
    fn json_help(base: Option<String>) -> String {
        const JSON_HINT: &str = "Provide a JSON string payload (for example: '{\"key\":\"value\"}').";
        match base {
            Some(text) if !text.trim().is_empty() => {
                if text.trim_end().ends_with('.') {
                    format!("{} {}", text.trim_end(), JSON_HINT)
                } else {
                    format!("{}. {}", text.trim_end(), JSON_HINT)
                }
            }
            _ => JSON_HINT.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FieldKind {
    Boolean,
    String,
    Number,
    Json,
}

/// Errors that can occur in the plugin engine.
#[derive(Debug, thiserror::Error)]
pub enum PluginEngineError {
    #[error("Plugin not found: {name}")]
    PluginNotFound { name: String },

    #[error("Client manager error: {0}")]
    ClientManagerError(String),

    #[error("Registry error: {0}")]
    RegistryError(#[from] RegistryError),

    #[error("Lifecycle error: {0}")]
    LifecycleError(#[from] crate::plugin::LifecycleError),

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Maximum restart attempts exceeded for plugin {name}")]
    MaxRestartAttemptsExceeded { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::McpServer;
    use crate::config::McpConfig;
    use oatty_registry::RegistryConfig;
    use serde_json::{Value, json};
    use url::Url;

    fn tool_with_schema(schema: Value) -> McpToolMetadata {
        McpToolMetadata {
            name: "demo".to_string(),
            title: None,
            description: Some("demo tool".to_string()),
            input_schema: schema,
            output_schema: None,
            annotations: None,
            auth_summary: None,
        }
    }

    #[tokio::test]
    async fn test_plugin_engine_creation() {
        let config = McpConfig::default();
        let registry = Arc::new(Mutex::new(CommandRegistry {
            commands: Vec::new(),
            workflows: vec![],
            provider_contracts: Default::default(),
            config: RegistryConfig { catalogs: None },
        }));
        let engine = PluginEngine::new(config, Arc::clone(&registry)).unwrap();

        let plugins = engine.list_plugins().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_plugin_engine_start_stop() {
        let config = McpConfig::default();
        let registry = Arc::new(Mutex::new(CommandRegistry {
            commands: Vec::new(),
            workflows: vec![],
            provider_contracts: Default::default(),
            config: RegistryConfig { catalogs: None },
        }));
        let engine = PluginEngine::new(config, Arc::clone(&registry)).unwrap();

        engine.start().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[test]
    fn convert_schema_maps_required_and_optional_fields() {
        let schema = json!({
            "type": "object",
            "required": ["app"],
            "properties": {
                "app": {
                    "type": "string",
                    "description": "Oatty application name"
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Confirm execution"
                }
            }
        });

        let tool = tool_with_schema(schema);
        let (positionals, flags) = PluginEngine::convert_schema_to_inputs(&tool);

        assert_eq!(positionals.len(), 1);
        assert_eq!(positionals[0].name, "app");
        assert_eq!(positionals[0].help.as_deref(), Some("Oatty application name"));

        assert_eq!(flags.len(), 1);
        let confirm = &flags[0];
        assert_eq!(confirm.name, "confirm");
        assert_eq!(confirm.r#type, "boolean");
        assert_eq!(confirm.description.as_deref(), Some("Confirm execution"));
    }

    #[test]
    fn convert_schema_converts_unknown_types_to_json_inputs() {
        let schema = json!({
            "type": "object",
            "required": ["config"],
            "properties": {
                "config": {
                    "type": "object",
                    "description": "Detailed configuration"
                },
                "metadata": {
                    "type": "array",
                    "description": "Optional metadata"
                }
            }
        });

        let tool = tool_with_schema(schema);
        let (positionals, flags) = PluginEngine::convert_schema_to_inputs(&tool);

        assert_eq!(positionals.len(), 1);
        assert_eq!(positionals[0].name, "config");
        let help = positionals[0].help.as_ref().expect("help text present");
        assert!(help.contains("Detailed configuration"));
        assert!(help.contains("Provide a JSON string payload"));

        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].name, "metadata");
        let flag_help = flags[0].description.as_ref().expect("flag JSON help present");
        assert!(flag_help.contains("Optional metadata"));
        assert!(flag_help.contains("Provide a JSON string payload"));
    }

    #[test]
    fn convert_schema_adds_payload_fallback_when_properties_missing() {
        let tool = tool_with_schema(json!({ "type": "object" }));

        let (positionals, flags) = PluginEngine::convert_schema_to_inputs(&tool);

        assert!(positionals.is_empty());
        assert_eq!(flags.len(), 1);
        let payload_flag = &flags[0];
        assert_eq!(payload_flag.name, "payload");
        let help = payload_flag.description.as_ref().expect("payload flag help present");
        assert!(help.contains("Structured inputs collapsed into JSON"));
        assert!(help.contains("Provide a JSON string payload"));
    }

    #[test]
    fn synthesize_generates_command_spec_from_tool() {
        let tools = vec![McpToolMetadata {
            name: "demo_info".to_string(),
            title: Some("Display info".to_string()),
            description: Some("Show application details".to_string()),
            input_schema: json!({
                "type": "object",
                "required": ["app"],
                "properties": {
                    "app": {
                        "type": "string",
                        "description": "Application name"
                    },
                    "verbose": {
                        "type": "boolean",
                        "description": "Verbose output"
                    }
                }
            }),
            output_schema: None,
            annotations: None,
            auth_summary: None,
        }];

        let specs = PluginEngine::synthesize_mcp_specs("demo-plugin", &tools, Some("Authentication required"));

        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_eq!(spec.group, "demo");
        assert_eq!(spec.name, "info");
        assert_eq!(spec.summary, "Authentication required — Show application details");
        assert_eq!(spec.positional_args.len(), 1);
        assert_eq!(spec.positional_args[0].name, "app");
        assert_eq!(spec.flags.len(), 1);
        assert_eq!(spec.flags[0].name, "verbose");
        assert_eq!(spec.flags[0].r#type, "boolean");
        assert!(matches!(spec.execution(), oatty_types::command::CommandExecution::Mcp(_)));
        let mcp = spec.mcp().expect("mcp execution present");
        assert_eq!(mcp.plugin_name, "demo-plugin");
        assert_eq!(mcp.tool_name, "demo_info");
        assert_eq!(mcp.auth_summary.as_deref(), Some("Authentication required"));
    }

    #[test]
    fn synthesize_falls_back_to_plugin_group_when_prefix_collides() {
        let tools = vec![
            McpToolMetadata {
                name: "deploy_start".to_string(),
                title: None,
                description: None,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "app": { "type": "string" }
                    }
                }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
            McpToolMetadata {
                name: "deploy_stop".to_string(),
                title: None,
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
        ];

        let specs = PluginEngine::synthesize_mcp_specs("deploy-plugin", &tools, None);
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().all(|spec| spec.group == "deploy"));
        let mut names: Vec<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["start", "stop"]);
    }

    #[test]
    fn synthesize_without_shared_prefix_defaults_to_plugin_name_group() {
        let tools = vec![
            McpToolMetadata {
                name: "alpha_sync".to_string(),
                title: None,
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
            McpToolMetadata {
                name: "beta_sync".to_string(),
                title: None,
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
        ];

        let specs = PluginEngine::synthesize_mcp_specs("multi-plugin", &tools, None);
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().all(|spec| spec.group == "multi-plugin"));
        assert!(specs.iter().any(|spec| spec.name == "alpha:sync"));
        assert!(specs.iter().any(|spec| spec.name == "beta:sync"));
    }

    #[test]
    fn synthesize_uses_multi_segment_shared_prefix() {
        let tools = vec![
            McpToolMetadata {
                name: "org_settings_create_team".to_string(),
                title: None,
                description: None,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "team_name": { "type": "string" }
                    }
                }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
            McpToolMetadata {
                name: "org_settings_delete_team".to_string(),
                title: None,
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
        ];

        let specs = PluginEngine::synthesize_mcp_specs("org-service", &tools, None);
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().all(|spec| spec.group == "org"));
        assert!(specs.iter().any(|spec| spec.name == "settings:create:team"));
        assert!(specs.iter().any(|spec| spec.name == "settings:delete:team"));
    }

    #[test]
    fn synthesize_shared_prefix_removed_from_command_name() {
        let tools = vec![
            McpToolMetadata {
                name: "fleet_jobs_create".to_string(),
                title: None,
                description: None,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "job_type": { "type": "string" }
                    }
                }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
            McpToolMetadata {
                name: "fleet_jobs_delete".to_string(),
                title: None,
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                annotations: None,
                auth_summary: None,
            },
        ];

        let specs = PluginEngine::synthesize_mcp_specs("fleet", &tools, None);
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().all(|spec| spec.group == "fleet"));
        assert!(specs.iter().any(|spec| spec.name == "jobs:create"));
        assert!(specs.iter().any(|spec| spec.name == "jobs:delete"));
    }

    #[test]
    fn synthesize_converts_complex_fields_to_json_string() {
        let tools = vec![McpToolMetadata {
            name: "config_set".to_string(),
            title: None,
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "config": {
                        "type": "object",
                        "description": "Configuration payload"
                    }
                }
            }),
            output_schema: None,
            annotations: None,
            auth_summary: None,
        }];

        let specs = PluginEngine::synthesize_mcp_specs("cfg", &tools, None);
        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_eq!(spec.flags.len(), 1);
        let flag = &spec.flags[0];
        assert_eq!(flag.r#type, "string");
        assert!(
            flag.description
                .as_deref()
                .unwrap_or_default()
                .contains("Provide a JSON string payload")
        );
    }

    #[test]
    fn format_auth_summary_maps_status_to_message() {
        assert_eq!(PluginEngine::format_auth_summary(AuthStatus::Unknown), None);
        assert_eq!(
            PluginEngine::format_auth_summary(AuthStatus::Authorized),
            Some("Authenticated".to_string())
        );
        assert_eq!(
            PluginEngine::format_auth_summary(AuthStatus::Required),
            Some("Authentication required".to_string())
        );
        assert_eq!(
            PluginEngine::format_auth_summary(AuthStatus::Failed),
            Some("Authentication failed".to_string())
        );
    }

    #[tokio::test]
    async fn test_engine_registers_tags_from_config() {
        let mut cfg = McpConfig::default();
        let server = McpServer {
            base_url: Some(Url::parse("https://example.com").unwrap()),
            tags: Some(vec!["alpha".into(), "beta".into()]),
            disabled: Some(true),
            ..Default::default()
        };
        cfg.mcp_servers.insert("svc".into(), server);

        let registry = Arc::new(Mutex::new(CommandRegistry {
            commands: Vec::new(),
            workflows: vec![],
            provider_contracts: Default::default(),
            config: RegistryConfig { catalogs: None },
        }));
        let engine = PluginEngine::new(cfg, Arc::clone(&registry)).unwrap();
        engine.start().await.unwrap();

        let registry = engine.prepare_registry().await.unwrap();
        let names = registry.get_plugin_names();
        assert_eq!(names, vec!["svc".to_string()]);
        let info = registry.get_plugin("svc").unwrap();
        assert_eq!(info.tags, vec!["alpha", "beta"]);
        assert!(!info.enabled);

        engine.stop().await.unwrap();
    }
}
