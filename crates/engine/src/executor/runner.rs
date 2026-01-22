use anyhow::{Result, anyhow};
use reqwest::Method;
use serde_json::Value;
use std::str::FromStr;
use tracing::{debug, warn};

use crate::provider::ProviderIdentifier;
use crate::resolve::RunContext;

use oatty_api::OattyClient;
use oatty_registry::CommandRegistry;
use oatty_util::{block_on_future, build_path, http::execute_http_json_request};

/// Execute a single command.
///
/// Engines can provide concrete implementations that call HTTP, CLI, or other backends.
/// The default runner is a no-op façade that echoes inputs for testing and previews.
pub trait CommandRunner {
    /// Execute the given `run` command with optional named `with` parameters and JSON `body`.
    ///
    /// Implementations may use the `run_context` for read-only access to inputs, env, or previous
    /// step outputs to influence execution.
    fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, run_context: &RunContext) -> Result<Value>;
}

/// A simple runner that returns a synthetic JSON payload. This allows tests and
/// previews without external side effects.
pub struct NoopRunner;
impl CommandRunner for NoopRunner {
    fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, _run_context: &RunContext) -> Result<Value> {
        let mut obj = serde_json::Map::new();
        obj.insert("run".into(), Value::String(run.to_string()));
        if let Some(w) = with {
            obj.insert("with".into(), w.clone());
        }
        if let Some(b) = body {
            obj.insert("body".into(), b.clone());
        }
        Ok(Value::Object(obj))
    }
}

/// Registry-backed command runner that resolves `run` identifiers via the
/// command registry and executes HTTP requests using the catalog-selected base URL.
pub struct RegistryCommandRunner {
    registry: CommandRegistry,
}

impl RegistryCommandRunner {
    /// Create a new registry-backed runner from explicit dependencies.
    pub fn new(registry: CommandRegistry) -> Self {
        Self { registry }
    }
}

impl CommandRunner for RegistryCommandRunner {
    fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, _run_context: &RunContext) -> Result<Value> {
        debug!(
            run = %run,
            has_with = with.is_some(),
            has_body = body.is_some(),
            "registry runner executing command"
        );
        let identifier = parse_run_identifier(run)?;
        let command_spec = self.registry.find_by_group_and_cmd(&identifier.group, &identifier.name)?;

        if let Some(mcp) = command_spec.mcp() {
            warn!(
                command = %command_spec.canonical_id(),
                plugin = %mcp.plugin_name,
                tool = %mcp.tool_name,
                "command delegates to MCP tool; HTTP runner only"
            );
            return Err(anyhow!(
                "command '{}' delegates to MCP tool '{}:{}'; commands currently support HTTP only",
                command_spec.name,
                mcp.plugin_name,
                mcp.tool_name
            ));
        }
        debug!(command = %command_spec.canonical_id(), "resolved command spec");
        let http_spec = command_spec
            .http()
            .ok_or_else(|| anyhow!("command '{}' is not HTTP-backed", command_spec.name))?;
        let method = Method::from_str(&http_spec.method).map_err(|error| anyhow!(error))?;
        let base_url = self
            .registry
            .resolve_base_url_for_command(&command_spec)
            .ok_or_else(|| anyhow!("base url not configured"))?;
        let headers = self
            .registry
            .resolve_headers_for_command(&command_spec)
            .ok_or_else(|| anyhow!("could not determine headers for command: {}", command_spec.canonical_id()))?;
        debug!(
            command = %command_spec.canonical_id(),
            base_url = %base_url,
            header_count = headers.len(),
            "resolved command HTTP settings"
        );
        let client = OattyClient::new(base_url, headers).map_err(|error| anyhow!("could not create the HTTP client: {error}"))?;

        let mut input_map = extract_input_map(with);
        let path_variables = extract_path_variables(&command_spec, &mut input_map);

        let request_path = build_path(&http_spec.path, &path_variables);
        let request_method = method.clone();
        let request_body = body.cloned();
        let request_future =
            async move { execute_http_json_request(&client, request_method, &request_path, input_map, request_body).await };
        let response_payload = block_on_future(request_future)?;

        Ok(response_payload)
    }
}

fn parse_run_identifier(run: &str) -> Result<ProviderIdentifier> {
    ProviderIdentifier::parse(run).ok_or_else(|| anyhow!("invalid run identifier: {run}"))
}

fn extract_input_map(with: Option<&Value>) -> serde_json::Map<String, Value> {
    match with {
        Some(Value::Object(map)) => map.clone(),
        _ => serde_json::Map::new(),
    }
}

fn extract_path_variables(
    command_spec: &oatty_types::CommandSpec,
    input_map: &mut serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    let mut path_variables = serde_json::Map::new();
    for positional_argument in &command_spec.positional_args {
        if let Some(value) = input_map.remove(&positional_argument.name) {
            path_variables.insert(positional_argument.name.clone(), value);
        }
    }
    path_variables
}
