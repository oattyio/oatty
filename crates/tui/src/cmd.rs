//! # Command Execution Layer
//!
//! This module translates high-level application effects (`Effect`) into
//! imperative commands (`Cmd`) and executes them. It provides the "boundary"
//! where the pure state management of the app interacts with side effects
//! such as
//! - Writing to the system clipboard
//! - Making live API calls to Heroku
//! - Spawning background tasks and recording logs
//!
//! ## Design
//! - [`Cmd`] is the effectful command type (clipboard / http).
//! - [`run_from_effects`] translates state-driven [`Effect`]s into [`Cmd`]s.
//! - [`run_cmds`] takes these commands and executes them, ensuring logs remain
//!   user-visible.
//! - [`execute_http`] and [`exec_remote`] handle async HTTP requests and return
//!   structured [`ExecOutcome`] for UI presentation.
//!
//! This design follows a **functional core, imperative shell** pattern:
//! state updates are pure, but commands handle side effects.

use anyhow::Result;
use anyhow::anyhow;
use heroku_mcp::config::default_config_path;
use heroku_mcp::config::save_config_to_path;
use heroku_mcp::config::validate_config;
use heroku_mcp::config::validate_server_name;
use heroku_mcp::config::{McpServer, determine_env_source};
use heroku_mcp::{McpConfig, PluginEngine};
use heroku_registry::CommandSpec;
use heroku_registry::find_by_group_and_cmd;
use heroku_types::{Effect, EnvVar, Modal, Route};
use heroku_types::{ExecOutcome, command::CommandExecution};
use heroku_util::build_range_header_from_body;
use heroku_util::exec_remote;
use heroku_util::lex_shell_like;
use heroku_util::resolve_path;
use reqwest::Url;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use serde_json::from_str;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::vec;
use tokio::task::JoinHandle;

use crate::app::App;
use crate::ui::components::logs::state::LogEntry;
use crate::ui::components::plugins::EnvRow;
use crate::ui::components::plugins::PluginTransport;

/// Represents side-effectful system commands executed outside pure state
/// updates.
///
/// These commands bridge between the application's functional state model
/// and imperative actions (I/O, networking, system integration).
#[derive(Debug)]
pub enum Cmd {
    ApplyPaletteError(String),
    /// Write text into the system clipboard.
    ///
    /// # Example
    /// ```rust,ignore
    /// use your_crate::Cmd;
    /// let cmd = Cmd::ClipboardSet("hello".into());
    /// match cmd {
    ///     Cmd::ClipboardSet(text) => assert_eq!(text, "hello"),
    ///     _ => panic!("unexpected variant"),
    /// }
    /// ```
    ClipboardSet(String),

    /// Make an HTTP request to the Heroku API.
    ///
    /// Carries:
    /// - [`CommandSpec`]: API request metadata (including path, method, and service)
    /// - `serde_json::Map`: JSON body
    ///
    /// # Example
    /// ```rust,ignore
    /// use your_crate::Cmd;
    /// use heroku_registry::{CommandSpec, HttpCommandSpec, ServiceId};
    /// use heroku_types::CommandExecution;
    /// use std::collections::HashMap;
    /// use serde_json::{Map, Value};
    ///
    /// let http = HttpCommandSpec::new("GET", "/apps", ServiceId::CoreApi, Vec::new());
    /// let spec = CommandSpec::new_http(
    ///     "apps".into(),
    ///     "apps:list".into(),
    ///     "List apps".into(),
    ///     Vec::new(),
    ///     Vec::new(),
    ///     http,
    /// );
    /// let cmd = Cmd::ExecuteHttp(spec.clone(), Map::new());
    ///
    /// if let Cmd::ExecuteHttp(s, b) = cmd {
    ///     assert!(matches!(s.execution(), CommandExecution::Http(_)));
    ///     assert!(b.is_empty());
    /// }
    /// ```
    ExecuteHttp(CommandSpec, Map<String, Value>),
    /// Invoke an MCP tool via the plugin engine.
    ExecuteMcp(CommandSpec, Map<String, Value>),
    /// Load MCP plugins from config (synchronous file read) and populate UI state.
    LoadPlugins,
    PluginsStart(String),
    PluginsStop(String),
    PluginsRestart(String),
    PluginsLoadDetail(String),
    PluginsRefresh,
    PluginsExportLogsDefault(String),
    PluginsValidate,
    PluginsSave,
}

/// Collection of immediate and background work generated while handling effects.
#[derive(Default)]
pub struct CommandBatch {
    /// Outcomes that completed synchronously.
    pub immediate: Vec<ExecOutcome>,
    /// Background tasks still running.
    pub pending: Vec<JoinHandle<ExecOutcome>>,
}

/// Convert application [`Effect`]s into actual [`Cmd`] instances and dispatch
/// the resulting work.
///
/// This maintains a clean separation: effects represent "what should happen",
/// while commands describe "how it should happen". Synchronous commands yield
/// `ExecOutcome`s immediately; long-running commands are spawned so the caller
/// can poll them later.
pub async fn run_from_effects(app: &mut App<'_>, effects: Vec<Effect>) -> CommandBatch {
    let mut commands = Vec::new();

    for effect in effects {
        let effect_commands = match effect {
            Effect::CopyToClipboardRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
            Effect::CopyLogsRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
            Effect::NextPageRequested(next_raw) => handle_next_page_requested(app, next_raw),
            Effect::PrevPageRequested => handle_prev_page_requested(app),
            Effect::FirstPageRequested => handle_first_page_requested(app),
            Effect::LastPageRequested => handle_last_page_requested(app),
            Effect::PluginsLoadRequested => Some(vec![Cmd::LoadPlugins]),
            Effect::PluginsStart(name) => Some(vec![Cmd::PluginsStart(name)]),
            Effect::PluginsStop(name) => Some(vec![Cmd::PluginsStop(name)]),
            Effect::PluginsRestart(name) => Some(vec![Cmd::PluginsRestart(name)]),
            Effect::PluginsLoadDetail(name) => {
                let state = app.plugins.ensure_details_state();
                state.begin_load(name.clone());
                Some(vec![Cmd::PluginsLoadDetail(name)])
            }
            Effect::PluginsRefresh => Some(vec![Cmd::PluginsRefresh]),
            Effect::PluginsExportLogsDefault(name) => Some(vec![Cmd::PluginsExportLogsDefault(name)]),
            Effect::PluginsOpenAdd => Some(vec![]),
            Effect::PluginsValidateAdd => Some(vec![Cmd::PluginsValidate]),
            Effect::PluginsApplyAdd => Some(vec![Cmd::PluginsSave]),
            Effect::ShowModal(modal) => handle_show_modal(app, modal),
            Effect::CloseModal => handle_close_modal(app),
            Effect::SwitchTo(route) => handle_switch_to(app, route),
            Effect::SendToPalette(spec) => handle_send_to_palette(app, spec),
            Effect::Run => start_palette_execution(app),
        };
        if let Some(cmds) = effect_commands {
            commands.extend(cmds);
        }
    }

    run_cmds(app, commands).await
}

/// Handle the next page requested effect by executing the previous command with updated pagination.
///
/// Updates the pagination history and creates an HTTP execution command with the new range.
///
/// # Arguments
/// * `app` - The application state containing previous command context
/// * `next_raw` - The raw next-range value for the Range header
///
/// # Returns
/// A vector containing an HTTP execution command if context is available
fn handle_next_page_requested(app: &mut App, next_raw: String) -> Option<Vec<Cmd>> {
    let mut commands = Vec::new();

    if let (Some(spec), Some(mut body)) = (app.last_spec.clone(), app.last_body.clone()) {
        // Inject raw next-range override for Range header
        body.insert("next-range".into(), Value::String(next_raw.clone()));

        // Append to history for Prev/First navigation
        app.pagination_history.push(Some(next_raw));

        commands.push(Cmd::ExecuteHttp(spec, body));
    } else {
        app.logs.entries.push("Cannot request next page: no prior command context".into());
    }

    Some(commands)
}

/// Handle the previous page requested effect by navigating back in pagination history.
///
/// Updates the pagination history and creates an HTTP execution command with the previous range.
///
/// # Arguments
/// * `app` - The application state containing previous command context and pagination history
///
/// # Returns
/// A vector containing an HTTP execution command if navigation is possible
fn handle_prev_page_requested(app: &mut App) -> Option<Vec<Cmd>> {
    let mut commands = Vec::new();

    if let (Some(spec), Some(mut body)) = (app.last_spec.clone(), app.last_body.clone()) {
        if app.pagination_history.len() <= 1 {
            // No previous page to go to
            return Some(commands);
        }

        // Remove current page from history
        let _ = app.pagination_history.pop();

        // Navigate to previous page
        if let Some(prev) = app.pagination_history.last().cloned().flatten() {
            body.insert("next-range".into(), Value::String(prev));
        } else {
            let _ = body.remove("next-range");
        }

        commands.push(Cmd::ExecuteHttp(spec, body));
    } else {
        app.logs
            .entries
            .push("Cannot request previous page: no prior command context".into());
    }

    Some(commands)
}

/// Handle the first page requested effect by navigating to the beginning of pagination history.
///
/// Resets the pagination history to the first entry and creates an HTTP execution command.
///
/// # Arguments
/// * `app` - The application state containing previous command context and pagination history
///
/// # Returns
/// A vector containing an HTTP execution command if context is available
fn handle_first_page_requested(app: &mut App) -> Option<Vec<Cmd>> {
    let mut commands = Vec::new();

    if let (Some(spec), Some(mut body)) = (app.last_spec.clone(), app.last_body.clone()) {
        // Get the first page range if available
        if let Some(first) = app.pagination_history.first().cloned().flatten() {
            body.insert("next-range".into(), Value::String(first));
        } else {
            let _ = body.remove("next-range");
        }

        // Reset history to the first entry
        let first_opt = app.pagination_history.first().cloned().flatten();
        app.pagination_history.clear();
        app.pagination_history.push(first_opt);

        commands.push(Cmd::ExecuteHttp(spec, body));
    } else {
        app.logs.entries.push("Cannot request first page: no prior command context".into());
    }

    Some(commands)
}

/// Handle the last page requested effect by attempting to navigate to the final page.
fn handle_last_page_requested(app: &mut App) -> Option<Vec<Cmd>> {
    let next_range = app.last_pagination.as_ref().and_then(|pagination| pagination.next_range.clone());

    match next_range {
        Some(range) => handle_next_page_requested(app, range),
        None => Some(Vec::new()),
    }
}

/// Handle the show modal effect by setting the appropriate modal component.
///
/// # Arguments
/// * `app` - The application state
/// * `modal` - The modal type to show
///
/// # Returns
/// A vector containing no commands (modal changes are direct UI state updates)
fn handle_show_modal(app: &mut App, modal: Modal) -> Option<Vec<Cmd>> {
    if matches!(modal, Modal::PluginDetails) {
        app.plugins.ensure_details_state();
    }
    app.set_open_modal_kind(Some(modal));
    None // No commands needed for direct UI state update
}

/// Handle the close modal effect by clearing the open modal.
///
/// # Arguments
/// * `app` - The application state
///
/// # Returns
/// A vector containing no commands (modal changes are direct UI state updates)
fn handle_close_modal(app: &mut App) -> Option<Vec<Cmd>> {
    // retain focus so it can be restored when the modal closes
    app.set_open_modal_kind(None);
    None // No commands needed for direct UI state update
}

/// Handle the switch to route effect by setting the appropriate main view.
///
/// # Arguments
/// * `app` - The application state
/// * `route` - The route to switch to
///
/// # Returns
/// A vector containing no commands (view changes are direct UI state updates)
fn handle_switch_to(app: &mut App, route: Route) -> Option<Vec<Cmd>> {
    app.set_current_route(route);
    Some(vec![])
}

/// When pressing Enter in the browser, populate the palette with the
/// constructed command and close the command browser.
fn handle_send_to_palette(app: &mut App, command_spec: CommandSpec) -> Option<Vec<Cmd>> {
    let name = command_spec.name;
    let group = command_spec.group;

    app.palette.set_input(format!("{} {}", group, name));
    app.palette.set_cursor(app.palette.input().len());
    app.palette.apply_build_suggestions(&app.ctx.providers, &*app.ctx.theme);
    Some(vec![])
}

/// Execute a sequence of commands, splitting completed outcomes from spawned
/// background work.
///
/// Each command corresponds to a user-visible side effect, such as writing
/// content to the clipboard or making a network call. Commands that can finish
/// synchronously push their [`ExecOutcome`]s immediately, while long-running
/// commands return `JoinHandle`s so the caller can poll them later without
/// blocking the UI loop.
pub async fn run_cmds(app: &mut App<'_>, commands: Vec<Cmd>) -> CommandBatch {
    let mut batch = CommandBatch::default();
    for command in commands {
        let outcome = match command {
            Cmd::ApplyPaletteError(error) => apply_palette_error(app, error),
            Cmd::ClipboardSet(text) => execute_clipboard_set(app, text),
            Cmd::ExecuteHttp(spec, body) => {
                batch.pending.push(spawn_execute_http(app, spec, body));
                continue;
            }
            Cmd::ExecuteMcp(spec, body) => {
                batch.pending.push(spawn_execute_mcp(app, spec, body));
                continue;
            }
            Cmd::PluginsStart(name) => {
                batch.pending.push(spawn_execute_plugin_action(app, PluginAction::Start, name));
                continue;
            }
            Cmd::PluginsStop(name) => {
                batch.pending.push(spawn_execute_plugin_action(app, PluginAction::Stop, name));
                continue;
            }
            Cmd::PluginsRestart(name) => {
                batch.pending.push(spawn_execute_plugin_action(app, PluginAction::Restart, name));
                continue;
            }
            Cmd::PluginsLoadDetail(name) => {
                batch.pending.push(spawn_load_plugin_detail(app, name));
                continue;
            }
            Cmd::LoadPlugins => execute_load_plugins(app).await,
            Cmd::PluginsRefresh => execute_plugins_refresh(app).await,
            Cmd::PluginsExportLogsDefault(name) => execute_plugins_export_default(app, name).await,
            Cmd::PluginsValidate => execute_plugins_validate(app),
            Cmd::PluginsSave => execute_plugins_save(app).await,
        };
        batch.immediate.push(outcome);
    }
    batch
}

/// Execute a clipboard set command by writing text to the system clipboard.
///
/// Updates the application logs with success or error messages and maintains
/// log size limits for performance.
///
/// # Arguments
/// * `app` - The application state for logging
/// * `text` - The text content to write to the clipboard
fn execute_clipboard_set(app: &mut App, text: String) -> ExecOutcome {
    // Perform clipboard write and log outcome
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
        Ok(()) => {
            // Success - could add success log here if desired
        }
        Err(e) => {
            app.logs.entries.push(format!("Clipboard error: {}", e));
        }
    }

    // Limit log size for performance
    let log_len = app.logs.entries.len();
    if log_len > 500 {
        let _ = app.logs.entries.drain(0..log_len - 500);
    }

    ExecOutcome::default()
}

fn apply_palette_error(app: &mut App, error: String) -> ExecOutcome {
    app.palette.apply_error(error);
    ExecOutcome::default()
}

/// Load MCP plugins from the user's config file and populate the PluginsState list.
async fn execute_load_plugins(app: &mut App<'_>) -> ExecOutcome {
    let plugin_engine = app.ctx.plugin_engine.clone();
    let mut plugin_details = plugin_engine.list_plugins().await;
    plugin_details.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
    app.plugins.table.replace_items(plugin_details);

    ExecOutcome::default()
}

#[derive(Clone, Copy)]
enum PluginAction {
    Start,
    Stop,
    Restart,
}

/// Execute a plugin lifecycle action using the MCP supervisor.
fn spawn_execute_plugin_action(app: &mut App<'_>, action: PluginAction, name: String) -> JoinHandle<ExecOutcome> {
    app.executing = true;
    app.throbber_idx = 0;

    let plugin_engine = app.ctx.plugin_engine.clone();
    tokio::spawn(async move {
        let client_mgr = plugin_engine.client_manager();
        let res = match action {
            PluginAction::Start => client_mgr.start_plugin(&name).await,
            PluginAction::Stop => client_mgr.stop_plugin(&name).await,
            PluginAction::Restart => client_mgr.restart_plugin(&name).await,
        };
        let msg = match (action, res) {
            (PluginAction::Start, Ok(_)) => format!("Plugins: started '{}'", name),
            (PluginAction::Stop, Ok(_)) => format!("Plugins: stopped '{}'", name),
            (PluginAction::Restart, Ok(_)) => format!("Plugins: restarted '{}'", name),
            (PluginAction::Start, Err(e)) => format!("Plugins: start '{}' failed: {}", name, e),
            (PluginAction::Stop, Err(e)) => format!("Plugins: stop '{}' failed: {}", name, e),
            (PluginAction::Restart, Err(e)) => format!("Plugins: restart '{}' failed: {}", name, e),
        };
        let detail_result = plugin_engine.get_plugin_detail(&name).await.ok();
        ExecOutcome::PluginDetail(msg, detail_result)
    })
}

fn spawn_load_plugin_detail(app: &mut App<'_>, name: String) -> JoinHandle<ExecOutcome> {
    let plugin_engine = app.ctx.plugin_engine.clone();
    tokio::spawn(async move {
        match plugin_engine.get_plugin_detail(&name).await {
            Ok(detail) => ExecOutcome::PluginDetailLoad(name, Ok(detail)),
            Err(error) => ExecOutcome::PluginDetailLoad(name, Err(error.to_string())),
        }
    })
}

/// Refresh plugin statuses/health and dispatch a payload through ExecOutcome.result_json.
async fn execute_plugins_refresh(app: &mut App<'_>) -> ExecOutcome {
    let plugin_engine = &*app.ctx.plugin_engine;
    let plugins = plugin_engine.list_plugins().await;

    app.browser.update_browser_filtered();

    ExecOutcome::PluginsRefresh(format!("{} plugins refreshed", plugins.len()), Some(plugins))
}

/// Export logs to a default path in temp dir (redacted).
async fn execute_plugins_export_default(app: &mut App<'_>, name: String) -> ExecOutcome {
    let plugin_engine = &*app.ctx.plugin_engine;
    let mcp_client_mgr = plugin_engine.client_manager();
    // Default temp path
    let mut path = std::env::temp_dir();
    path.push(format!("mcp_{}_logs.txt", name));
    let res = mcp_client_mgr.log_manager().export_logs(&name, &path).await;
    let msg = match res {
        Ok(_) => format!("Plugins: exported logs for '{}' to {}", name, path.display()),
        Err(e) => format!("Plugins: export logs for '{}' failed: {}", name, e),
    };
    ExecOutcome::Log(msg)
}

/// Validate Add Plugin view input and emit a preview payload.
fn execute_plugins_validate(app: &mut App) -> ExecOutcome {
    let Some(add_view_state) = &app.plugins.add else {
        return ExecOutcome::default();
    };
    let name = add_view_state.name.trim();
    if let Err(e) = validate_server_name(name) {
        return ExecOutcome::PluginValidationErr(e.to_string());
    }

    // Build server candidate based on selected transport
    let mut server = McpServer::default();
    match add_view_state.transport {
        PluginTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if base_url.is_empty() {
                return ExecOutcome::PluginValidationErr("Base URL is required for remote transport".into());
            } else if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                return ExecOutcome::PluginValidationErr("Invalid Base URL".into());
            }
            match collect_key_value_rows(&add_view_state.kv_editor.rows) {
                Ok(Some(map)) => {
                    server.headers = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome::PluginValidationErr(format!("Invalid headers: {}", errors.join("; ")));
                }
            }
        }
        PluginTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                return ExecOutcome::PluginValidationErr("Command is required for local transport".into());
            } else {
                server.command = Some(command.to_string());
                if !add_view_state.args.trim().is_empty() {
                    let parsed: Vec<String> = add_view_state.args.split_whitespace().map(|s| s.to_string()).collect();
                    server.args = Some(parsed);
                }
            }
            match collect_key_value_rows(&add_view_state.kv_editor.rows) {
                Ok(Some(map)) => {
                    server.env = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome::PluginValidationErr(format!("Invalid env vars: {}", errors.join("; ")));
                }
            }
        }
    }

    ExecOutcome::PluginValidationOk("✓ Looks good!".to_string())
}

/// Apply Add Plugin view: write server to config and refresh plugins list.
async fn execute_plugins_save(app: &mut App<'_>) -> ExecOutcome {
    let Some(add_view_state) = &app.plugins.add else {
        return ExecOutcome::default();
    };
    let name = add_view_state.name.trim().to_string();
    let mut server = McpServer::default();
    match add_view_state.transport {
        PluginTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                return ExecOutcome::PluginValidationErr("Plugin validation failed: invalid Base URL".into());
            }
            match collect_key_value_rows(&add_view_state.kv_editor.rows) {
                Ok(Some(envs)) => {
                    server.headers = Some(envs);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome::PluginValidationErr(format!("plugin validation failed: invalid headers: {}", errors.join("; ")));
                }
            }
        }
        PluginTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                return ExecOutcome::PluginValidationErr("Plugin validation failed: command is required".into());
            }
            server.command = Some(command.to_string());
            if !add_view_state.args.trim().is_empty() {
                let parsed: Vec<String> = add_view_state.args.split_whitespace().map(|s| s.to_string()).collect();
                server.args = Some(parsed);
            }
            match collect_key_value_rows(&add_view_state.kv_editor.rows) {
                Ok(Some(map)) => {
                    server.env = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome::PluginValidationErr(format!("Plugin validation failed: invalid env vars: {}", errors.join("; ")));
                }
            }
        }
    }

    // Write to config
    let path = default_config_path();
    let mut cfg = if let Ok(text) = read_to_string(&path) {
        from_str::<McpConfig>(&text).unwrap_or_default()
    } else {
        McpConfig::default()
    };
    cfg.mcp_servers.insert(name.clone(), server);
    if let Err(e) = validate_config(&cfg) {
        return ExecOutcome::PluginValidationErr(format!("Plugin validation failed: {}", e));
    }
    let _ = save_config_to_path(&mut cfg, &path);
    // Refresh list
    execute_load_plugins(app).await;

    // Dismiss Add view and select the newly added plugin if present
    app.plugins.add = None;
    if let Some(index) = app.plugins.table.items.iter().position(|item| item.name == name) {
        app.plugins.table.selected = Some(index);
    }
    ExecOutcome::Log(format!("Plugins: added '{}'", name))
}

/// Strict validator for key/value rows captured in the Add Plugin editor.
fn collect_key_value_rows(rows: &[EnvRow]) -> Result<Option<Vec<EnvVar>>, Vec<String>> {
    let mut envs = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (index, row) in rows.iter().enumerate() {
        let key = row.key.trim();
        if key.is_empty() {
            errors.push(format!("row {} has empty key", index + 1));
            continue;
        }
        let value = row.value.trim().to_string();
        let env_source = determine_env_source(&value);
        let env = EnvVar::new(key.to_string(), value, env_source);
        envs.push(env);
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if envs.is_empty() { Ok(None) } else { Ok(Some(envs)) }
}

/// Spawn an HTTP execution on the Tokio scheduler while updating local state
/// to show the spinner.
fn spawn_execute_http(app: &mut App<'_>, spec: CommandSpec, body: Map<String, Value>) -> JoinHandle<ExecOutcome> {
    app.executing = true;
    app.throbber_idx = 0;

    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);

    tokio::spawn(async move { execute_http_task(active, spec, body).await })
}

fn spawn_execute_mcp(app: &mut App<'_>, spec: CommandSpec, arguments: Map<String, Value>) -> JoinHandle<ExecOutcome> {
    app.executing = true;
    app.throbber_idx = 0;

    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);
    let engine = app.ctx.plugin_engine.clone();

    tokio::spawn(async move { execute_mcp_task(active, engine, spec, arguments).await })
}

/// Background task body for executing an HTTP request and translating it into
/// an [`ExecOutcome`].
async fn execute_http_task(active_exec_count: Arc<AtomicUsize>, spec: CommandSpec, body: Map<String, Value>) -> ExecOutcome {
    let result = exec_remote(&spec, body).await;
    let outcome = result.unwrap_or_else(|err| ExecOutcome::Log(format!("Error: {}", err)));

    active_exec_count.fetch_sub(1, Ordering::Relaxed);

    outcome
}

async fn execute_mcp_task(
    active_exec_count: Arc<AtomicUsize>,
    engine: Arc<PluginEngine>,
    spec: CommandSpec,
    arguments: Map<String, Value>,
) -> ExecOutcome {
    let result = engine.execute_tool(&spec, &arguments).await;
    let outcome = result.unwrap_or_else(|err| ExecOutcome::Log(format!("Error: {}", err)));

    active_exec_count.fetch_sub(1, Ordering::Relaxed);

    outcome
}
/// Parses command arguments and flags from input tokens.
///
/// This function processes the command line tokens after the group and subcommand,
/// separating positional arguments from flags and validating flag syntax.
///
/// # Arguments
///
/// * `argument_tokens` - The tokens after the group and subcommand
/// * `command_spec` - The command specification for validation
///
/// # Returns
///
/// Returns `Ok((flags, args))` where flags is a map of flag names to values
/// and args is a vector of positional arguments, or an error if parsing fails.
///
/// # Flag Parsing Rules
///
/// - `--flag=value` format is supported
/// - Boolean flags don't require values
/// - Non-boolean flags require values (next token or after =)
/// - Unknown flags are rejected
fn parse_command_arguments(
    argument_tokens: &[String],
    command_spec: &CommandSpec,
) -> Result<(HashMap<String, Option<String>>, Vec<String>)> {
    let mut user_flags: HashMap<String, Option<String>> = HashMap::new();
    let mut user_args: Vec<String> = Vec::new();
    let mut index = 0;

    while index < argument_tokens.len() {
        let token = &argument_tokens[index];

        if token.starts_with("--") {
            let flag_name = token.trim_start_matches('-');

            // Handle --flag=value format
            if let Some(equals_pos) = flag_name.find('=') {
                let name = &flag_name[..equals_pos];
                let value = &flag_name[equals_pos + 1..];
                user_flags.insert(name.to_string(), Some(value.to_string()));
            } else {
                // Handle --flag or --flag value format
                if let Some(flag_spec) = command_spec.flags.iter().find(|f| f.name == flag_name) {
                    if flag_spec.r#type == "boolean" {
                        user_flags.insert(flag_name.to_string(), None);
                    } else {
                        // Non-boolean flag requires a value
                        if index + 1 < argument_tokens.len() && !argument_tokens[index + 1].starts_with('-') {
                            user_flags.insert(flag_name.to_string(), Some(argument_tokens[index + 1].to_string()));
                            index += 1; // Skip the value token
                        } else {
                            return Err(anyhow!("Flag '--{}' requires a value", flag_name));
                        }
                    }
                } else {
                    return Err(anyhow!("Unknown flag '--{}'", flag_name));
                }
            }
        } else {
            // Positional argument
            user_args.push(token.to_string());
        }

        index += 1;
    }

    Ok((user_flags, user_args))
}

/// Validates command arguments and flags against the command specification.
///
/// This function ensures that all required positional arguments and flags are
/// provided with appropriate values.
///
/// # Arguments
///
/// * `positional_arguments` - The provided positional arguments
/// * `user_flags` - The provided flags and their values
/// * `command_spec` - The command specification to validate against
///
/// # Returns
///
/// Returns `Ok(())` if validation passes, or an error message if validation fails.
///
/// # Validation Rules
///
/// - All required positional arguments must be provided
/// - All required flags must be present
/// - Non-boolean required flags must have non-empty values
fn validate_command_arguments(
    positional_arguments: &[String],
    user_flags: &HashMap<String, Option<String>>,
    command_spec: &CommandSpec,
) -> Result<()> {
    // Validate required positional arguments
    if positional_arguments.len() < command_spec.positional_args.len() {
        let missing_arguments: Vec<String> = command_spec.positional_args[positional_arguments.len()..]
            .iter()
            .map(|arg| arg.name.to_string())
            .collect();
        return Err(anyhow!("Missing required argument(s): {}", missing_arguments.join(", ")));
    }

    // Validate required flags
    for flag_spec in &command_spec.flags {
        if flag_spec.required {
            if flag_spec.r#type == "boolean" {
                if !user_flags.contains_key(&flag_spec.name) {
                    return Err(anyhow!("Missing required flag: --{}", flag_spec.name));
                }
            } else {
                match user_flags.get(&flag_spec.name) {
                    Some(Some(value)) if !value.is_empty() => {}
                    _ => {
                        return Err(anyhow!("Missing required flag value: --{} <value>", flag_spec.name));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Builds a JSON request body from user-provided flags.
///
/// This function converts the parsed flags into a JSON object that can be sent
/// as the request body for the HTTP command execution.
///
/// # Arguments
///
/// * `user_flags` - The flags provided by the user
/// * `command_spec` - The command specification for type information
///
/// # Returns
///
/// Returns a JSON Map containing the flag values with appropriate types.
///
/// # Type Conversion
///
/// - Boolean flags are converted to `true` if present
/// - String flags are converted to their string values
/// - Flags not in the specification are ignored
fn build_request_body(user_flags: HashMap<String, Option<String>>, command_spec: &CommandSpec) -> Map<String, Value> {
    let mut request_body = Map::new();

    for (flag_name, flag_value) in user_flags.into_iter() {
        if let Some(flag_spec) = command_spec.flags.iter().find(|f| f.name == flag_name) {
            if flag_spec.r#type == "boolean" {
                request_body.insert(flag_name, Value::Bool(true));
            } else if let Some(value) = flag_value {
                match flag_spec.r#type.as_str() {
                    "number" => {
                        if let Ok(number) = Number::from_str(value.as_str()) {
                            request_body.insert(flag_name, Value::Number(number));
                        }
                    }
                    _ => {
                        request_body.insert(flag_name, Value::String(value));
                    }
                };
            }
        }
    }

    request_body
}

/// Executes a command from the palette input.
///
/// This function parses the current palette input, validates the command and its
/// arguments, and initiates the HTTP execution. It handles command parsing,
/// argument validation, and sets up the execution context for the command.
///
/// # Arguments
///
/// * `application` - The application state containing the palette input and registry
///
/// # Returns
///
/// Returns `Ok(command_spec)` if the command is valid and execution is started,
/// or `Err(error_message)` if there are validation errors.
///
/// # Validation
///
/// The function validates:
/// - Command format (group subcommand)
/// - Required positional arguments
/// - Required flags and their values
/// - Flag syntax and types
///
/// # Execution Context
///
/// After validation, the function:
/// - Resolves the command path with positional arguments
/// - Builds the request body with flag values
/// - Stores execution context for pagination and replay
/// - Initiates background HTTP execution
///
/// # Example
///
/// ```
/// // For input "apps info my-app --verbose"
/// // Validates command exists, app_id is provided, starts execution
/// ```
fn start_palette_execution(application: &mut App) -> Option<Vec<Cmd>> {
    let valid = validate_command(application);
    match valid {
        Ok((command_spec, request_body, user_args)) => {
            let command_input = application.palette.input();
            application.logs.entries.push(format!("Running: {}", command_input));
            application.logs.rich_entries.push(LogEntry::Text {
                level: Some("info".into()),
                msg: format!("Running: {}", command_input),
            });
            execute_command(command_spec, request_body, user_args)
        }
        Err(error) => Some(vec![Cmd::ApplyPaletteError(error.to_string())]),
    }
}

fn validate_command(application: &mut App) -> Result<(CommandSpec, Map<String, Value>, Vec<String>)> {
    // Step 1: Parse and validate the palette input
    let input_owned = application.palette.input().to_string();
    let input = input_owned.trim().to_string();
    if input.is_empty() {
        return Err(anyhow!("Empty command input. Type a command (e.g., apps info)"));
    }
    // Tokenize the input using shell-like parsing for consistent behavior
    let tokens = lex_shell_like(&input);
    if tokens.len() < 2 {
        return Err(anyhow!(
            "Incomplete command '{}'. Use '<group> <sub>' format (e.g., apps info)",
            input
        ));
    }

    // Step 2: Find the command specification in the registry
    let command_spec = {
        let lock = application
            .ctx
            .registry
            .lock()
            .map_err(|_| anyhow!("Could not obtain lock to registry"))?;
        let commands = &lock.commands;
        find_by_group_and_cmd(commands, tokens[0].as_str(), tokens[1].as_str())?
    };

    // Step 3: Parse command arguments and flags from input tokens
    let (user_flags, user_args) = parse_command_arguments(&tokens[2..], &command_spec)?;

    // Step 4: Validate command arguments and flags
    validate_command_arguments(&user_args, &user_flags, &command_spec)?;

    // Step 5: Build request body from flags
    let request_body = build_request_body(user_flags, &command_spec);
    // Step 6: Persist execution context for pagination UI and replay
    persist_execution_context(application, &command_spec, &request_body, &input);

    Ok((command_spec, request_body, user_args))
}

fn persist_execution_context(application: &mut App, command_spec: &CommandSpec, request_body: &Map<String, Value>, input: &str) {
    application.last_command_ranges = match command_spec.execution() {
        CommandExecution::Http(http) => Some(http.ranges.clone()),
        _ => None,
    };
    application.last_spec = Some(command_spec.clone());
    application.last_body = Some(request_body.clone());
    let initial_range = build_range_header_from_body(request_body);

    application.initial_range = initial_range.clone();
    application.pagination_history.clear();
    application.pagination_history.push(initial_range);
    application.palette.push_history_if_needed(input);
}

fn execute_command(command_spec: CommandSpec, request_body: Map<String, Value>, user_args: Vec<String>) -> Option<Vec<Cmd>> {
    match command_spec.execution() {
        CommandExecution::Http(http) => {
            let mut command_spec_to_run = command_spec.clone();
            let mut positional_argument_map: HashMap<String, String> = HashMap::new();
            for (index, positional_argument) in command_spec.positional_args.iter().enumerate() {
                positional_argument_map.insert(positional_argument.name.clone(), user_args.get(index).cloned().unwrap_or_default());
            }

            if let Some(http_spec) = command_spec_to_run.http_mut() {
                http_spec.path = resolve_path(&http.path, &positional_argument_map);
            }

            Some(vec![Cmd::ExecuteHttp(command_spec_to_run, request_body)])
        }
        CommandExecution::Mcp(_) => {
            let mut arguments = request_body;
            for (index, positional_argument) in command_spec.positional_args.iter().enumerate() {
                if let Some(value) = user_args.get(index) {
                    arguments.insert(positional_argument.name.clone(), Value::String(value.clone()));
                }
            }

            Some(vec![Cmd::ExecuteMcp(command_spec, arguments)])
        }
    }
}
