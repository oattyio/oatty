use anyhow::{anyhow, bail, Context, Result};
use clap::ArgMatches;
use heroku_api::HerokuClient;
use heroku_engine::{dry_run_plan, load_workflow_from_file};
use heroku_registry::{build_clap, Registry};
use heroku_util::redact_sensitive;
use reqwest::Method;
use serde_json::{json, to_string_pretty, Map, Value};
use std::{collections::HashMap, fs::read_dir, path::Path};
use tracing::Level;
use tracing_subscriber::fmt;

#[tokio::main]
/// Entrypoint for the CLI application.
///
/// This function serves as the main entry point for the Heroku CLI tool. It handles
/// command-line argument parsing and routes execution to either the TUI interface
/// or command execution mode.
///
/// # Behavior
/// - If no subcommands are provided, launches the TUI interface
/// - If workflow subcommands are provided (when FEATURE_WORKFLOWS=1), handles workflow operations
/// - Otherwise, executes the specified Heroku API command
///
/// # Returns
/// Returns `Result<()>` where `Ok(())` indicates successful execution and `Err` contains
/// any error that occurred during execution.
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
    let registry = Registry::from_embedded_schema()?;
    let cli = build_clap(&registry);
    let matches = cli.get_matches();

    // No subcommands => TUI
    if matches.subcommand_name().is_none() {
        return heroku_tui::run(registry).map(|_| ());
    }

    run_command(&registry, &matches).await
}

/// Initializes the tracing system for logging and diagnostics.
///
/// This function sets up the tracing subscriber with configuration based on the
/// `HEROKU_LOG` environment variable. It configures log levels and output formatting
/// for the application's diagnostic system.
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
    let filter = std::env::var("HEROKU_LOG").unwrap_or_else(|_| "info".into());
    let _ = fmt()
        .with_env_filter(filter)
        .with_max_level(Level::INFO)
        .try_init();
}

/// Executes a Heroku API command in CLI mode.
///
/// This function handles the execution of Heroku API commands when the CLI is run
/// with specific command arguments. It parses the command structure, builds the
/// appropriate HTTP request, and executes it against the Heroku API.
///
/// # Arguments
/// - `registry`: The command registry containing API endpoint specifications
/// - `matches`: Parsed command-line arguments from clap
///
/// # Command Structure
/// Commands follow the format: `<group> <qualified_subcommand>` (e.g., `apps app:create`)
/// where:
/// - `group`: The resource group (e.g., "apps", "dynos", "config")
/// - `qualified_subcommand`: The specific command within the group (e.g., "app:create", "list")
///
/// # Behavior
/// 1. Extracts the command group and subcommand from parsed arguments
/// 2. Looks up the command specification in the registry
/// 3. Collects positional arguments and flags from the command line
/// 4. Builds the HTTP request body from flags
/// 5. Constructs and executes the HTTP request to the Heroku API
/// 6. Handles dry-run mode by printing the request details instead of executing
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
/// # Create app with dry-run
/// heroku-cli apps app:create --name my-app --dry-run
///
/// # Set config var
/// heroku-cli config config:set KEY=value
/// ```
async fn run_command(registry: &Registry, matches: &ArgMatches) -> Result<()> {
    // format is <group> <qualified subcommand> e.g. apps app:create
    let (group, sub) = matches
        .subcommand()
        .context("expected a resource group subcommand")?;

    let (cmd_name, cmd_matches) = sub
        .subcommand()
        .context("expected a command under the group")?;

    // Route workflow commands via the registry so they are available in the TUI.
    if group == "workflow" {
        return run_workflow_cmd(registry, cmd_matches, Some(matches)).await;
    }

    let cmd_spec = registry.find_by_group_and_cmd(group, cmd_name)?;

    // Collect positional values
    let mut pos_values: HashMap<String, String> = HashMap::new();
    for key in &cmd_spec.positional_args {
        if let Some(val) = cmd_matches.get_one::<String>(key) {
            pos_values.insert(key.clone(), val.to_string());
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

    let body_value = (!body.is_empty()).then_some(Value::Object(body));

    let client = HerokuClient::new_from_env()?;
    let method = Method::from_bytes(cmd_spec.method.as_bytes())?;
    let path = resolve_path(&cmd_spec.path, &pos_values);
    let mut builder = client.request(method, &path);
    if let Some(ref b) = body_value {
        builder = builder.json(b);
    }

    let dry_run = matches.get_flag("dry-run");
    if dry_run {
        // Build request to inspect
        let req = builder.build()?;
        let url = req.url().to_string();
        let method = req.method().to_string();
        // Basic headers set; redact secrets
        let mut headers_out = Map::new();
        for (name, value) in req.headers().iter() {
            let val = value.to_str().unwrap_or("");
            let line = format!("{}: {}", name.as_str(), val);
            let redacted = redact_sensitive(&line);
            // Extract after ': '
            let out_val = redacted
                .splitn(2, ':')
                .nth(1)
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
            headers_out.insert(name.as_str().to_string(), Value::String(out_val));
        }
        let out = json!({
            "method": method,
            "url": url,
            "headers": headers_out,
            "body": body_value
        });
        println!("{}", to_string_pretty(&out)?);
        return Ok(());
    }

    // Execute
    let resp = builder.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    println!("{}\n{}", status, text);
    Ok(())
}

/// Resolves a path template by replacing placeholders with actual values.
/// The path template follow the same format as JSON hyper-schema URI
/// templates. See https://json-schema.org/draft/2019-09/json-schema-hypermedia#uriTemplating
///
/// This function takes a path template containing placeholders in the format `{key}`
/// and replaces them with corresponding values from the provided HashMap.
///
/// # Arguments
/// - `template`: A string containing path placeholders in the format `{key}`
/// - `pos`: A HashMap mapping placeholder keys to their replacement values
///
/// # Returns
/// Returns a `String` with all placeholders replaced by their corresponding values.
/// If a placeholder key is not found in the HashMap, it remains unchanged in the output.
///
/// # Examples
/// ```
/// use std::collections::HashMap;
///
/// let template = "/apps/{app}/dynos/{dyno}";
/// let mut pos = HashMap::new();
/// pos.insert("app".to_string(), "my-app".to_string());
/// pos.insert("dyno".to_string(), "web.1".to_string());
///
/// let result = resolve_path(template, &pos);
/// assert_eq!(result, "/apps/my-app/dynos/web.1");
///
/// // Missing placeholder remains unchanged
/// let template = "/apps/{app}/config/{missing}";
/// let mut pos = HashMap::new();
/// pos.insert("app".to_string(), "my-app".to_string());
///
/// let result = resolve_path(template, &pos);
/// assert_eq!(result, "/apps/my-app/config/{missing}");
/// ```
fn resolve_path(template: &str, pos: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in pos {
        let needle = format!("{{{}}}", k);
        out = out.replace(&needle, v);
    }
    out
}

/// Checks if the workflow feature is enabled via environment variable.
///
/// This function determines whether workflow functionality should be available
/// by checking the `FEATURE_WORKFLOWS` environment variable. This allows for
/// feature flagging of workflow capabilities.
///
/// # Environment Variables
/// - `FEATURE_WORKFLOWS`: Controls workflow feature availability
///   - `"1"` or `"true"` (case-insensitive): Enables workflows
///   - Any other value or unset: Disables workflows
///
/// # Returns
/// Returns `true` if workflows are enabled, `false` otherwise.
///
/// # Examples
/// ```bash
/// # Enable workflows
/// FEATURE_WORKFLOWS=1 cargo run
///
/// # Enable workflows (alternative)
/// FEATURE_WORKFLOWS=true cargo run
///
/// # Disable workflows (default)
/// cargo run
/// ```
fn feature_workflows() -> bool {
    std::env::var("FEATURE_WORKFLOWS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Adds workflow subcommands to the CLI if the feature is enabled.
///
/// This function conditionally extends the CLI with workflow-related subcommands
/// when the `FEATURE_WORKFLOWS` environment variable is set. If workflows are
/// disabled, it returns the original CLI command unchanged.
///
/// # Arguments
/// - `root`: The base CLI command to extend with workflow subcommands
///
/// # Returns
/// Returns a `clap::Command` that includes workflow subcommands if enabled,
/// or the original command if workflows are disabled.
///
/// # Workflow Subcommands
/// When enabled, adds the following subcommands:
/// - `list`: Lists available workflows in the workflows/ directory
/// - `preview`: Previews a workflow plan without executing it
///   - `--file, -f`: Path to workflow YAML/JSON file
///   - `--name`: Workflow name within the file
/// - `run`: Executes a workflow
///   - `--file, -f`: Path to workflow YAML/JSON file
///   - `--name`: Workflow name within the file
///   - `--dry-run`: Preview the execution plan without running
///
/// # Examples
/// ```bash
/// # List workflows (if enabled)
/// heroku-cli workflow list
///
/// # Preview workflow
/// heroku-cli workflow preview --file workflows/my-workflow.yaml --name create-app
///
/// # Run workflow with dry-run
/// heroku-cli workflow run --file workflows/my-workflow.yaml --name create-app --dry-run
/// ```
// Workflow command tree is now injected via the registry; no separate CLI wiring needed.

/// Executes workflow-related commands.
///
/// This function handles the execution of workflow subcommands when the workflow
/// feature is enabled. It provides functionality for listing, previewing, and
/// running workflows defined in YAML/JSON files.
///
/// # Arguments
/// - `registry`: The command registry containing API endpoint specifications
/// - `m`: Parsed command-line arguments for workflow subcommands
///
/// # Subcommands
///
/// ## `list`
/// Lists all workflow files found in the `workflows/` directory.
///
/// ## `preview`
/// Loads a workflow from a file and generates a dry-run plan showing what
/// actions would be taken without actually executing them.
///
/// ### Arguments
/// - `--file, -f`: Path to the workflow YAML/JSON file (defaults to `workflows/create_app_and_db.yaml`)
/// - `--name`: Name of the workflow within the file (defaults to the first workflow if not specified)
///
/// ## `run`
/// Loads and executes a workflow. Currently only supports dry-run mode.
///
/// ### Arguments
/// - `--file, -f`: Path to the workflow YAML/JSON file (defaults to `workflows/create_app_and_db.yaml`)
/// - `--name`: Name of the workflow within the file (defaults to the first workflow if not specified)
/// - `--dry-run`: Preview the execution plan without running (currently required)
///
/// # Returns
/// Returns `Result<()>` where `Ok(())` indicates successful execution and `Err`
/// contains any error that occurred during workflow processing.
///
/// # Examples
/// ```bash
/// # List available workflows
/// heroku-cli workflow list
///
/// # Preview a workflow
/// heroku-cli workflow preview --file workflows/my-workflow.yaml --name create-app
///
/// # Run workflow in dry-run mode
/// heroku-cli workflow run --file workflows/my-workflow.yaml --name create-app --dry-run
/// ```
async fn run_workflow_cmd(
    registry: &heroku_registry::Registry,
    m: &ArgMatches,
    root: Option<&ArgMatches>,
) -> Result<()> {
    if !feature_workflows() {
        bail!("workflows are disabled; set FEATURE_WORKFLOWS=1");
    }
    match m.subcommand() {
        Some(("list", _)) => {
            let dir = Path::new("workflows");
            if dir.exists() {
                for entry in read_dir(dir)? {
                    let e = entry?;
                    println!("{}", e.path().display());
                }
            } else {
                println!("No workflows directory found");
            }
        }
        Some(("preview", sub)) => {
            let file = sub
                .get_one::<String>("file")
                .map(|s| s.as_str())
                .unwrap_or("workflows/create_app_and_db.yaml");
            let workflow_file = load_workflow_from_file(file)?;
            let name = sub
                .get_one::<String>("name")
                .map(|s| s.as_str())
                .or_else(|| workflow_file.workflows.keys().next().map(|s| s.as_str()))
                .ok_or_else(|| anyhow!("no workflow name provided and file has none"))?;
            let workflow = workflow_file
                .workflows
                .get(name)
                .ok_or_else(|| anyhow!("workflow '{}' not found in file", name))?;
            let plan = dry_run_plan(workflow, registry).await?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        Some(("run", sub)) => {
            let file = sub
                .get_one::<String>("file")
                .map(|s| s.as_str())
                .unwrap_or("workflows/create_app_and_db.yaml");
            let wf_file = load_workflow_from_file(file)?;
            let name = sub
                .get_one::<String>("name")
                .map(|s| s.as_str())
                .or_else(|| wf_file.workflows.keys().next().map(|s| s.as_str()))
                .ok_or_else(|| anyhow!("no workflow name provided and file has none"))?;
            let wf = wf_file
                .workflows
                .get(name)
                .ok_or_else(|| anyhow!("workflow '{}' not found in file", name))?;
            let dry =
                sub.get_flag("dry-run") || root.map(|r| r.get_flag("dry-run")).unwrap_or(false);
            if dry {
                let plan = dry_run_plan(wf, registry).await?;
                println!("{}", to_string_pretty(&plan)?);
            } else {
                println!("Workflow run not implemented yet; use --dry-run");
            }
        }
        _ => {
            println!("Available subcommands: list, preview, run");
        }
    }
    Ok(())
}
