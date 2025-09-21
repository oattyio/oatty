//! # Command Execution Layer
//!
//! This module translates high-level application effects (`Effect`) into
//! imperative commands (`Cmd`) and executes them. It provides the "boundary"
//! where the pure state management of the app interacts with side effects
//! such as:
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
use heroku_mcp::McpConfig;
use heroku_mcp::PluginDetail;
use heroku_mcp::config::McpServer;
use heroku_mcp::config::default_config_path;
use heroku_mcp::config::save_config_to_path;
use heroku_mcp::config::validate_config;
use heroku_mcp::config::validate_server_name;
use heroku_mcp::{client::HealthCheckResult, types::plugin::AuthStatus};
use heroku_registry::CommandSpec;
use heroku_types::ExecOutcome;
use heroku_types::{Effect, Modal, Route};
use heroku_util::exec_remote;
use heroku_util::lex_shell_like;
use heroku_util::resolve_path;
use reqwest::Url;
use serde_json::Map;
use serde_json::Value;
use serde_json::from_str;
use serde_json::json;
use serde_json::to_string_pretty;
use serde_json::to_value;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::sync::atomic::Ordering;
use std::vec;

use crate::app::App;
use crate::ui::components::logs::state::LogEntry;
use crate::ui::components::plugins::EnvRow;
use crate::{
    app::{self},
    ui::components::plugins::{AddTransport, PluginListItem},
};

/// Represents side-effectful system commands executed outside of pure state
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
    /// - [`CommandSpec`]: API request metadata
    /// - `String`: URL path (such as `/apps`)
    /// - `serde_json::Map`: JSON body
    ///
    /// # Example
    /// ```rust,ignore
    /// use your_crate::Cmd;
    /// use heroku_registry::CommandSpec;
    /// use{Map, Value};
    ///
    /// let spec = CommandSpec { method: "GET".into(), path: "/apps".into() };
    /// let cmd = Cmd::ExecuteHttp(spec.clone(), "/apps".into(), Map::new());
    ///
    /// if let Cmd::ExecuteHttp(s, p, b) = cmd {
    ///     assert_eq!(s.method, "GET");
    ///     assert_eq!(p, "/apps");
    ///     assert!(b.is_empty());
    /// }
    /// ```
    ExecuteHttp(CommandSpec, Map<String, Value>),
    /// Load MCP plugins from config (synchronous file read) and populate UI state.
    LoadPlugins,
    PluginsStart(String),
    PluginsStop(String),
    PluginsRestart(String),
    PluginsRefresh,
    PluginsRefreshLogs(String),
    PluginsExportLogsDefault(String),
    PluginsOpenSecrets(String),
    PluginsSaveEnv {
        name: String,
        rows: Vec<(String, String)>,
    },
    PluginsValidateAdd,
    PluginsApplyAdd,
}

/// Convert application [`Effect`]s into actual [`Cmd`] instances.
///
/// This enables a clean separation: effects represent "what should happen",
/// while commands describe "how it should happen".
///
pub async fn run_from_effects(app: &mut app::App<'_>, effects: Vec<Effect>) -> Vec<ExecOutcome> {
    let mut commands = Vec::new();

    for effect in effects {
        let effect_commands = match effect {
            Effect::CopyToClipboardRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
            Effect::CopyLogsRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
            Effect::NextPageRequested(next_raw) => handle_next_page_requested(app, next_raw),
            Effect::PrevPageRequested => handle_prev_page_requested(app),
            Effect::FirstPageRequested => handle_first_page_requested(app),
            Effect::PluginsLoadRequested => Some(vec![Cmd::LoadPlugins]),
            Effect::PluginsStart(name) => Some(vec![Cmd::PluginsStart(name)]),
            Effect::PluginsStop(name) => Some(vec![Cmd::PluginsStop(name)]),
            Effect::PluginsRestart(name) => Some(vec![Cmd::PluginsRestart(name)]),
            Effect::PluginsRefresh => Some(vec![Cmd::PluginsRefresh]),
            Effect::PluginsOpenLogs(name) => Some(vec![Cmd::PluginsRefreshLogs(name)]),
            Effect::PluginsRefreshLogs(name) => Some(vec![Cmd::PluginsRefreshLogs(name)]),
            Effect::PluginsExportLogsDefault(name) => Some(vec![Cmd::PluginsExportLogsDefault(name)]),
            Effect::PluginsOpenSecrets(name) => Some(vec![Cmd::PluginsOpenSecrets(name)]),
            Effect::PluginsSaveEnv { name, rows } => Some(vec![Cmd::PluginsSaveEnv { name, rows }]),
            Effect::PluginsOpenAdd => Some(vec![]),
            Effect::PluginsValidateAdd => Some(vec![Cmd::PluginsValidateAdd]),
            Effect::PluginsApplyAdd => Some(vec![Cmd::PluginsApplyAdd]),
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
fn handle_next_page_requested(app: &mut app::App, next_raw: String) -> Option<Vec<Cmd>> {
    let mut commands = Vec::new();

    if let (Some(spec), Some(mut body)) = (app.last_spec.clone(), app.last_body.clone()) {
        // Inject raw next-range override for Range header
        body.insert("next-range".into(), Value::String(next_raw.clone()));

        // Append to history for Prev/First navigation
        app.pagination_history.push(Some(next_raw));

        commands.push(Cmd::ExecuteHttp(spec, body));
    } else {
        app.logs
            .entries
            .push("Cannot request next page: no prior command context".into());
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
fn handle_prev_page_requested(app: &mut app::App) -> Option<Vec<Cmd>> {
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
fn handle_first_page_requested(app: &mut app::App) -> Option<Vec<Cmd>> {
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
        app.logs
            .entries
            .push("Cannot request first page: no prior command context".into());
    }

    Some(commands)
}

/// Handle the show modal effect by setting the appropriate modal component.
///
/// # Arguments
/// * `app` - The application state
/// * `modal` - The modal type to show
///
/// # Returns
/// A vector containing no commands (modal changes are direct UI state updates)
fn handle_show_modal(app: &mut app::App, modal: Modal) -> Option<Vec<Cmd>> {
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
fn handle_close_modal(app: &mut app::App) -> Option<Vec<Cmd>> {
    // retain focus so it can be restored when the modal closes
    app.set_open_modal_kind(None);
    None // No commands needed for direct UI state update
}

/// Handle the switch to route effect by setting the appropriate main view component.
///
/// # Arguments
/// * `app` - The application state
/// * `route` - The route to switch to
///
/// # Returns
/// A vector containing no commands (view changes are direct UI state updates)
fn handle_switch_to(app: &mut app::App, route: Route) -> Option<Vec<Cmd>> {
    app.set_current_route(route);
    Some(vec![])
}

/// When pressing Enter in the browser, populate the palette with the
/// constructed command and close the command browser.
fn handle_send_to_palette(app: &mut app::App, command_spec: CommandSpec) -> Option<Vec<Cmd>> {
    let name = command_spec.name;
    let group = command_spec.group;

    app.palette.set_input(format!("{} {}", group, name));
    app.palette.set_cursor(app.palette.input().len());
    app.palette
        .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers, &*app.ctx.theme);
    Some(vec![])
}

/// Execute a sequence of commands and update application logs.
///
/// Each command corresponds to a user-visible side effect, such as writing
/// content to the clipboard or making a network call. Logs are appended with
/// human-readable results.
///
/// # Example
/// ```rust,ignore
/// # use your_crate::{run_cmds, Cmd};
/// # struct DummyApp { logs: Vec<String> }
/// # impl DummyApp { fn new() -> Self { Self { logs: Vec::new() } } }
/// # fn make_app() -> DummyApp { DummyApp::new() }
/// let mut app = make_app();
/// let commands = vec![Cmd::ClipboardSet("test".into())];
/// run_cmds(&mut app, commands);
/// // (In real case, app.logs would contain "Copied: test" after success.)
/// ```
pub async fn run_cmds(app: &mut app::App<'_>, commands: Vec<Cmd>) -> Vec<ExecOutcome> {
    let mut outcomes: Vec<ExecOutcome> = vec![];
    for command in commands {
        let outcome = match command {
            Cmd::ApplyPaletteError(error) => apply_palette_error(app, error),
            Cmd::ClipboardSet(text) => execute_clipboard_set(app, text),
            Cmd::ExecuteHttp(spec, body) => execute_http(app, spec, body).await,
            Cmd::LoadPlugins => execute_load_plugins(app).await,
            Cmd::PluginsStart(name) => execute_plugins_action(app, PluginAction::Start, name).await,
            Cmd::PluginsStop(name) => execute_plugins_action(app, PluginAction::Stop, name).await,
            Cmd::PluginsRestart(name) => execute_plugins_action(app, PluginAction::Restart, name).await,
            Cmd::PluginsRefresh => execute_plugins_refresh(app).await,
            Cmd::PluginsRefreshLogs(name) => execute_plugins_refresh_logs(app, name).await,
            Cmd::PluginsExportLogsDefault(name) => execute_plugins_export_default(app, name).await,
            Cmd::PluginsOpenSecrets(name) => execute_plugins_open_env(name),
            Cmd::PluginsSaveEnv { name, rows } => execute_plugins_save_env(name, rows),
            Cmd::PluginsValidateAdd => execute_plugins_validate_add(app),
            Cmd::PluginsApplyAdd => execute_plugins_apply_add(app).await,
        };
        outcomes.push(outcome);
    }
    outcomes
}

/// Execute a clipboard set command by writing text to the system clipboard.
///
/// Updates the application logs with success or error messages and maintains
/// log size limits for performance.
///
/// # Arguments
/// * `app` - The application state for logging
/// * `text` - The text content to write to the clipboard
fn execute_clipboard_set(app: &mut app::App, text: String) -> ExecOutcome {
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
async fn execute_load_plugins(app: &mut app::App<'_>) -> ExecOutcome {
    let mut items: Vec<PluginListItem> = Vec::new();
    let plugin_engine = app.ctx.plugin_engine.clone();
    for plugin_detail in plugin_engine.list_plugins().await {
        let PluginDetail {
            command_or_url,
            status,
            tags,
            name,
            ..
        } = plugin_detail.clone();
        items.push(PluginListItem {
            auth_status: AuthStatus::Unknown,
            name,
            status: String::from(status.display()),
            command_or_url,
            tags,
            latency_ms: None,
            last_error: None,
        });
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    app.plugins.table.replace_items(items);

    ExecOutcome::default()
}

#[derive(Clone, Copy)]
enum PluginAction {
    Start,
    Stop,
    Restart,
}

/// Execute a plugin lifecycle action using the MCP supervisor.
async fn execute_plugins_action(app: &mut app::App<'_>, action: PluginAction, name: String) -> ExecOutcome {
    let plugin_engine = &*app.ctx.plugin_engine;

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
    ExecOutcome::new(msg)
}

/// Refresh plugin statuses/health and dispatch a payload through ExecOutcome.result_json.
async fn execute_plugins_refresh(app: &mut app::App<'_>) -> ExecOutcome {
    let plugin_engine = app.ctx.plugin_engine.clone();
    let names: Vec<String> = app.plugins.table.items.iter().map(|item| item.name.clone()).collect();

    let mut arr = Vec::new();
    let client_mgr = plugin_engine.client_manager();
    for name in names {
        if let Some(client) = client_mgr.get_client(&name).await {
            let mut _client = client.lock().await;
            let status_result = client_mgr.get_plugin_status(&name).await;
            let stat = status_result.expect("status not available");

            let health_result = _client.health_check().await;
            let HealthCheckResult { latency_ms, error, .. } = health_result.ok().unwrap_or_default();
            arr.push(json!({
                "name": name,
                "status": stat.display(),
                "latency_ms": latency_ms,
                "last_error": error.unwrap_or_default(),
            }));
        }
    }
    let payload = json!({ "plugins_refresh": arr });
    ExecOutcome {
        log: "Plugins: refreshed".into(),
        result_json: Some(payload),
        open_table: false,
        pagination: None,
    }
}

/// Refresh recent logs for the given plugin and dispatch payload.
async fn execute_plugins_refresh_logs(app: &mut app::App<'_>, name: String) -> ExecOutcome {
    let plugin_engine = &*app.ctx.plugin_engine;
    let mcp_client_mgr = plugin_engine.client_manager();
    let lines = mcp_client_mgr.log_manager().get_recent_logs(&name, 500).await;
    let payload = json!({ "plugins_logs": { "name": name, "lines": lines } });

    ExecOutcome {
        log: "Plugins: logs refreshed".into(),
        result_json: Some(payload),
        open_table: false,
        pagination: None,
    }
}

/// Export logs to a default path in temp dir (redacted).
async fn execute_plugins_export_default(app: &mut app::App<'_>, name: String) -> ExecOutcome {
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
    ExecOutcome {
        log: msg,
        result_json: None,
        open_table: false,
        pagination: None,
    }
}

/// Open environment editor: load rows from config and dispatch a payload.
fn execute_plugins_open_env(name: String) -> ExecOutcome {
    let path = default_config_path();
    let mut rows: Vec<EnvRow> = Vec::new();
    if let Ok(text) = read_to_string(&path) {
        if let Ok(cfg) = from_str::<McpConfig>(&text) {
            if let Some(s) = cfg.mcp_servers.get(&name) {
                if let Some(env) = &s.env {
                    for (k, v) in env.iter() {
                        rows.push(EnvRow {
                            key: k.clone(),
                            value: v.clone(),
                            is_secret: v.trim().starts_with("${secret:")
                                || k.contains("SECRET")
                                || k.contains("TOKEN")
                                || k.contains("PASSWORD"),
                        });
                    }
                }
            }
        }
    }
    // Dispatch as result_json for App to apply
    let payload = json!({
        "plugins_env": {
            "name": name,
            "rows": rows
                .iter()
                .map(|row| json!({
                    "key": row.key,
                    "value": row.value,
                    "is_secret": row.is_secret,
                }))
                .collect::<Vec<_>>(),
        }
    });
    ExecOutcome {
        log: "Plugins: env loaded".into(),
        result_json: Some(payload),
        open_table: false,
        pagination: None,
    }
}

/// Save environment changes back to config (overwrites env map for the plugin).
fn execute_plugins_save_env(name: String, rows: Vec<(String, String)>) -> ExecOutcome {
    let path = default_config_path();
    let mut cfg = if let Ok(text) = read_to_string(&path) {
        from_str::<McpConfig>(&text).unwrap_or_default()
    } else {
        McpConfig::default()
    };
    if let Some(srv) = cfg.mcp_servers.get_mut(&name) {
        let mut map = std::collections::HashMap::new();
        for (k, v) in rows.into_iter() {
            map.insert(k, v);
        }
        srv.env = Some(map);
    }
    // Validate and save
    if let Err(e) = validate_config(&cfg) {
        return ExecOutcome {
            log: format!("Env save validation failed: {}", e),
            result_json: None,
            open_table: false,
            pagination: None,
        };
    }
    let _ = save_config_to_path(&cfg, &path);
    ExecOutcome {
        log: format!("Plugins: saved env for '{}'", name),
        result_json: None,
        open_table: false,
        pagination: None,
    }
}

/// Validate Add Plugin view input and emit a preview payload.
fn execute_plugins_validate_add(app: &mut app::App) -> ExecOutcome {
    let Some(add_view_state) = &app.plugins.add else {
        return ExecOutcome::default();
    };
    let name = add_view_state.name.trim();
    let mut message = String::from("Looks good");
    let mut ok = true;
    if let Err(e) = validate_server_name(name) {
        ok = false;
        message = e.to_string();
    }

    // Build server candidate based on selected transport
    let mut server = McpServer::default();
    match add_view_state.transport {
        AddTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if base_url.is_empty() {
                ok = false;
                message = "Base URL is required for remote transport".into();
            } else if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                ok = false;
                message = "Invalid Base URL".into();
            }
            match collect_key_value_rows(&add_view_state.header_editor.rows) {
                Ok(Some(map)) => {
                    server.headers = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    ok = false;
                    message = format!("Invalid headers: {}", errors.join("; "));
                }
            }
        }
        AddTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                ok = false;
                message = "Command is required for local transport".into();
            } else {
                server.command = Some(command.to_string());
                if !add_view_state.args.trim().is_empty() {
                    let parsed: Vec<String> = add_view_state.args.split_whitespace().map(|s| s.to_string()).collect();
                    server.args = Some(parsed);
                }
            }
            match collect_key_value_rows(&add_view_state.env_editor.rows) {
                Ok(Some(map)) => {
                    server.env = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    ok = false;
                    message = format!("Invalid env vars: {}", errors.join("; "));
                }
            }
        }
    }

    // Build a preview payload; skip live health probe (rmcp connects at start).
    let patch = build_add_patch(name, &server);
    let payload = json!({
        "plugins_add_preview": { "ok": ok, "message": message, "patch": patch }
    });
    ExecOutcome {
        log: format!("Add validate: {}", name),
        result_json: Some(payload),
        open_table: false,
        pagination: None,
    }
}

/// Apply Add Plugin view: write server to config and refresh plugins list.
async fn execute_plugins_apply_add(app: &mut app::App<'_>) -> ExecOutcome {
    let Some(add_view_state) = &app.plugins.add else {
        return ExecOutcome::default();
    };
    let name = add_view_state.name.trim().to_string();
    let mut server = McpServer::default();
    match add_view_state.transport {
        AddTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                return ExecOutcome {
                    log: "Add apply validation failed: invalid Base URL".into(),
                    result_json: None,
                    open_table: false,
                    pagination: None,
                };
            }
            match collect_key_value_rows(&add_view_state.header_editor.rows) {
                Ok(Some(map)) => {
                    server.headers = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome {
                        log: format!("Add apply validation failed: invalid headers: {}", errors.join("; ")),
                        result_json: None,
                        open_table: false,
                        pagination: None,
                    };
                }
            }
        }
        AddTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                return ExecOutcome {
                    log: "Add apply validation failed: command is required".into(),
                    result_json: None,
                    open_table: false,
                    pagination: None,
                };
            }
            server.command = Some(command.to_string());
            if !add_view_state.args.trim().is_empty() {
                let parsed: Vec<String> = add_view_state.args.split_whitespace().map(|s| s.to_string()).collect();
                server.args = Some(parsed);
            }
            match collect_key_value_rows(&add_view_state.env_editor.rows) {
                Ok(Some(map)) => {
                    server.env = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome {
                        log: format!("Add apply validation failed: invalid env vars: {}", errors.join("; ")),
                        result_json: None,
                        open_table: false,
                        pagination: None,
                    };
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
        return ExecOutcome {
            log: format!("Add apply validation failed: {}", e),
            result_json: None,
            open_table: false,
            pagination: None,
        };
    }
    let _ = save_config_to_path(&cfg, &path);
    // Refresh list
    execute_load_plugins(app).await;

    // Dismiss Add view and select the newly added plugin if present
    app.plugins.add = None;
    if let Some(index) = app.plugins.table.items.iter().position(|item| item.name == name) {
        app.plugins.table.selected = Some(index);
    }
    ExecOutcome {
        log: format!("Plugins: added '{}'", name),
        result_json: None,
        open_table: false,
        pagination: None,
    }
}

fn build_add_patch(name: &str, server: &heroku_mcp::config::McpServer) -> String {
    let mut map = Map::new();
    let v = to_value(server).unwrap_or(serde_json::json!({}));
    let mut servers = Map::new();
    servers.insert(name.to_string(), v);
    map.insert("mcpServers".to_string(), Value::Object(servers));
    to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_default()
}

/// Strict validator for key/value rows captured in the Add Plugin editor.
fn collect_key_value_rows(rows: &[EnvRow]) -> Result<Option<std::collections::HashMap<String, String>>, Vec<String>> {
    let mut map = std::collections::HashMap::new();
    let mut errors: Vec<String> = Vec::new();

    for (index, row) in rows.iter().enumerate() {
        let key = row.key.trim();
        if key.is_empty() {
            errors.push(format!("row {} has empty key", index + 1));
            continue;
        }
        map.insert(key.to_string(), row.value.trim().to_string());
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if map.is_empty() { Ok(None) } else { Ok(Some(map)) }
}

/// Spawn a background thread to execute a Heroku API request.
///
/// Updates the application state to indicate execution has begun, attaches
/// a channel receiver for results, and spawns a worker thread that runs a
/// Tokio runtime for the async HTTP call.
///
/// # Example
/// ```rust,ignore
/// # use your_crate::execute_http;
/// # use heroku_registry::CommandSpec;
/// # useMap;
/// # struct DummyApp { executing: bool, exec_receiver: Option<std::sync::mpsc::Receiver<()>>, throbber_idx: usize }
/// # impl DummyApp { fn new() -> Self { Self { executing: false, exec_receiver: None, throbber_idx: 0 } } }
/// let mut app = DummyApp::new();
/// let spec = CommandSpec { method: "GET".into(), path: "/apps".into() };
/// execute_http(&mut app, spec, "/apps".into(), Map::new());
/// assert!(app.executing);
/// ```
///
/// # Arguments
/// * `app` - The application state to update with execution status
/// * `spec` - The command specification for the HTTP request
/// * `path` - The API endpoint path
/// * `body` - The request body as a JSON map
async fn execute_http(app: &mut app::App<'_>, spec: CommandSpec, body: Map<String, Value>) -> ExecOutcome {
    // Live request: spawn async task and show throbber
    app.executing = true;
    app.throbber_idx = 0;

    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);

    let outcome = exec_remote(&spec, body).await;

    let outcome = match outcome {
        Ok(out) => out,
        Err(err) => heroku_types::ExecOutcome {
            log: format!("Error: {}", err),
            result_json: None,
            open_table: false,
            pagination: None,
        },
    };

    // Mark one execution completed
    active.fetch_sub(1, Ordering::Relaxed);

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
        return Err(anyhow!(
            "Missing required argument(s): {}",
            missing_arguments.join(", ")
        ));
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
                request_body.insert(flag_name, Value::String(value));
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
fn start_palette_execution(application: &mut app::App) -> Option<Vec<Cmd>> {
    let valid = validate_command(application);
    match valid {
        Ok((command_spec, request_body, user_args)) => {
            let command_input = application.palette.input();
            application.logs.entries.push(format!("Running: {}", command_input));
            application.logs.rich_entries.push(LogEntry::Text {
                level: Some("info".into()),
                msg: format!("Running: {}", command_input),
            });
            return execute_command(command_spec, request_body, user_args);
        }
        Err(error) => Some(vec![Cmd::ApplyPaletteError(error.to_string())]),
    }
}

fn validate_command(application: &mut app::App) -> Result<(CommandSpec, Map<String, Value>, Vec<String>)> {
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
    let command_spec = application
        .ctx
        .registry
        .find_by_group_and_cmd(tokens[0].as_str(), tokens[1].as_str())?
        .clone();

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

fn persist_execution_context(
    application: &mut app::App,
    command_spec: &CommandSpec,
    request_body: &Map<String, Value>,
    input: &str,
) {
    application.last_command_ranges = Some(command_spec.ranges.clone());
    application.last_spec = Some(command_spec.clone());
    application.last_body = Some(request_body.clone());

    let init_field = request_body
        .get("range-field")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let init_start = request_body
        .get("range-start")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let init_end = request_body
        .get("range-end")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let init_order = request_body
        .get("order")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let init_max = request_body
        .get("max")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<usize>().ok());
    let initial_range = init_field.map(|field| {
        let mut h = format!("{} {}..{}", field, init_start, init_end);
        if let Some(ord) = init_order {
            h.push_str(&format!("; order={};", ord));
        }
        if let Some(m) = init_max {
            h.push_str(&format!("; max={};", m));
        }
        h
    });

    application.initial_range = initial_range.clone();
    application.pagination_history.clear();
    application.pagination_history.push(initial_range);
    application.palette.push_history_if_needed(input);
}

fn execute_command(
    command_spec: CommandSpec,
    request_body: Map<String, Value>,
    user_args: Vec<String>,
) -> Option<Vec<Cmd>> {
    let mut command_spec_to_run = command_spec.clone();
    let mut positional_argument_map: HashMap<String, String> = HashMap::new();
    for (index, positional_argument) in command_spec.positional_args.iter().enumerate() {
        positional_argument_map.insert(
            positional_argument.name.clone(),
            user_args.get(index).cloned().unwrap_or_default(),
        );
    }

    command_spec_to_run.path = resolve_path(&command_spec.path, &positional_argument_map);

    return Some(vec![Cmd::ExecuteHttp(command_spec_to_run, request_body.clone())]);
}
