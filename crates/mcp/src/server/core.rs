use crate::PluginEngine;
use crate::server::http::McpHttpLogEntry;
use crate::server::schemas::{CommandSummariesRequest, RunCommandRequestParam, SearchRequestParam};
use anyhow::Result;
use oatty_registry::{CommandRegistry, SearchHandle};
use oatty_types::{CommandSpec, ExecOutcome, SearchResult};
use oatty_util::http::exec_remote_from_shell_command;
use reqwest::Method;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorData, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::vec;
use tokio::sync::mpsc::UnboundedSender;

/// Shared services for MCP tool handlers.
#[derive(Debug)]
pub struct McpToolServices {
    command_registry: Arc<Mutex<CommandRegistry>>,
    plugin_engine: Arc<PluginEngine>,
    search_handle: SearchHandle,
}

impl McpToolServices {
    /// Create services backed by the provided command registry and search engine.
    pub(crate) fn new(
        command_registry: Arc<Mutex<CommandRegistry>>,
        plugin_engine: Arc<PluginEngine>,
        search_handle: SearchHandle,
    ) -> Self {
        Self {
            command_registry,
            plugin_engine,
            search_handle,
        }
    }

    async fn search_commands(&self, query: String, vendor: Option<&str>) -> Result<Vec<SearchResult>> {
        let results = self.search_handle.search(query).await?;
        let Some(vendor_name) = vendor else {
            return Ok(results);
        };
        let registry = self
            .command_registry
            .lock()
            .map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;
        let filtered = results
            .into_iter()
            .filter(|result| vendor_matches(&registry, result, vendor_name))
            .collect();
        Ok(filtered)
    }
}

#[derive(Clone)]
pub struct OattyMcpCore {
    tool_router: ToolRouter<Self>,
    log_sender: Option<UnboundedSender<McpHttpLogEntry>>,
    services: Arc<McpToolServices>,
}

#[tool_router]
impl OattyMcpCore {
    /// Create a new MCP core handler with shared service dependencies.
    pub fn new(log_sender: Option<UnboundedSender<McpHttpLogEntry>>, services: Arc<McpToolServices>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            log_sender,
            services,
        }
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "LLM-ONLY TOOL.\nINTENT: discover executable commands before calling any run_* tool.\nWHEN TO USE:\n- User asks to find/list/recommend tools, commands, or integrations.\n- User asks whether a tool already exists for a task (for example: \"Are there tools that do this?\").\n- User asks what commands are available for a workflow, provider, or vendor.\nINPUT:\n- query: free-text search string.\n- vendor: optional exact vendor filter.\nOUTPUT:\n- candidates with routing fields: canonical_id, execution_type, http_method.\nROUTING RULES:\n- execution_type=http and http_method=GET => use run_safe_command.\n- execution_type=http and http_method in {POST,PUT,PATCH} => use run_command.\n- execution_type=http and http_method=DELETE => use run_destructive_command.\n- execution_type=mcp => use run_safe_command or run_command.\nNEXT STEP: copy canonical_id exactly from output."
    )]
    async fn search_commands(&self, param: Parameters<SearchRequestParam>) -> Result<CallToolResult, ErrorData> {
        let results = self
            .services
            .search_commands(param.0.query.clone(), param.0.vendor.as_deref())
            .await
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let response = CallToolResult::structured(serde_json::json!(results));
        self.emit_log(
            "search_commands",
            Some(serde_json::to_value(&param.0).unwrap_or_else(|_| Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "List command topics by vendor. Returns a list of all available command topics with vendor information."
    )]
    async fn list_command_topics(&self) -> Result<CallToolResult, ErrorData> {
        let catalogs = list_registry_catalogs(&self.services.command_registry, &self.services.plugin_engine)
            .await
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let response = CallToolResult::structured(serde_json::json!(catalogs));
        self.emit_log(
            "list_command_catalogs",
            None,
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "LLM-ONLY TOOL.\nINTENT: inspect command input shape before execution.\nINPUT:\n- catalog_title: exact title from list_command_topics.\nOUTPUT PER COMMAND:\n- canonical_id\n- summary\n- execution_type\n- http_method (nullable)\n- positional_args[] (ordered)\n- flags[] (name, type, required, defaults, enum_values)\nUSE WHEN: you need required args/flags or validation-safe construction of run_* payloads."
    )]
    async fn get_command_summaries_by_catalog(&self, param: Parameters<CommandSummariesRequest>) -> Result<CallToolResult, ErrorData> {
        let summaries = list_command_summaries_by_catalog(&self.services.command_registry, param.0.catalog_title.as_str())
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        let response = CallToolResult::structured(serde_json::json!(summaries));
        self.emit_log(
            "get_command_summaries_by_catalog",
            Some(serde_json::to_value(&param.0).unwrap_or_else(|_| Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true, open_world_hint = true),
        description = "LLM-ONLY TOOL.\nINTENT: execute read-only commands.\nWHEN TO USE:\n- HTTP command with method GET.\n- MCP command that is read-only.\nINPUT CONTRACT:\n- canonical_id: '<group> <command>' (example: 'apps apps:list').\n- positional_args: ordered values matching command definition.\n- named_flags: list of [flag_name, value].\n- boolean flags: presence means true; value element is ignored.\nDO NOT USE FOR: HTTP POST/PUT/PATCH/DELETE.\nEXAMPLE:\n{\"canonical_id\":\"apps apps:list\",\"positional_args\":[],\"named_flags\":[[\"json\",\"\"]]}"
    )]
    async fn run_safe_command(&self, param: Parameters<RunCommandRequestParam>) -> Result<CallToolResult, ErrorData> {
        let response = self.execute_command_with_guard(&param.0, HttpMethodGuard::SafeGet).await?;
        self.emit_log(
            "run_safe_command",
            Some(serde_json::to_value(&param.0).unwrap_or_else(|_| Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(open_world_hint = true),
        description = "LLM-ONLY TOOL.\nINTENT: execute non-destructive write commands.\nWHEN TO USE:\n- HTTP command with method POST, PUT, or PATCH.\n- MCP command that is non-destructive.\nINPUT CONTRACT:\n- canonical_id: '<group> <command>' (example: 'apps apps:create').\n- positional_args: ordered values matching command definition.\n- named_flags: list of [flag_name, value].\n- boolean flags: presence means true; value element is ignored.\nDO NOT USE FOR: HTTP GET (use run_safe_command) or HTTP DELETE (use run_destructive_command).\nEXAMPLE:\n{\"canonical_id\":\"apps apps:create\",\"positional_args\":[\"my-app\"],\"named_flags\":[[\"region\",\"us\"],[\"private\",\"\"]]}"
    )]
    async fn run_command(&self, param: Parameters<RunCommandRequestParam>) -> Result<CallToolResult, ErrorData> {
        let response = self.execute_command_with_guard(&param.0, HttpMethodGuard::Write).await?;
        self.emit_log(
            "run_command",
            Some(serde_json::to_value(&param.0).unwrap_or_else(|_| Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(open_world_hint = true),
        description = "LLM-ONLY TOOL.\nINTENT: execute destructive HTTP commands.\nWHEN TO USE:\n- HTTP command with method DELETE only.\nINPUT CONTRACT:\n- canonical_id: '<group> <command>'.\n- positional_args: ordered values matching command definition.\n- named_flags: list of [flag_name, value].\n- boolean flags: presence means true; value element is ignored.\nHARD LIMITS:\n- MCP commands are rejected.\n- HTTP methods other than DELETE are rejected.\nEXAMPLE:\n{\"canonical_id\":\"apps apps:delete\",\"positional_args\":[\"my-app\"],\"named_flags\":[]}"
    )]
    async fn run_destructive_command(&self, param: Parameters<RunCommandRequestParam>) -> Result<CallToolResult, ErrorData> {
        let response = self
            .execute_command_with_guard(&param.0, HttpMethodGuard::DestructiveDelete)
            .await?;
        self.emit_log(
            "run_destructive_command",
            Some(serde_json::to_value(&param.0).unwrap_or_else(|_| Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    #[tool(description = "List available workflows")]
    async fn get_workflows(&self) -> Result<CallToolResult, ErrorData> {
        let response = CallToolResult::success(vec![Content::text("1".to_string())]);
        self.emit_log(
            "get_workflows",
            None,
            Some(serde_json::to_value(&response).unwrap_or_else(|_| Value::Null)),
        );
        Ok(response)
    }

    async fn execute_command_with_guard(
        &self,
        param: &RunCommandRequestParam,
        method_guard: HttpMethodGuard,
    ) -> Result<CallToolResult, ErrorData> {
        let command_spec = resolve_command_spec(&self.services.command_registry, &param.canonical_id)?;
        if let Some(http_spec) = command_spec.http() {
            let method = Method::from_str(&http_spec.method)
                .map_err(|error| ErrorData::invalid_params(format!("invalid HTTP method: {error}"), None))?;
            method_guard.ensure_allowed(&method)?;

            let hydrated_input = hydrate_shell_command(&command_spec, param)?;
            let exec_outcome = execute_http_command(&self.services.command_registry, &command_spec, hydrated_input).await?;
            let structured = exec_outcome_to_value(exec_outcome)?;
            return Ok(CallToolResult::structured(structured));
        }

        if command_spec.mcp().is_some() {
            method_guard.ensure_mcp_allowed()?;
            let arguments = build_mcp_arguments(&command_spec, param)?;
            let exec_outcome = self
                .services
                .plugin_engine
                .execute_tool(&command_spec, &arguments, 0)
                .await
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
            let structured = exec_outcome_to_value(exec_outcome)?;
            return Ok(CallToolResult::structured(structured));
        }

        Err(ErrorData::invalid_params(
            "command execution type is unsupported by the MCP server",
            None,
        ))
    }

    fn emit_log(&self, tool_name: &str, request: Option<Value>, response: Option<Value>) {
        let Some(sender) = self.log_sender.as_ref() else {
            return;
        };
        let mut payload = Map::new();
        if let Some(request) = request {
            payload.insert("request".to_string(), request);
        }
        if let Some(response) = response {
            payload.insert("response".to_string(), response);
        }
        let payload = if payload.is_empty() { None } else { Some(Value::Object(payload)) };
        let message = format!("MCP HTTP: {tool_name}");
        let _ = sender.send(McpHttpLogEntry::new(message, payload));
    }
}

#[tool_handler]
impl ServerHandler for OattyMcpCore {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "Oatty".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Oatty MCP".to_string()),
                ..Default::default()
            },
            instructions: Some(
                "LLM-ONLY SERVER INSTRUCTIONS.\nSEQUENCE:\n1) Call search_commands.\n2) Select canonical_id from results.\n3) Route by execution_type/http_method.\nROUTING TABLE:\n- http + GET => run_safe_command\n- http + POST|PUT|PATCH => run_command\n- http + DELETE => run_destructive_command\n- mcp + read-only => run_safe_command\n- mcp + non-destructive => run_command\n- mcp + destructive => unsupported\nVALIDATION FLOW:\n- If args/flags are unclear, call get_command_summaries_by_catalog.\n- Build positional_args in declared order.\n- Build named_flags as [name,value]; boolean flags use presence semantics.".to_string()
            ),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HttpMethodGuard {
    SafeGet,
    Write,
    DestructiveDelete,
}

impl HttpMethodGuard {
    fn ensure_allowed(&self, method: &Method) -> Result<(), ErrorData> {
        let allowed = match self {
            HttpMethodGuard::SafeGet => method == Method::GET,
            HttpMethodGuard::Write => matches!(*method, Method::POST | Method::PUT | Method::PATCH),
            HttpMethodGuard::DestructiveDelete => method == Method::DELETE,
        };
        if allowed {
            Ok(())
        } else {
            Err(ErrorData::invalid_params(
                format!("HTTP method '{method}' is not allowed for this tool"),
                Some(serde_json::json!({
                    "expected_tool": self.tool_name(),
                    "allowed_http_methods": self.allowed_http_methods(),
                    "next_step": "Use search_commands to inspect execution_type/http_method before selecting a runner tool."
                })),
            ))
        }
    }

    fn ensure_mcp_allowed(&self) -> Result<(), ErrorData> {
        match self {
            HttpMethodGuard::DestructiveDelete => Err(ErrorData::invalid_params(
                "MCP-backed commands cannot be run with the destructive command tool",
                Some(serde_json::json!({
                    "expected_tool": "run_command",
                    "allowed_execution_type": "mcp",
                    "next_step": "Use run_safe_command for read-only MCP tools or run_command for non-destructive MCP tools."
                })),
            )),
            HttpMethodGuard::SafeGet | HttpMethodGuard::Write => Ok(()),
        }
    }

    fn tool_name(&self) -> &'static str {
        match self {
            HttpMethodGuard::SafeGet => "run_safe_command",
            HttpMethodGuard::Write => "run_command",
            HttpMethodGuard::DestructiveDelete => "run_destructive_command",
        }
    }

    fn allowed_http_methods(&self) -> &'static [&'static str] {
        match self {
            HttpMethodGuard::SafeGet => &["GET"],
            HttpMethodGuard::Write => &["POST", "PUT", "PATCH"],
            HttpMethodGuard::DestructiveDelete => &["DELETE"],
        }
    }
}

fn resolve_command_spec(registry: &Arc<Mutex<CommandRegistry>>, canonical_id: &str) -> Result<CommandSpec, ErrorData> {
    let (group, name) = split_canonical_id(canonical_id)?;
    let registry_guard = registry
        .lock()
        .map_err(|error| ErrorData::internal_error(format!("registry lock failed: {error}"), None))?;
    registry_guard
        .find_by_group_and_cmd_cloned(&group, &name)
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
}

fn split_canonical_id(canonical_id: &str) -> Result<(String, String), ErrorData> {
    let trimmed = canonical_id.trim();
    let (group, name) = trimmed.split_once(' ').ok_or_else(|| {
        ErrorData::invalid_params(
            "canonical_id must be in 'group command' format",
            Some(serde_json::json!({
                "expected_format": "<group> <command>",
                "example": "apps apps:list",
                "next_step": "Use search_commands to copy an exact canonical_id."
            })),
        )
    })?;
    if group.is_empty() || name.is_empty() {
        return Err(ErrorData::invalid_params(
            "canonical_id must include both group and command",
            Some(serde_json::json!({
                "expected_format": "<group> <command>",
                "example": "apps apps:list"
            })),
        ));
    }
    Ok((group.to_string(), name.to_string()))
}

async fn list_registry_catalogs(registry: &Arc<Mutex<CommandRegistry>>, plugin_engine: &Arc<PluginEngine>) -> Result<Vec<Value>> {
    let mut response: Vec<Value> = {
        let registry_guard = registry.lock().map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;
        let empty = Vec::new();
        let catalogs = registry_guard.config.catalogs.as_ref().unwrap_or(&empty);

        catalogs
            .iter()
            .filter(|catalog| catalog.is_enabled)
            .map(|catalog| {
                let vendor = catalog
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.vendor.clone())
                    .unwrap_or_default();
                serde_json::json!({
                    "title": catalog.title,
                    "vendor": vendor,
                    "description": catalog.description,
                    "type": "command"
                })
            })
            .collect()
    };

    if let Some(mut infos) = plugin_engine.get_active_client_infos().await {
        infos.drain(..).for_each(|info| {
            let instructions = info.instructions.unwrap_or_default();
            let Implementation { title, name, .. } = info.server_info;
            response.push(serde_json::json!({
                "title": title.unwrap_or_default(),
                "vendor": name,
                "description": instructions,
                "type": "plugin"
            }))
        })
    }

    Ok(response)
}

fn list_command_summaries_by_catalog(registry: &Arc<Mutex<CommandRegistry>>, catalog_title: &str) -> Result<Vec<Value>> {
    let registry_guard = registry.lock().map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;

    let catalogs = registry_guard
        .config
        .catalogs
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no catalogs configured"))?;
    let catalog_index = catalogs
        .iter()
        .position(|catalog| catalog.title == catalog_title)
        .ok_or_else(|| anyhow::anyhow!("catalog '{}' not found", catalog_title))?;
    let summaries = registry_guard
        .commands
        .iter()
        .filter(|command| command.catalog_identifier == catalog_index)
        .map(|command| {
            serde_json::json!({
                "canonical_id": command.canonical_id(),
                "summary": command.summary,
                "execution_type": command_execution_type(command),
                "http_method": command.http().map(|http| http.method.clone()),
                "positional_args": command.positional_args.iter().map(|positional_arg| {
                    serde_json::json!({
                        "name": positional_arg.name,
                        "required": true,
                        "help": positional_arg.help,
                    })
                }).collect::<Vec<Value>>(),
                "flags": command.flags.iter().map(|flag| {
                    serde_json::json!({
                        "name": flag.name,
                        "short_name": flag.short_name,
                        "required": flag.required,
                        "type": flag.r#type,
                        "enum_values": flag.enum_values,
                        "default_value": flag.default_value,
                        "description": flag.description,
                    })
                }).collect::<Vec<Value>>(),
            })
        })
        .collect();
    Ok(summaries)
}

fn command_execution_type(command_spec: &CommandSpec) -> &'static str {
    if command_spec.http().is_some() {
        return "http";
    }
    if command_spec.mcp().is_some() {
        return "mcp";
    }
    "unknown"
}

fn hydrate_shell_command(command_spec: &CommandSpec, param: &RunCommandRequestParam) -> Result<String, ErrorData> {
    let positional_args = param.positional_args.clone().unwrap_or_default();
    let named_flags = param.named_flags.clone().unwrap_or_default();

    let flag_map = build_flag_map(command_spec, &named_flags)?;
    let positional_strings = positional_args.clone();
    command_spec
        .validate_arguments(&flag_map, &positional_strings)
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;

    let mut parts = Vec::new();
    parts.push(command_spec.group.clone());
    parts.push(command_spec.name.clone());
    for arg in positional_args {
        parts.push(format_shell_token(&arg));
    }
    for (name, value) in named_flags {
        if is_boolean_flag(command_spec, &name) {
            parts.push(format!("--{}", name));
        } else {
            parts.push(format!("--{}={}", name, format_shell_token(&value)));
        }
    }

    Ok(parts.join(" "))
}

fn build_flag_map(command_spec: &CommandSpec, named_flags: &[(String, String)]) -> Result<HashMap<String, Option<String>>, ErrorData> {
    let mut map = HashMap::new();
    for (name, value) in named_flags {
        let flag_spec = command_spec.flags.iter().find(|flag| flag.name == *name).ok_or_else(|| {
            ErrorData::invalid_params(
                format!("unknown flag '--{}'", name),
                Some(serde_json::json!({
                    "unknown_flag": name,
                    "known_flags": command_spec.flags.iter().map(|flag| flag.name.clone()).collect::<Vec<String>>(),
                    "next_step": "Call get_command_summaries_by_catalog for valid flag names."
                })),
            )
        })?;
        if flag_spec.r#type == "boolean" {
            map.insert(name.clone(), None);
        } else {
            map.insert(name.clone(), Some(value.clone()));
        }
    }
    Ok(map)
}

fn build_mcp_arguments(command_spec: &CommandSpec, param: &RunCommandRequestParam) -> Result<Map<String, Value>, ErrorData> {
    let positional_args = param.positional_args.clone().unwrap_or_default();
    let named_flags = param.named_flags.clone().unwrap_or_default();

    let flag_map = build_flag_map(command_spec, &named_flags)?;
    let positional_strings = positional_args.clone();
    command_spec
        .validate_arguments(&flag_map, &positional_strings)
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;

    let mut arguments = Map::new();

    for (spec, value) in command_spec.positional_args.iter().zip(positional_args.iter()) {
        arguments.insert(spec.name.clone(), Value::String(value.clone()));
    }

    for (name, value) in named_flags {
        if is_boolean_flag(command_spec, &name) {
            arguments.insert(name, Value::Bool(true));
        } else {
            arguments.insert(name, Value::String(value));
        }
    }

    Ok(arguments)
}

fn is_boolean_flag(command_spec: &CommandSpec, name: &str) -> bool {
    command_spec
        .flags
        .iter()
        .find(|flag| flag.name == name)
        .is_some_and(|flag| flag.r#type == "boolean")
}

fn format_shell_token(token: &str) -> String {
    if token.chars().all(|ch| !ch.is_whitespace() && ch != '"' && ch != '\\') {
        return token.to_string();
    }
    let escaped = token.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

async fn execute_http_command(
    registry: &Arc<Mutex<CommandRegistry>>,
    command_spec: &CommandSpec,
    hydrated_input: String,
) -> Result<ExecOutcome, ErrorData> {
    let (base_url, headers) = {
        let registry_guard = registry
            .lock()
            .map_err(|error| ErrorData::internal_error(format!("registry lock failed: {error}"), None))?;
        let base_url = registry_guard
            .resolve_base_url_for_command(command_spec)
            .ok_or_else(|| ErrorData::invalid_params("base url not configured", None))?;
        let headers = registry_guard
            .resolve_headers_for_command(command_spec)
            .ok_or_else(|| ErrorData::invalid_params("headers not configured", None))?
            .clone();
        (base_url, headers)
    };

    exec_remote_from_shell_command(command_spec, base_url, &headers, hydrated_input, 0)
        .await
        .map_err(|error| ErrorData::internal_error(error, None))
}

fn exec_outcome_to_value(outcome: ExecOutcome) -> Result<Value, ErrorData> {
    match outcome {
        ExecOutcome::Http {
            status_code,
            log_entry,
            payload,
            request_id,
        } => Ok(serde_json::json!({
            "status_code": status_code,
            "log_entry": log_entry,
            "payload": payload,
            "request_id": request_id,
        })),
        ExecOutcome::Mcp {
            log_entry,
            payload,
            request_id,
        } => Ok(serde_json::json!({
            "status_code": 200,
            "log_entry": log_entry,
            "payload": payload,
            "request_id": request_id,
        })),
        other => Ok(serde_json::json!({
            "log_entry": format!("Unexpected execution outcome: {other:?}"),
            "status_code": 520,
            "payload": Value::Null,
            "request_id": 0,
        })),
    }
}

fn vendor_matches(registry: &CommandRegistry, result: &SearchResult, vendor_name: &str) -> bool {
    let command = registry
        .commands
        .iter()
        .find(|command| command.canonical_id() == result.canonical_id);
    let Some(command) = command else {
        return false;
    };
    let Some(catalogs) = registry.config.catalogs.as_ref() else {
        return false;
    };
    let Some(catalog) = catalogs.get(command.catalog_identifier) else {
        return false;
    };
    let Some(manifest) = catalog.manifest.as_ref() else {
        return false;
    };
    manifest.vendor == vendor_name
}
