use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::ArgMatches;
use indexmap::IndexSet;
use oatty_api::OattyClient;
use oatty_engine::workflow::document::{build_runtime_catalog, runtime_workflow_from_definition};
use oatty_engine::{
    ProviderBindingOutcome, ProviderResolutionEvent, ProviderResolutionSource, RegistryCommandRunner, StepResult, StepStatus,
    WorkflowRunState,
};
use oatty_mcp::{PluginEngine, config::load_config, server::Indexer};
use oatty_registry::{CommandRegistry, build_clap};
use oatty_types::{
    EnvVar, ExecOutcome, RuntimeWorkflow,
    command::{CommandExecution, CommandFlag, CommandSpec},
    workflow::{WorkflowDefinition, validate_candidate_value},
};
use oatty_util::{
    DEFAULT_HISTORY_PROFILE, HistoryKey, HistoryStore, InMemoryHistoryStore, JsonHistoryStore, build_path, has_meaningful_value,
    value_contains_secret, workflow_input_uses_history,
};
use reqwest::Method;
use serde_json::{Map, Number, Value, json};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::warn;
use tracing_subscriber::fmt;

static TUI_ACTIVE: AtomicBool = AtomicBool::new(false);

struct GatedStderr;
impl Write for GatedStderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if TUI_ACTIVE.load(Ordering::Relaxed) {
            // Pretend everything was written successfully, but drop output
            Ok(buf.len())
        } else {
            io::stderr().write(buf)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        if TUI_ACTIVE.load(Ordering::Relaxed) {
            Ok(())
        } else {
            io::stderr().flush()
        }
    }
}

#[tokio::main]
/// Entrypoint for the CLI application.
///
/// This function serves as the main entry point for the Oatty CLI tool. It
/// handles command-line argument parsing and routes execution to either the TUI
/// interface or command execution mode.
///
/// # Behavior
/// - If no subcommands are provided, launches the TUI interface
/// - If workflow subcommands are provided, handles workflow operations
/// - Otherwise, executes the specified Oatty API command
///
/// # Returns
/// Returns `Result<()>` where `Ok(())` indicates successful execution and `Err`
/// contains any error that occurred during execution.
///
/// # Examples
/// ```bash
/// # Launch TUI
/// oatty
///
/// # Execute command
/// oatty apps list
///
/// # Workflow command (if enabled)
/// oatty workflow list
/// ```
async fn main() -> Result<()> {
    init_tracing();
    let cfg = load_config()?;
    let command_registry = Arc::new(Mutex::new(CommandRegistry::from_config()?));

    let plugin_engine = Arc::new(PluginEngine::new(cfg, Arc::clone(&command_registry))?);
    plugin_engine.prepare_registry().await?;
    plugin_engine.start().await?;

    let _indexer = Indexer::new(Arc::clone(&command_registry), plugin_engine.client_manager().subscribe());
    // indexer.start().await?;

    let cli = build_clap(Arc::clone(&command_registry));
    let matches = cli.get_matches();

    // No subcommands --> TUI
    if matches.subcommand_name().is_none() {
        // Silence tracing output to stderr while the TUI is active to avoid overlay
        TUI_ACTIVE.store(true, Ordering::Relaxed);
        let tui_result = oatty_tui::run(Arc::clone(&command_registry), Arc::clone(&plugin_engine)).await;
        TUI_ACTIVE.store(false, Ordering::Relaxed);
        plugin_engine.stop().await?;
        return tui_result;
    }

    let result = run_command(Arc::clone(&command_registry), &matches, Arc::clone(&plugin_engine)).await;
    plugin_engine.stop().await?;
    result
}

/// Initializes the tracing system for logging and diagnostics.
///
/// This function sets up the tracing subscriber with configuration based on the
/// `OATTY_LOG` environment variable. It configures log levels and output
/// formatting for the application's diagnostic system.
///
/// # Environment Variables
/// - `OATTY_LOG`: Controls the logging level. Valid values are:
///   - `error`: Only error messages
///   - `warn`: Warning and error messages
///   - `info`: Info, warning, and error messages (default)
///   - `debug`: Debug, info, warning, and error messages
///   - `trace`: All log levels
///
/// # Behavior
/// - Reads the `OATTY_LOG` environment variable
/// - Defaults to "info" level if not set or invalid
/// - Configures the tracing subscriber with the specified filter
/// - Sets maximum log level to `Level::INFO`
///
/// # Examples
/// ```bash
/// # Set debug logging
/// OATTY_LOG=debug cargo run
///
/// # Set error-only logging
/// OATTY_LOG=error cargo run
/// ```
fn init_tracing() {
    // Respect OATTY_LOG without imposing a lower max level ceiling.
    // Example: OATTY_LOG=debug will now allow `tracing::debug!` to emit.
    let filter = std::env::var("OATTY_LOG").unwrap_or_else(|_| "info".into());
    let _ = fmt().with_env_filter(filter).with_writer(|| GatedStderr).try_init();
}

fn resolve_runtime_workflow(registry: Arc<Mutex<CommandRegistry>>, matches: &ArgMatches) -> Result<RuntimeWorkflow> {
    if let Some(file) = matches.get_one::<String>("file") {
        return load_runtime_workflow_from_file(Path::new(file));
    }

    let workflow_id = matches
        .get_one::<String>("id")
        .context("a workflow identifier must be supplied via --id or --file")?;

    let definitions = {
        let guard = registry.lock().expect("could not obtain lock on registry");
        guard.workflows.clone()
    };

    let catalog = build_runtime_catalog(&definitions)?;
    catalog
        .get(workflow_id)
        .cloned()
        .with_context(|| format!("unknown workflow id: {workflow_id}"))
}

fn load_runtime_workflow_from_file(path: &Path) -> Result<RuntimeWorkflow> {
    let content = fs::read_to_string(path).with_context(|| format!("read workflow {}", path.display()))?;
    let definition: WorkflowDefinition = if matches!(path.extension().and_then(|ext| ext.to_str()), Some(ext) if ext.eq_ignore_ascii_case("json"))
    {
        serde_json::from_str(&content).with_context(|| format!("parse workflow json {}", path.display()))?
    } else {
        serde_yaml::from_str(&content).with_context(|| format!("parse workflow yaml {}", path.display()))?
    };

    runtime_workflow_from_definition(&definition)
}

fn output_workflow_json(state: &WorkflowRunState, results: &[StepResult]) -> Result<()> {
    let provider_events: Vec<_> = state
        .telemetry()
        .provider_resolution_events()
        .iter()
        .map(provider_resolution_event_to_json)
        .collect();
    let step_events: Vec<_> = state
        .telemetry()
        .step_events()
        .iter()
        .map(|event| {
            json!({
                "step_id": event.step_id,
                "status": format!("{:?}", event.status),
            })
        })
        .collect();

    let payload = json!({
        "workflow_id": state.workflow.identifier,
        "title": state.workflow.title,
        "description": state.workflow.description,
        "results": results,
        "telemetry": {
            "provider_resolutions": provider_events,
            "step_events": step_events,
        }
    });

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn output_workflow_human(state: &WorkflowRunState, results: &[StepResult]) {
    println!("Workflow '{}'", state.workflow.identifier);
    for result in results {
        println!("  • {:<20} {}", result.id, format_step_status(result.status));
    }

    let provider_events = state.telemetry().provider_resolution_events();
    if !provider_events.is_empty() {
        println!("\nProvider resolutions:");
        for event in provider_events {
            println!(
                "  - {}.{} [{}] {}",
                event.input,
                event.argument,
                match event.source {
                    ProviderResolutionSource::Automatic => "auto",
                    ProviderResolutionSource::Manual => "manual",
                },
                describe_provider_outcome(&event.outcome)
            );
        }
    }
}

fn provider_resolution_event_to_json(event: &ProviderResolutionEvent) -> Value {
    json!({
        "input": event.input,
        "argument": event.argument,
        "source": match event.source {
            ProviderResolutionSource::Automatic => "automatic",
            ProviderResolutionSource::Manual => "manual",
        },
        "outcome": provider_outcome_to_json(&event.outcome),
    })
}

fn provider_outcome_to_json(outcome: &ProviderBindingOutcome) -> Value {
    match outcome {
        ProviderBindingOutcome::Resolved(value) => json!({
            "status": "resolved",
            "value": value.clone(),
        }),
        ProviderBindingOutcome::Prompt(prompt) => json!({
            "status": "prompt",
            "required": prompt.required,
            "reason": prompt.reason.message,
            "path": prompt.reason.path,
            "source": describe_binding_source(&prompt.source),
        }),
        ProviderBindingOutcome::Skip(decision) => json!({
            "status": "skip",
            "reason": decision.reason.message,
            "path": decision.reason.path,
            "source": describe_binding_source(&decision.source),
        }),
        ProviderBindingOutcome::Error(error) => json!({
            "status": "error",
            "message": error.message,
            "source": error
                .source
                .as_ref()
                .map(describe_binding_source),
        }),
    }
}

fn seed_history_defaults_for_cli(state: &mut WorkflowRunState, store: &dyn HistoryStore) {
    let user_id = DEFAULT_HISTORY_PROFILE.to_string();

    for (input_name, definition) in &state.workflow.inputs {
        if !workflow_input_uses_history(definition) {
            continue;
        }

        let key = HistoryKey::workflow_input(user_id.clone(), state.workflow.identifier.clone(), input_name.clone());

        match store.get_latest_value(&key) {
            Ok(Some(stored)) => {
                if stored.value.is_null() || value_contains_secret(&stored.value) {
                    continue;
                }
                if let Some(validation) = &definition.validate
                    && let Err(error) = validate_candidate_value(&stored.value, validation)
                {
                    warn!(
                        input = %input_name,
                        workflow = %state.workflow.identifier,
                        error = %error,
                        "discarded history default that failed validation"
                    );
                    continue;
                }
                state.run_context.inputs.insert(input_name.clone(), stored.value);
            }
            Ok(None) => {}
            Err(error) => warn!(
                input = %input_name,
                workflow = %state.workflow.identifier,
                error = %error,
                "failed to load history default"
            ),
        }
    }
}

fn persist_history_after_cli_run(state: &WorkflowRunState, store: &dyn HistoryStore) {
    let user_id = DEFAULT_HISTORY_PROFILE.to_string();

    for (input_name, definition) in &state.workflow.inputs {
        if !workflow_input_uses_history(definition) {
            continue;
        }

        let Some(value) = state.run_context.inputs.get(input_name) else {
            continue;
        };

        if !has_meaningful_value(value) || value_contains_secret(value) {
            continue;
        }

        if let Some(validation) = &definition.validate
            && let Err(error) = validate_candidate_value(value, validation)
        {
            warn!(
                input = %input_name,
                workflow = %state.workflow.identifier,
                error = %error,
                "skipping history persistence for invalid value"
            );
            continue;
        }

        let key = HistoryKey::workflow_input(user_id.clone(), state.workflow.identifier.clone(), input_name.clone());

        if let Err(error) = store.insert_value(key, value.clone()) {
            warn!(
                input = %input_name,
                workflow = %state.workflow.identifier,
                error = %error,
                "failed to persist history value"
            );
        }
    }
}

fn describe_provider_outcome(outcome: &ProviderBindingOutcome) -> String {
    match outcome {
        ProviderBindingOutcome::Resolved(value) => {
            if let Some(s) = value.as_str() {
                format!("resolved to '{s}'")
            } else {
                format!("resolved to {}", value)
            }
        }
        ProviderBindingOutcome::Prompt(prompt) => format!("prompted (required: {}, reason: {})", prompt.required, prompt.reason.message),
        ProviderBindingOutcome::Skip(decision) => format!("skipped ({})", decision.reason.message),
        ProviderBindingOutcome::Error(error) => format!("error: {}", error.message),
    }
}

fn describe_binding_source(source: &oatty_engine::BindingSource) -> String {
    match source {
        oatty_engine::BindingSource::Step { step_id } => format!("step:{step_id}"),
        oatty_engine::BindingSource::Input { input_name } => format!("input:{input_name}"),
        oatty_engine::BindingSource::Multiple { step_id, input_name } => {
            format!("step:{step_id}, input:{input_name}")
        }
    }
}

fn format_step_status(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Succeeded => "succeeded",
        StepStatus::Failed => "failed",
        StepStatus::Skipped => "skipped",
    }
}

/// Executes a Oatty API command in CLI mode.
///
/// This function handles the execution of Oatty API commands when the CLI is
/// run with specific command arguments. It parses the command structure, builds
/// the appropriate HTTP request, and executes it against the Oatty API.
///
/// # Arguments
/// - `registry`: The command registry containing API endpoint specifications
/// - `matches`: Parsed command-line arguments from clap
///
/// # Command Structure
/// Commands follow the format: `<group> <qualified_subcommand>` (e.g., `apps
/// app:create`) where:
/// - `group`: The resource group (e.g., "apps", "dynos", "config")
/// - `qualified_subcommand`: The specific command within the group (e.g.,
///   "app:create", "list")
///
/// # Behavior
/// 1. Extracts the command group and subcommand from parsed arguments
/// 2. Looks up the command specification in the registry
/// 3. Collects positional arguments and flags from the command line
/// 4. Builds the HTTP request body from flags
/// 5. Constructs and executes the HTTP request or MCP tool invocation
/// 7. Outputs the response to stdout (with JSON pretty-printing when requested)
///
/// # Returns
/// Returns `Result<()>` where `Ok(())` indicates successful command execution
/// and `Err` contains any error that occurred during processing.
///
/// # Examples
/// ```bash
/// # List apps
/// oatty apps list
///
/// oatty apps app:create --name my-app
///
/// # Set config var
/// oatty config config:set KEY=value
/// ```
async fn run_command(registry: Arc<Mutex<CommandRegistry>>, matches: &ArgMatches, plugin_engine: Arc<PluginEngine>) -> Result<()> {
    let (group, group_matches) = extract_group_and_matches(matches)?;
    let (command_name, command_matches) = extract_command_and_matches(group_matches)?;

    if group == "workflow" {
        return handle_workflow_command(Arc::clone(&registry), matches, command_name, command_matches);
    }

    let (command_spec, base_url, headers) = resolve_command_context(&registry, group, command_name)?;
    let positional_values = collect_positional_values(&command_spec, command_matches);
    let request_body = collect_request_body(&command_spec, command_matches)?;
    let body_value = (!request_body.is_empty()).then_some(Value::Object(request_body.clone()));
    let json_output = matches.get_flag("json");

    match command_spec.execution() {
        CommandExecution::Http(http) => {
            let client = OattyClient::new(&base_url, &headers)?;
            let method = Method::from_bytes(http.method.as_bytes())?;
            let path = build_request_path(&http.path, &positional_values);
            let mut builder = client.request(method, &path);
            if let Some(ref b) = body_value {
                builder = builder.json(b);
            }

            let resp = builder.send().await?;
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if json_output {
                output_json_or_text(&text)?;
            } else {
                println!("{}\n{}", status, text);
            }
            Ok(())
        }
        CommandExecution::Mcp(_) => {
            let mut arguments = request_body;
            for positional_argument in &command_spec.positional_args {
                if let Some(value) = positional_values.get(&positional_argument.name) {
                    arguments.insert(positional_argument.name.clone(), Value::String(value.clone()));
                }
            }

            let outcome = plugin_engine.execute_tool(&command_spec, &arguments, 0).await?;
            match outcome {
                ExecOutcome::Mcp { log_entry, .. } => println!("{}", log_entry),
                ExecOutcome::Log(log) => println!("{}", log),
                other => println!("{:?}", other),
            }
            Ok(())
        }
    }
}

/// Extract the CLI group and its matches from the parsed arguments.
fn extract_group_and_matches(matches: &ArgMatches) -> Result<(&str, &ArgMatches)> {
    matches.subcommand().context("expected a resource group subcommand")
}

/// Extract the command name and its matches from a group-level `ArgMatches`.
fn extract_command_and_matches(group_matches: &ArgMatches) -> Result<(&str, &ArgMatches)> {
    group_matches.subcommand().context("expected a command under the group")
}

/// Resolve the command specification and HTTP metadata for the requested command.
fn resolve_command_context(
    registry: &Arc<Mutex<CommandRegistry>>,
    group: &str,
    command_name: &str,
) -> Result<(CommandSpec, String, IndexSet<EnvVar>)> {
    let registry_lock = registry.lock().expect("could not obtain lock on registry");
    let command_spec = registry_lock.find_by_group_and_cmd(group, command_name)?;
    let base_url = registry_lock
        .resolve_base_url_for_command(&command_spec)
        .ok_or(anyhow!("base url not defined for this command"))?;
    let headers = registry_lock
        .resolve_headers_for_command(&command_spec)
        .ok_or(anyhow!("headers not defined for this command"))?
        .clone();
    Ok((command_spec, base_url, headers))
}

/// Collect positional argument values from the parsed command matches.
fn collect_positional_values(command_spec: &CommandSpec, command_matches: &ArgMatches) -> HashMap<String, String> {
    let mut positional_values: HashMap<String, String> = HashMap::new();
    for positional_argument in &command_spec.positional_args {
        if let Some(value) = command_matches.get_one::<String>(&positional_argument.name) {
            positional_values.insert(positional_argument.name.clone(), value.to_string());
        }
    }
    positional_values
}

/// Collect request body fields from the parsed command matches.
fn collect_request_body(command_spec: &CommandSpec, command_matches: &ArgMatches) -> Result<Map<String, Value>> {
    let mut request_body = Map::new();
    for flag in &command_spec.flags {
        if flag.r#type == "boolean" {
            if command_matches.get_flag(&flag.name) {
                request_body.insert(flag.name.clone(), Value::Bool(true));
            }
        } else if let Some(raw_value) = command_matches.get_one::<String>(&flag.name) {
            let parsed_value = parse_flag_value(flag, raw_value)?;
            request_body.insert(flag.name.clone(), parsed_value);
        }
    }
    Ok(request_body)
}

/// Parse a flag value into the appropriate JSON type based on its schema metadata.
fn parse_flag_value(flag: &CommandFlag, raw_value: &str) -> Result<Value> {
    match flag.r#type.as_str() {
        "integer" => {
            let parsed = raw_value
                .parse::<i64>()
                .with_context(|| format!("invalid integer value for --{}", flag.name))?;
            Ok(Value::Number(Number::from(parsed)))
        }
        "number" => {
            let parsed = raw_value
                .parse::<f64>()
                .with_context(|| format!("invalid number value for --{}", flag.name))?;
            Number::from_f64(parsed)
                .map(Value::Number)
                .ok_or_else(|| anyhow!("invalid number value for --{}", flag.name))
        }
        "boolean" => {
            let parsed = raw_value
                .parse::<bool>()
                .with_context(|| format!("invalid boolean value for --{}", flag.name))?;
            Ok(Value::Bool(parsed))
        }
        "object" => {
            let parsed: Value = serde_json::from_str(raw_value).with_context(|| format!("invalid JSON object for --{}", flag.name))?;
            if parsed.is_object() {
                Ok(parsed)
            } else {
                Err(anyhow!("expected JSON object for --{}", flag.name))
            }
        }
        "array" => {
            let parsed: Value = serde_json::from_str(raw_value).with_context(|| format!("invalid JSON array for --{}", flag.name))?;
            if parsed.is_array() {
                Ok(parsed)
            } else {
                Err(anyhow!("expected JSON array for --{}", flag.name))
            }
        }
        _ => Ok(Value::String(raw_value.to_string())),
    }
}

/// Build a percent-encoded request path for positional arguments.
fn build_request_path(template: &str, positional_values: &HashMap<String, String>) -> String {
    let mut variables = Map::new();
    for (key, value) in positional_values {
        variables.insert(key.clone(), Value::String(value.clone()));
    }
    build_path(template, &variables)
}

/// Print a JSON response body when possible, falling back to raw text.
fn output_json_or_text(text: &str) -> Result<()> {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(_) => println!("{}", text),
    }
    Ok(())
}

fn handle_workflow_command(
    registry: Arc<Mutex<CommandRegistry>>,
    root_matches: &ArgMatches,
    subcommand: &str,
    sub_matches: &ArgMatches,
) -> Result<()> {
    let json_output = root_matches.get_flag("json");

    match subcommand {
        "list" => list_workflows(registry, json_output),
        "preview" => preview_workflow(registry, json_output, sub_matches),
        "run" => run_workflow(registry, json_output, sub_matches),
        other => bail!("Unsupported workflow subcommand: {other}"),
    }
}

fn list_workflows(registry: Arc<Mutex<CommandRegistry>>, json_output: bool) -> Result<()> {
    let definitions = {
        let guard = registry.lock().expect("could not obtain lock on registry");
        guard.workflows.clone()
    };

    if definitions.is_empty() {
        if json_output {
            println!("[]");
        } else {
            println!("No workflows available.");
        }
        return Ok(());
    }

    let catalog = build_runtime_catalog(&definitions)?;
    if json_output {
        let payload: Vec<_> = catalog
            .values()
            .map(|wf| {
                json!({
                    "id": wf.identifier,
                    "title": wf.title,
                    "description": wf.description,
                    "inputs": wf.inputs.len(),
                    "steps": wf.steps.len(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("Available workflows:");
        for workflow in catalog.values() {
            let step_count = workflow.steps.len();
            let input_count = workflow.inputs.len();
            match workflow.title.as_deref() {
                Some(title) if !title.is_empty() => {
                    println!(
                        "- {} — {} ({} steps, {} inputs)",
                        workflow.identifier, title, step_count, input_count
                    );
                }
                _ => {
                    println!("- {} ({} steps, {} inputs)", workflow.identifier, step_count, input_count);
                }
            }
        }
    }

    Ok(())
}

fn preview_workflow(registry: Arc<Mutex<CommandRegistry>>, json_output: bool, matches: &ArgMatches) -> Result<()> {
    let runtime = resolve_runtime_workflow(registry, matches)?;
    let format = matches.get_one::<String>("format").map(|s| s.as_str()).unwrap_or("yaml");

    if json_output || format == "json" {
        println!("{}", serde_json::to_string_pretty(&runtime)?);
    } else {
        println!("{}", serde_yaml::to_string(&runtime)?);
    }

    Ok(())
}

fn run_workflow(registry: Arc<Mutex<CommandRegistry>>, json_output: bool, matches: &ArgMatches) -> Result<()> {
    let mut state = WorkflowRunState::new(resolve_runtime_workflow(Arc::clone(&registry), matches)?);

    let history_store: Box<dyn HistoryStore> = match JsonHistoryStore::with_defaults() {
        Ok(store) => Box::new(store),
        Err(error) => {
            warn!(error = %error, "failed to initialize history store; using in-memory history");
            Box::new(InMemoryHistoryStore::new())
        }
    };

    seed_history_defaults_for_cli(&mut state, history_store.as_ref());

    if let Some(overrides) = matches.get_many::<String>("input") {
        for raw in overrides {
            let (key, value) = raw.split_once('=').context("workflow input overrides must use KEY=VALUE syntax")?;
            state.set_input_value(key.trim(), Value::String(value.trim().to_string()));
        }
    }

    state.apply_input_defaults();
    state.evaluate_input_providers()?;

    if let Some(blocked) = state
        .telemetry()
        .provider_resolution_events()
        .iter()
        .find(|event| matches!(event.outcome, ProviderBindingOutcome::Prompt(_) | ProviderBindingOutcome::Error(_)))
    {
        bail!(
            "provider argument {}.{} requires attention: {}",
            blocked.input,
            blocked.argument,
            describe_provider_outcome(&blocked.outcome)
        );
    }

    let registry_snapshot = {
        let guard = registry.lock().expect("could not obtain lock on registry");
        guard.clone()
    };

    let runner = RegistryCommandRunner::new(registry_snapshot);
    let results = state.execute_with_runner(&runner)?;
    let run_succeeded = results.iter().all(|result| result.status != StepStatus::Failed);

    if run_succeeded {
        persist_history_after_cli_run(&state, history_store.as_ref());
    }

    if json_output {
        output_workflow_json(&state, &results)?;
    } else {
        output_workflow_human(&state, &results);
    }

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use oatty_engine::{
        ArgumentPrompt, BindingFailure, BindingSource, MissingReason, ProviderResolutionEvent, ProviderResolutionSource, SkipDecision,
    };
    use oatty_types::workflow::{RuntimeWorkflow, WorkflowDefaultSource, WorkflowInputDefault, WorkflowInputDefinition};
    use serde_json::json;

    fn missing_reason(message: &str, path: Option<&str>) -> MissingReason {
        MissingReason {
            message: message.to_string(),
            path: path.map(ToString::to_string),
        }
    }

    #[test]
    fn describe_binding_source_formats_variants() {
        let step = BindingSource::Step { step_id: "deploy".into() };
        assert_eq!(describe_binding_source(&step), "step:deploy");

        let input = BindingSource::Input { input_name: "app".into() };
        assert_eq!(describe_binding_source(&input), "input:app");

        let combined = BindingSource::Multiple {
            step_id: "build".into(),
            input_name: "pipeline".into(),
        };
        assert_eq!(describe_binding_source(&combined), "step:build, input:pipeline");
    }

    #[test]
    fn describe_provider_outcome_handles_all_variants() {
        let resolved = ProviderBindingOutcome::Resolved(Value::String("demo".into()));
        assert_eq!(describe_provider_outcome(&resolved), "resolved to 'demo'");

        let prompt = ProviderBindingOutcome::Prompt(ArgumentPrompt {
            argument: "app".into(),
            source: BindingSource::Input { input_name: "app".into() },
            required: true,
            reason: missing_reason("needs user confirmation", Some("$.inputs.app")),
        });
        assert_eq!(
            describe_provider_outcome(&prompt),
            "prompted (required: true, reason: needs user confirmation)"
        );

        let skip = ProviderBindingOutcome::Skip(SkipDecision {
            argument: "region".into(),
            source: BindingSource::Step {
                step_id: "select-region".into(),
            },
            reason: missing_reason("not provided", None),
        });
        assert_eq!(describe_provider_outcome(&skip), "skipped (not provided)");

        let error = ProviderBindingOutcome::Error(BindingFailure {
            argument: "pipeline".into(),
            source: None,
            message: "api failure".into(),
        });
        assert_eq!(describe_provider_outcome(&error), "error: api failure");
    }

    #[test]
    fn provider_outcome_to_json_serializes_prompt_and_skip() {
        let prompt = ProviderBindingOutcome::Prompt(ArgumentPrompt {
            argument: "app".into(),
            source: BindingSource::Input { input_name: "app".into() },
            required: false,
            reason: missing_reason("needs value", Some("$.inputs.app")),
        });
        let prompt_json = provider_outcome_to_json(&prompt);
        assert_eq!(
            prompt_json,
            json!({
                "status": "prompt",
                "required": false,
                "reason": "needs value",
                "path": "$.inputs.app",
                "source": "input:app",
            })
        );

        let skip = ProviderBindingOutcome::Skip(SkipDecision {
            argument: "region".into(),
            source: BindingSource::Step { step_id: "select".into() },
            reason: missing_reason("missing", Some("$.steps[0]")),
        });
        let skip_json = provider_outcome_to_json(&skip);
        assert_eq!(
            skip_json,
            json!({
                "status": "skip",
                "reason": "missing",
                "path": "$.steps[0]",
                "source": "step:select",
            })
        );
    }

    #[test]
    fn provider_resolution_event_to_json_captures_source() {
        let event = ProviderResolutionEvent {
            input: "environment".into(),
            argument: "region".into(),
            outcome: ProviderBindingOutcome::Resolved(json!("us")),
            source: ProviderResolutionSource::Automatic,
        };
        let serialized = provider_resolution_event_to_json(&event);
        assert_eq!(
            serialized,
            json!({
                "input": "environment",
                "argument": "region",
                "source": "automatic",
                "outcome": {
                    "status": "resolved",
                    "value": "us"
                }
            })
        );
    }

    #[test]
    fn format_step_status_maps_enum_variants() {
        assert_eq!(format_step_status(StepStatus::Succeeded), "succeeded");
        assert_eq!(format_step_status(StepStatus::Failed), "failed");
        assert_eq!(format_step_status(StepStatus::Skipped), "skipped");
    }

    fn history_enabled_run_state() -> WorkflowRunState {
        let mut inputs = IndexMap::new();
        inputs.insert(
            "region".into(),
            WorkflowInputDefinition {
                default: Some(WorkflowInputDefault {
                    from: WorkflowDefaultSource::History,
                    value: None,
                }),
                ..Default::default()
            },
        );
        let workflow = RuntimeWorkflow {
            identifier: "deploy-app".into(),
            title: None,
            description: None,
            inputs,
            steps: Vec::new(),
        };
        WorkflowRunState::new(workflow)
    }

    fn history_key_for(state: &WorkflowRunState) -> HistoryKey {
        HistoryKey::workflow_input(DEFAULT_HISTORY_PROFILE, state.workflow.identifier.clone(), "region")
    }

    #[test]
    fn seed_history_defaults_for_cli_populates_run_context() {
        let mut run_state = history_enabled_run_state();
        let store = InMemoryHistoryStore::new();
        let key = history_key_for(&run_state);
        store.insert_value(key, json!("iad")).unwrap();

        assert!(run_state.run_context.inputs.get("region").is_none());
        seed_history_defaults_for_cli(&mut run_state, &store);
        assert_eq!(run_state.run_context.inputs.get("region"), Some(&json!("iad")));
    }

    #[test]
    fn persist_history_after_cli_run_saves_non_secret_values() {
        let store = InMemoryHistoryStore::new();
        let mut run_state = history_enabled_run_state();
        run_state.run_context.inputs.insert("region".into(), json!("iad"));

        persist_history_after_cli_run(&run_state, &store);
        let key = history_key_for(&run_state);
        let stored = store.get_latest_value(&key).unwrap().expect("value persisted");
        assert_eq!(stored.value, json!("iad"));
    }

    #[test]
    fn persist_history_after_cli_run_skips_secret_values() {
        let store = InMemoryHistoryStore::new();
        let mut run_state = history_enabled_run_state();
        let secret = json!("-----BEGIN PRIVATE KEY----- sensitive");
        assert!(value_contains_secret(&secret));
        run_state.run_context.inputs.insert("region".into(), secret);

        persist_history_after_cli_run(&run_state, &store);
        let key = history_key_for(&run_state);
        assert!(store.get_latest_value(&key).unwrap().is_none());
    }
}
