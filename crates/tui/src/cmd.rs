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

use heroku_mcp::McpConfig;
use heroku_mcp::config::McpServer;
use heroku_mcp::config::default_config_path;
use heroku_mcp::config::save_config_to_path;
use heroku_mcp::config::validate_config;
use heroku_mcp::config::validate_server_name;
use heroku_mcp::{client::HealthCheckResult, types::plugin::AuthStatus};
use heroku_registry::CommandSpec;
use heroku_types::{Effect, Modal, Route};
use heroku_util::exec_remote;
use reqwest::Url;
use serde_json::Map;
use serde_json::Value;
use serde_json::from_str;
use serde_json::json;
use serde_json::to_string_pretty;
use serde_json::to_value;
use std::fs::read_to_string;
use std::sync::atomic::Ordering;
use std::vec;

use crate::cmd;
use crate::ui::components::plugins::EnvRow;
use crate::{
    app::{self},
    ui::components::{
        browser::BrowserComponent,
        help::HelpComponent,
        plugins::{AddTransport, PluginListItem, PluginsComponent},
        table::TableComponent,
    },
};

/// Represents side-effectful system commands executed outside of pure state
/// updates.
///
/// These commands bridge between the application's functional state model
/// and imperative actions (I/O, networking, system integration).
#[derive(Debug)]
pub enum Cmd {
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
    ExecuteHttp(Box<CommandSpec>, Map<String, Value>),
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
    PluginsCancel,
}

/// Convert application [`Effect`]s into actual [`Cmd`] instances.
///
/// This enables a clean separation: effects represent "what should happen",
/// while commands describe "how it should happen".
///
pub fn run_from_effects(app: &mut app::App, effects: Vec<Effect>) {
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
            Effect::PluginsCancel => Some(vec![Cmd::PluginsCancel]),
            Effect::ShowModal(modal) => handle_show_modal(app, modal),
            Effect::CloseModal => handle_close_modal(app),
            Effect::SwitchTo(route) => handle_switch_to(app, route),
        };
        if let Some(cmds) = effect_commands {
            commands.extend(cmds);
        }
    }

    run_cmds(app, commands)
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

        commands.push(Cmd::ExecuteHttp(Box::new(spec), body));
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

        commands.push(Cmd::ExecuteHttp(Box::new(spec), body));
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

        commands.push(Cmd::ExecuteHttp(Box::new(spec), body));
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
    Some(vec![]) // No commands needed for direct UI state update
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
pub fn run_cmds(app: &mut app::App, commands: Vec<Cmd>) {
    for command in commands {
        match command {
            Cmd::ClipboardSet(text) => execute_clipboard_set(app, text),
            Cmd::ExecuteHttp(spec, body) => execute_http(app, *spec, body),
            Cmd::LoadPlugins => execute_load_plugins(app),
            Cmd::PluginsStart(name) => execute_plugins_action(app, PluginAction::Start, name),
            Cmd::PluginsStop(name) => execute_plugins_action(app, PluginAction::Stop, name),
            Cmd::PluginsRestart(name) => execute_plugins_action(app, PluginAction::Restart, name),
            Cmd::PluginsRefresh => execute_plugins_refresh(app),
            Cmd::PluginsRefreshLogs(name) => execute_plugins_refresh_logs(app, name),
            Cmd::PluginsExportLogsDefault(name) => execute_plugins_export_default(app, name),
            Cmd::PluginsOpenSecrets(name) => execute_plugins_open_env(app, name),
            Cmd::PluginsSaveEnv { name, rows } => execute_plugins_save_env(app, name, rows),
            Cmd::PluginsValidateAdd => execute_plugins_validate_add(app),
            Cmd::PluginsApplyAdd => execute_plugins_apply_add(app),
            Cmd::PluginsCancel => execute_plugins_cancel(app),
        }
    }
}

/// Execute a clipboard set command by writing text to the system clipboard.
///
/// Updates the application logs with success or error messages and maintains
/// log size limits for performance.
///
/// # Arguments
/// * `app` - The application state for logging
/// * `text` - The text content to write to the clipboard
fn execute_clipboard_set(app: &mut app::App, text: String) {
    // Perform clipboard write and log outcome
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
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
}

/// Load MCP plugins from the user's config file and populate the PluginsState list.
fn execute_load_plugins(app: &mut app::App) {
    let path = default_config_path();
    let content = read_to_string(&path);
    let mut items: Vec<PluginListItem> = Vec::new();

    if let Ok(text) = content {
        if let Ok(cfg) = from_str::<McpConfig>(&text) {
            for (name, server) in cfg.mcp_servers.into_iter() {
                let command_or_url = format_command_or_url(&server);
                let status = if server.disabled.unwrap_or(false) {
                    "Disabled".to_string()
                } else {
                    "Stopped".to_string()
                };
                let tags = server.tags.unwrap_or_default();
                items.push(PluginListItem {
                    auth_status: AuthStatus::Unknown,
                    name,
                    status,
                    command_or_url,
                    tags,
                    latency_ms: None,
                    last_error: None,
                });
            }
        }
    }

    // Sort by name
    items.sort_by(|a, b| a.name.cmp(&b.name));
    app.plugins.replace_items(items);

    fn format_command_or_url(server: &McpServer) -> String {
        if let Some(cmd) = &server.command {
            let mut s = cmd.clone();
            if let Some(args) = &server.args {
                if !args.is_empty() {
                    s.push(' ');
                    s.push_str(&args.join(" "));
                }
            }
            s
        } else if let Some(url) = &server.base_url {
            url.as_str().to_string()
        } else {
            "".to_string()
        }
    }
}

#[derive(Clone, Copy)]
enum PluginAction {
    Start,
    Stop,
    Restart,
}

/// Execute a plugin lifecycle action using the MCP supervisor.
fn execute_plugins_action(app: &mut app::App, action: PluginAction, name: String) {
    let sup_opt = app.ctx.mcp.as_ref().cloned();
    let tx = app.exec_sender.clone();
    if let Some(sup) = sup_opt {
        tokio::spawn(async move {
            let res = match action {
                PluginAction::Start => sup.start_plugin(&name).await,
                PluginAction::Stop => sup.stop_plugin(&name).await,
                PluginAction::Restart => sup.restart_plugin(&name).await,
            };
            let msg = match (action, res) {
                (PluginAction::Start, Ok(_)) => format!("Plugins: started '{}'", name),
                (PluginAction::Stop, Ok(_)) => format!("Plugins: stopped '{}'", name),
                (PluginAction::Restart, Ok(_)) => format!("Plugins: restarted '{}'", name),
                (PluginAction::Start, Err(e)) => format!("Plugins: start '{}' failed: {}", name, e),
                (PluginAction::Stop, Err(e)) => format!("Plugins: stop '{}' failed: {}", name, e),
                (PluginAction::Restart, Err(e)) => format!("Plugins: restart '{}' failed: {}", name, e),
            };
            let _ = tx.send(heroku_types::ExecOutcome {
                log: msg,
                result_json: None,
                open_table: false,
                pagination: None,
            });
        });
    } else {
        app.logs
            .entries
            .push("MCP supervisor not initialized; cannot perform action".into());
    }
}

/// Refresh plugin statuses/health and dispatch a payload through ExecOutcome.result_json.
fn execute_plugins_refresh(app: &mut app::App) {
    let sup_opt = app.ctx.mcp.as_ref().cloned();
    let names: Vec<String> = app.plugins.items.iter().map(|i| i.name.clone()).collect();
    if sup_opt.is_none() || names.is_empty() {
        return;
    }

    let tx = app.exec_sender.clone();
    tokio::spawn(async move {
        let mcp_client_mgr = sup_opt.unwrap();
        let mut arr = Vec::new();
        for name in names {
            if let Some(client) = mcp_client_mgr.get_client(&name).await {
                let mut _client = client.lock().await;
                let status_result = mcp_client_mgr.get_plugin_status(&name).await;
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
        let _ = tx.send(heroku_types::ExecOutcome {
            log: "Plugins: refreshed".into(),
            result_json: Some(payload),
            open_table: false,
            pagination: None,
        });
    });
}

/// Refresh recent logs for the given plugin and dispatch payload.
fn execute_plugins_refresh_logs(app: &mut app::App, name: String) {
    let sup_opt = app.ctx.mcp.as_ref().cloned();
    if sup_opt.is_none() {
        return;
    }
    let tx = app.exec_sender.clone();
    tokio::spawn(async move {
        let mcp_client_mgr = sup_opt.unwrap();
        let lines = mcp_client_mgr.log_manager().get_recent_logs(&name, 500).await;
        let payload = json!({ "plugins_logs": { "name": name, "lines": lines } });
        let _ = tx.send(heroku_types::ExecOutcome {
            log: "Plugins: logs refreshed".into(),
            result_json: Some(payload),
            open_table: false,
            pagination: None,
        });
    });
}

/// Export logs to a default path in temp dir (redacted).
fn execute_plugins_export_default(app: &mut app::App, name: String) {
    let sup_opt = app.ctx.mcp.as_ref().cloned();
    if sup_opt.is_none() {
        return;
    }
    let tx = app.exec_sender.clone();
    tokio::spawn(async move {
        let mcp_client_mgr = sup_opt.unwrap();
        // Default temp path
        let mut path = std::env::temp_dir();
        path.push(format!("mcp_{}_logs.txt", name));
        let res = mcp_client_mgr.log_manager().export_logs(&name, &path).await;
        let msg = match res {
            Ok(_) => format!("Plugins: exported logs for '{}' to {}", name, path.display()),
            Err(e) => format!("Plugins: export logs for '{}' failed: {}", name, e),
        };
        let _ = tx.send(heroku_types::ExecOutcome {
            log: msg,
            result_json: None,
            open_table: false,
            pagination: None,
        });
    });
}

/// Open environment editor: load rows from config and dispatch a payload.
fn execute_plugins_open_env(app: &mut app::App, name: String) {
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
        "plugins_env": { "name": name, "rows": rows.iter().map(|r|json!({"key": r.key, "value": r.value, "is_secret": r.is_secret})).collect::<Vec<_>>() }
    });
    let _ = app.exec_sender.send(heroku_types::ExecOutcome {
        log: "Plugins: env loaded".into(),
        result_json: Some(payload),
        open_table: false,
        pagination: None,
    });
}

/// Save environment changes back to config (overwrites env map for the plugin).
fn execute_plugins_save_env(app: &mut app::App, name: String, rows: Vec<(String, String)>) {
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
        let _ = app.exec_sender.send(heroku_types::ExecOutcome {
            log: format!("Env save validation failed: {}", e),
            result_json: None,
            open_table: false,
            pagination: None,
        });
        return;
    }
    let _ = save_config_to_path(&cfg, &path);
    let _ = app.exec_sender.send(heroku_types::ExecOutcome {
        log: format!("Plugins: saved env for '{}'", name),
        result_json: None,
        open_table: false,
        pagination: None,
    });
}

/// Validate Add Plugin view input and emit a preview payload.
fn execute_plugins_validate_add(app: &mut app::App) {
    let Some(add_view_state) = &app.plugins.add else { return };
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
            // Optional headers input (comma-separated key=value)
            if !add_view_state.headers_input.trim().is_empty() {
                match parse_key_value_list_strict(add_view_state.headers_input.as_str()) {
                    Ok(map) => {
                        if !map.is_empty() {
                            server.headers = Some(map);
                        }
                    }
                    Err(errors) => {
                        ok = false;
                        message = format!("Invalid headers: {}", errors.join("; "));
                    }
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
            // Optional env input (comma-separated key=value)
            if !add_view_state.env_input.trim().is_empty() {
                match parse_key_value_list_strict(add_view_state.env_input.as_str()) {
                    Ok(map) => {
                        if !map.is_empty() {
                            server.env = Some(map);
                        }
                    }
                    Err(errors) => {
                        ok = false;
                        message = format!("Invalid env vars: {}", errors.join("; "));
                    }
                }
            }
        }
    }

    // Build a preview payload; skip live health probe (rmcp connects at start).
    let patch = build_add_patch(name, &server);
    let payload = json!({
        "plugins_add_preview": { "ok": ok, "message": message, "patch": patch }
    });
    let _ = app.exec_sender.send(heroku_types::ExecOutcome {
        log: format!("Add validate: {}", name),
        result_json: Some(payload),
        open_table: false,
        pagination: None,
    });
}

/// Apply Add Plugin view: write server to config and refresh plugins list.
fn execute_plugins_apply_add(app: &mut app::App) {
    let Some(add_view_state) = &app.plugins.add else { return };
    let name = add_view_state.name.trim().to_string();
    let mut server = McpServer::default();
    match add_view_state.transport {
        AddTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                let _ = app.exec_sender.send(heroku_types::ExecOutcome {
                    log: "Add apply validation failed: invalid Base URL".into(),
                    result_json: None,
                    open_table: false,
                    pagination: None,
                });
                return;
            }
            // Optional headers input (comma-separated key=value)
            if !add_view_state.headers_input.trim().is_empty() {
                match parse_key_value_list_strict(add_view_state.headers_input.as_str()) {
                    Ok(map) => {
                        if !map.is_empty() {
                            server.headers = Some(map);
                        }
                    }
                    Err(errors) => {
                        let _ = app.exec_sender.send(heroku_types::ExecOutcome {
                            log: format!("Add apply validation failed: invalid headers: {}", errors.join("; ")),
                            result_json: None,
                            open_table: false,
                            pagination: None,
                        });
                        return;
                    }
                }
            }
        }
        AddTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                let _ = app.exec_sender.send(heroku_types::ExecOutcome {
                    log: "Add apply validation failed: command is required".into(),
                    result_json: None,
                    open_table: false,
                    pagination: None,
                });
                return;
            }
            server.command = Some(command.to_string());
            if !add_view_state.args.trim().is_empty() {
                let parsed: Vec<String> = add_view_state.args.split_whitespace().map(|s| s.to_string()).collect();
                server.args = Some(parsed);
            }
            // Optional env input (comma-separated key=value)
            if !add_view_state.env_input.trim().is_empty() {
                match parse_key_value_list_strict(add_view_state.env_input.as_str()) {
                    Ok(map) => {
                        if !map.is_empty() {
                            server.env = Some(map);
                        }
                    }
                    Err(errors) => {
                        let _ = app.exec_sender.send(heroku_types::ExecOutcome {
                            log: format!("Add apply validation failed: invalid env vars: {}", errors.join("; ")),
                            result_json: None,
                            open_table: false,
                            pagination: None,
                        });
                        return;
                    }
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
        let _ = app.exec_sender.send(heroku_types::ExecOutcome {
            log: format!("Add apply validation failed: {}", e),
            result_json: None,
            open_table: false,
            pagination: None,
        });
        return;
    }
    let _ = save_config_to_path(&cfg, &path);
    // Refresh list
    execute_load_plugins(app);

    // Dismiss Add view and select the newly added plugin if present
    app.plugins.add = None;
    if let Some(idx) = app.plugins.items.iter().position(|it| it.name == name) {
        app.plugins.selected = Some(idx);
    }

    let _ = app.exec_sender.send(heroku_types::ExecOutcome {
        log: format!("Plugins: added '{}'", name),
        result_json: None,
        open_table: false,
        pagination: None,
    });
}

fn execute_plugins_cancel(app: &mut app::App) {
    app.plugins.close_secrets();
}

fn build_add_patch(name: &str, server: &heroku_mcp::config::McpServer) -> String {
    let mut map = Map::new();
    let v = to_value(server).unwrap_or(serde_json::json!({}));
    let mut servers = Map::new();
    servers.insert(name.to_string(), v);
    map.insert("mcpServers".to_string(), Value::Object(servers));
    to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_default()
}

/// Strict validator for comma-separated `key=value` pairs.
fn parse_key_value_list_strict(input: &str) -> Result<std::collections::HashMap<String, String>, Vec<String>> {
    let mut out = std::collections::HashMap::new();
    let mut errors: Vec<String> = Vec::new();
    for (idx, pair) in input.split(',').enumerate() {
        let raw = pair;
        let p = pair.trim();
        if p.is_empty() {
            continue;
        }
        match p.split_once('=') {
            Some((k, v)) => {
                let key = k.trim();
                let val = v.trim();
                if key.is_empty() {
                    errors.push(format!("segment {} has empty key: '{}'", idx + 1, raw));
                } else {
                    out.insert(key.to_string(), val.to_string());
                }
            }
            None => errors.push(format!("segment {} missing '=': '{}'", idx + 1, raw)),
        }
    }
    if errors.is_empty() { Ok(out) } else { Err(errors) }
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
fn execute_http(app: &mut app::App, spec: CommandSpec, body: Map<String, Value>) {
    // Live request: spawn async task and show throbber
    app.executing = true;
    app.throbber_idx = 0;

    let tx = app.exec_sender.clone();
    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);

    tokio::spawn(async move {
        let outcome = exec_remote(&spec, body).await;

        match outcome {
            Ok(out) => {
                let _ = tx.send(out);
            }
            Err(err) => {
                let _ = tx.send(heroku_types::ExecOutcome {
                    log: format!("Error: {}", err),
                    result_json: None,
                    open_table: false,
                    pagination: None,
                });
            }
        }

        // Mark one execution completed
        active.fetch_sub(1, Ordering::Relaxed);
    });
}
