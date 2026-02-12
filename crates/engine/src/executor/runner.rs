use anyhow::{Result, anyhow};
use reqwest::Method;
use serde_json::Value;
use std::str::FromStr;
use tracing::{debug, warn};

use crate::provider::ProviderIdentifier;
use crate::resolve::RunContext;

use oatty_api::OattyClient;
use oatty_registry::CommandRegistry;
use oatty_types::workflow::RuntimeWorkflow;
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

/// Structured workflow preflight validation violation.
///
/// These violations are emitted before execution starts so callers can surface
/// actionable guidance for missing catalogs, unresolved command identifiers, or
/// unsupported command wiring.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkflowPreflightViolation {
    /// Zero-based index of the failing step in workflow authoring order.
    pub step_index: usize,
    /// Step identifier from the workflow definition.
    pub step_id: String,
    /// Raw `run` expression on the failing step.
    pub run: String,
    /// Stable machine-readable violation code.
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// Suggested remediation to unblock execution.
    pub suggested_action: String,
}

impl RegistryCommandRunner {
    /// Create a new registry-backed runner from explicit dependencies.
    pub fn new(registry: CommandRegistry) -> Self {
        Self { registry }
    }

    /// Validates workflow step command wiring against the loaded registry.
    ///
    /// This preflight check is side-effect free and allows callers to fail
    /// early with deterministic, structured diagnostics rather than surfacing
    /// runtime errors after partial execution begins.
    pub fn validate_workflow_execution_readiness(&self, workflow: &RuntimeWorkflow) -> Vec<WorkflowPreflightViolation> {
        workflow
            .steps
            .iter()
            .enumerate()
            .filter_map(|(step_index, step_definition)| {
                let parsed_identifier = match parse_run_identifier(&step_definition.run) {
                    Ok(identifier) => identifier,
                    Err(_) => {
                        return Some(WorkflowPreflightViolation {
                            step_index,
                            step_id: step_definition.id.clone(),
                            run: step_definition.run.clone(),
                            code: "WORKFLOW_STEP_RUN_INVALID",
                            message: format!(
                                "step run identifier '{}' is invalid; expected '<group> <command>'",
                                step_definition.run
                            ),
                            suggested_action: "Use search/discovery to copy a canonical command id and update this step.".to_string(),
                        });
                    }
                };

                let command_spec = match self
                    .registry
                    .find_by_group_and_cmd_cloned(&parsed_identifier.group, &parsed_identifier.name)
                {
                    Ok(command_specification) => command_specification,
                    Err(_) => {
                        return Some(WorkflowPreflightViolation {
                            step_index,
                            step_id: step_definition.id.clone(),
                            run: step_definition.run.clone(),
                            code: "WORKFLOW_STEP_COMMAND_NOT_FOUND",
                            message: format!("command '{}' was not found in the loaded catalogs", step_definition.run),
                            suggested_action: "Import/enable the required catalog, then update the step run id if needed.".to_string(),
                        });
                    }
                };

                if let Some(mcp_tool) = command_spec.mcp() {
                    return Some(WorkflowPreflightViolation {
                        step_index,
                        step_id: step_definition.id.clone(),
                        run: step_definition.run.clone(),
                        code: "WORKFLOW_STEP_MCP_UNSUPPORTED",
                        message: format!(
                            "command '{}' delegates to MCP tool '{}:{}' and cannot run in workflow HTTP execution mode",
                            command_spec.canonical_id(),
                            mcp_tool.plugin_name,
                            mcp_tool.tool_name
                        ),
                        suggested_action: "Select an HTTP-backed command or execute this MCP operation outside workflow steps.".to_string(),
                    });
                }

                if command_spec.http().is_none() {
                    return Some(WorkflowPreflightViolation {
                        step_index,
                        step_id: step_definition.id.clone(),
                        run: step_definition.run.clone(),
                        code: "WORKFLOW_STEP_NOT_HTTP_BACKED",
                        message: format!("command '{}' is not HTTP-backed", command_spec.canonical_id()),
                        suggested_action: "Use an HTTP-backed command for workflow steps.".to_string(),
                    });
                }

                if self.registry.resolve_base_url_for_command(&command_spec).is_none() {
                    return Some(WorkflowPreflightViolation {
                        step_index,
                        step_id: step_definition.id.clone(),
                        run: step_definition.run.clone(),
                        code: "WORKFLOW_STEP_BASE_URL_MISSING",
                        message: format!(
                            "catalog configuration is incomplete for command '{}' (base URL missing)",
                            command_spec.canonical_id()
                        ),
                        suggested_action: "Configure or re-import the command catalog with a valid base URL.".to_string(),
                    });
                }

                if self.registry.resolve_headers_for_command(&command_spec).is_none() {
                    return Some(WorkflowPreflightViolation {
                        step_index,
                        step_id: step_definition.id.clone(),
                        run: step_definition.run.clone(),
                        code: "WORKFLOW_STEP_HEADERS_MISSING",
                        message: format!(
                            "catalog configuration is incomplete for command '{}' (headers unresolved)",
                            command_spec.canonical_id()
                        ),
                        suggested_action: "Configure required catalog headers (for example Authorization) and retry the workflow run."
                            .to_string(),
                    });
                }

                None
            })
            .collect()
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
        let command_spec = self.registry.find_by_group_and_cmd_cloned(&identifier.group, &identifier.name)?;

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
