use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};
use clap::ArgMatches;
use heroku_api::HerokuClient;
use heroku_engine::{
    ProviderBindingOutcome, ProviderResolutionEvent, ProviderResolutionSource, RegistryCommandRunner, StepResult, StepStatus,
    WorkflowRunState, build_runtime_catalog, runtime_workflow_from_definition,
};
use heroku_mcp::{PluginEngine, config::load_config};
use heroku_registry::{Registry, build_clap, feat_gate::feature_workflows, find_by_group_and_cmd};
use heroku_types::{ExecOutcome, command::CommandExecution, service::ServiceId, workflow::WorkflowDefinition};
use heroku_util::resolve_path;
use reqwest::Method;
use serde_json::{Map, Value, json};
use serde_yaml;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
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
/// This function serves as the main entry point for the Heroku CLI tool. It
/// handles command-line argument parsing and routes execution to either the TUI
/// interface or command execution mode.
///
/// # Behavior
/// - If no subcommands are provided, launches the TUI interface
/// - If workflow subcommands are provided (when FEATURE_WORKFLOWS=1), handles
///   workflow operations
/// - Otherwise, executes the specified Heroku API command
///
/// # Returns
/// Returns `Result<()>` where `Ok(())` indicates successful execution and `Err`
/// contains any error that occurred during execution.
///
/// # Examples
/// ```bash
/// # Launch TUI
/// heroku-cli
///
/// # Execute command
/// heroku-cli apps list
///
/// # Workflow command (if enabled)
/// heroku-cli workflow list
/// ```
async fn main() -> Result<()> {
    init_tracing();
    let cfg = load_config()?;
    let registry = Arc::new(Mutex::new(Registry::from_embedded_schema()?));
    let plugin_engine = Arc::new(PluginEngine::new(cfg, Arc::clone(&registry))?);
    plugin_engine.prepare_registry().await?;
    plugin_engine.start().await?;

    let cli = build_clap(Arc::clone(&registry));
    let matches = cli.get_matches();

    // No subcommands => TUI
    if matches.subcommand_name().is_none() {
        // Silence tracing output to stderr while the TUI is active to avoid overlay
        TUI_ACTIVE.store(true, Ordering::Relaxed);
        let tui_result = heroku_tui::run(Arc::clone(&registry), Arc::clone(&plugin_engine)).await;
        TUI_ACTIVE.store(false, Ordering::Relaxed);
        plugin_engine.stop().await?;
        return tui_result;
    }

    let result = run_command(Arc::clone(&registry), &matches, Arc::clone(&plugin_engine)).await;
    plugin_engine.stop().await?;
    result
}

/// Initializes the tracing system for logging and diagnostics.
///
/// This function sets up the tracing subscriber with configuration based on the
/// `HEROKU_LOG` environment variable. It configures log levels and output
/// formatting for the application's diagnostic system.
///
/// # Environment Variables
/// - `HEROKU_LOG`: Controls the logging level. Valid values are:
///   - `error`: Only error messages
///   - `warn`: Warning and error messages
///   - `info`: Info, warning, and error messages (default)
///   - `debug`: Debug, info, warning, and error messages
///   - `trace`: All log levels
///
/// # Behavior
/// - Reads the `HEROKU_LOG` environment variable
/// - Defaults to "info" level if not set or invalid
/// - Configures the tracing subscriber with the specified filter
/// - Sets maximum log level to `Level::INFO`
///
/// # Examples
/// ```bash
/// # Set debug logging
/// HEROKU_LOG=debug cargo run
///
/// # Set error-only logging
/// HEROKU_LOG=error cargo run
/// ```
fn init_tracing() {
    // Respect HEROKU_LOG without imposing a lower max level ceiling.
    // Example: HEROKU_LOG=debug will now allow `tracing::debug!` to emit.
    let filter = std::env::var("HEROKU_LOG").unwrap_or_else(|_| "info".into());
    let _ = fmt().with_env_filter(filter).with_writer(|| GatedStderr).try_init();
}

fn resolve_runtime_workflow(registry: Arc<Mutex<Registry>>, matches: &ArgMatches) -> Result<heroku_engine::RuntimeWorkflow> {
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

fn load_runtime_workflow_from_file(path: &Path) -> Result<heroku_engine::RuntimeWorkflow> {
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

fn describe_binding_source(source: &heroku_engine::BindingSource) -> String {
    match source {
        heroku_engine::BindingSource::Step { step_id } => format!("step:{step_id}"),
        heroku_engine::BindingSource::Input { input_name } => format!("input:{input_name}"),
        heroku_engine::BindingSource::Multiple { step_id, input_name } => {
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

/// Executes a Heroku API command in CLI mode.
///
/// This function handles the execution of Heroku API commands when the CLI is
/// run with specific command arguments. It parses the command structure, builds
/// the appropriate HTTP request, and executes it against the Heroku API.
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
/// 5. Constructs and executes the HTTP request to the Heroku API
/// 7. Outputs the response to stdout
///
/// # Returns
/// Returns `Result<()>` where `Ok(())` indicates successful command execution
/// and `Err` contains any error that occurred during processing.
///
/// # Examples
/// ```bash
/// # List apps
/// heroku-cli apps list
///
/// heroku-cli apps app:create --name my-app
///
/// # Set config var
/// heroku-cli config config:set KEY=value
/// ```
async fn run_command(registry: Arc<Mutex<Registry>>, matches: &ArgMatches, plugin_engine: Arc<PluginEngine>) -> Result<()> {
    // format is <group> <qualified subcommand> e.g. apps app:create
    let (group, sub) = matches.subcommand().context("expected a resource group subcommand")?;

    let (cmd_name, cmd_matches) = sub.subcommand().context("expected a command under the group")?;

    if group == "workflow" {
        return handle_workflow_command(Arc::clone(&registry), matches, cmd_name, cmd_matches);
    }

    let cmd_spec = {
        let registry_lock = registry.lock().expect("could not obtain lock on registry");
        find_by_group_and_cmd(&registry_lock.commands, group, cmd_name)?
    };

    // Collect positional values
    let mut pos_values: HashMap<String, String> = HashMap::new();
    for pa in &cmd_spec.positional_args {
        if let Some(val) = cmd_matches.get_one::<String>(&pa.name) {
            pos_values.insert(pa.name.clone(), val.to_string());
        }
    }

    // Collect flags as body fields
    let mut body = Map::new();
    for f in &cmd_spec.flags {
        if f.r#type == "boolean" {
            if cmd_matches.get_flag(&f.name) {
                body.insert(f.name.clone(), Value::Bool(true));
            }
        } else if let Some(val) = cmd_matches.get_one::<String>(&f.name) {
            body.insert(f.name.clone(), Value::String(val.to_string()));
        }
    }

    let body_value = (!body.is_empty()).then_some(Value::Object(body.clone()));

    match cmd_spec.execution() {
        CommandExecution::Http(http) => {
            let client = HerokuClient::new_from_service_id(http.service_id)?;
            let method = Method::from_bytes(http.method.as_bytes())?;
            let path = resolve_path(&http.path, &pos_values);
            let mut builder = client.request(method, &path);
            if let Some(ref b) = body_value {
                builder = builder.json(b);
            }

            let resp = builder.send().await?;
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            println!("{}\n{}", status, text);
            Ok(())
        }
        CommandExecution::Mcp(_) => {
            let mut arguments = body;
            for pa in &cmd_spec.positional_args {
                if let Some(value) = pos_values.get(&pa.name) {
                    arguments.insert(pa.name.clone(), Value::String(value.clone()));
                }
            }

            let outcome = plugin_engine.execute_tool(&cmd_spec, &arguments).await?;
            match outcome {
                ExecOutcome::Mcp(log, _) => println!("{}", log),
                ExecOutcome::Log(log) => println!("{}", log),
                other => println!("{:?}", other),
            }
            Ok(())
        }
    }
}

fn handle_workflow_command(
    registry: Arc<Mutex<Registry>>,
    root_matches: &ArgMatches,
    subcommand: &str,
    sub_matches: &ArgMatches,
) -> Result<()> {
    if !feature_workflows() {
        bail!("Workflows feature is disabled. Set FEATURE_WORKFLOWS=1 to enable.");
    }

    let json_output = root_matches.get_flag("json");

    match subcommand {
        "list" => list_workflows(registry, json_output),
        "preview" => preview_workflow(registry, json_output, sub_matches),
        "run" => run_workflow(registry, json_output, sub_matches),
        other => bail!("Unsupported workflow subcommand: {other}"),
    }
}

fn list_workflows(registry: Arc<Mutex<Registry>>, json_output: bool) -> Result<()> {
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

fn preview_workflow(registry: Arc<Mutex<Registry>>, json_output: bool, matches: &ArgMatches) -> Result<()> {
    let runtime = resolve_runtime_workflow(registry, matches)?;
    let format = matches.get_one::<String>("format").map(|s| s.as_str()).unwrap_or("yaml");

    if json_output || format == "json" {
        println!("{}", serde_json::to_string_pretty(&runtime)?);
    } else {
        println!("{}", serde_yaml::to_string(&runtime)?);
    }

    Ok(())
}

fn run_workflow(registry: Arc<Mutex<Registry>>, json_output: bool, matches: &ArgMatches) -> Result<()> {
    let mut state = WorkflowRunState::new(resolve_runtime_workflow(Arc::clone(&registry), matches)?);

    if let Some(overrides) = matches.get_many::<String>("input") {
        for raw in overrides {
            let (key, value) = raw.split_once('=').context("workflow input overrides must use KEY=VALUE syntax")?;
            state.set_input_value(key.trim(), Value::String(value.trim().to_string()));
        }
    }

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

    let client = HerokuClient::new_from_service_id(ServiceId::CoreApi)?;
    let runner = RegistryCommandRunner::new(registry_snapshot, client);
    let results = state.execute_with_runner(&runner);

    if json_output {
        output_workflow_json(&state, &results)?;
    } else {
        output_workflow_human(&state, &results);
    }

    Ok(())
}
