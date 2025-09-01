use std::collections::HashMap;

use anyhow::{Context, Result};
use clap::ArgMatches;
use heroku_api::HerokuClient;
use heroku_registry::{Registry, build_clap};
use reqwest::Method;
use serde_json::{Map, Value};
use tracing_subscriber::fmt;

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
    let registry = Registry::from_embedded_schema()?;
    let cli = build_clap(&registry);
    let matches = cli.get_matches();

    // No subcommands => TUI
    if matches.subcommand_name().is_none() {
        heroku_tui::run(registry).await?;
        return Ok(());
    }

    run_command(&registry, &matches).await
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
    let _ = fmt().with_env_filter(filter).try_init();
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
async fn run_command(registry: &Registry, matches: &ArgMatches) -> Result<()> {
    // format is <group> <qualified subcommand> e.g. apps app:create
    let (group, sub) = matches.subcommand().context("expected a resource group subcommand")?;

    let (cmd_name, cmd_matches) = sub.subcommand().context("expected a command under the group")?;

    // Route workflow commands via the registry so they are available in the TUI.
    if group == "workflow" {
        return Ok(()); // unimplemented
    }

    let cmd_spec = registry.find_by_group_and_cmd(group, cmd_name)?;

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

    let body_value = (!body.is_empty()).then_some(Value::Object(body));

    let client = HerokuClient::new_from_env()?;
    let method = Method::from_bytes(cmd_spec.method.as_bytes())?;
    let path = resolve_path(&cmd_spec.path, &pos_values);
    let mut builder = client.request(method, &path);
    if let Some(ref b) = body_value {
        builder = builder.json(b);
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
/// This function takes a path template containing placeholders in the format
/// `{key}` and replaces them with corresponding values from the provided
/// HashMap.
///
/// # Arguments
/// - `template`: A string containing path placeholders in the format `{key}`
/// - `pos`: A HashMap mapping placeholder keys to their replacement values
///
/// # Returns
/// Returns a `String` with all placeholders replaced by their corresponding
/// values. If a placeholder key is not found in the HashMap, it remains
/// unchanged in the output.
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
