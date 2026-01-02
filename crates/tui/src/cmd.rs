//! # Command Execution Layer
//!
//! This module translates high-level application effects (`Effect`) into
//! imperative commands (`Cmd`) and executes them. It provides the "boundary"
//! where the pure state management of the app interacts with side effects
//! such as
//! - Writing to the system clipboard
//! - Making live API calls to Oatty
//! - Spawning background tasks and recording logs
//!
//! ## Design
//! - [`Cmd`] is the effectful command type (clipboard / http).
//! - [`run_from_effects`] translates state-driven [`Effect`]s into [`Cmd`]s.
//! - [`run_cmds`] takes these commands and executes them, ensuring logs remain
//!   user-visible.
//! - [`execute_http`] and [`exec_remote_from_shell_command`] handle async HTTP requests and return
//!   structured [`ExecOutcome`] for UI presentation.
//!
//! This design follows a **functional core, imperative shell** pattern:
//! state updates are pure, but commands handle side effects.

use crate::app::App;
use crate::ui::components::common::key_value_editor::EnvRow;
use crate::ui::components::plugins::PluginTransport;
use anyhow::Result;
use anyhow::anyhow;
use chrono::Utc;
use oatty_engine::{RegistryCommandRunner, drive_workflow_run, provider::ProviderFetchPlan};
use oatty_mcp::config::{
    McpServer, default_config_path, determine_env_source, load_config_from_path, save_config_to_path, validate_config, validate_server_name,
};
use oatty_mcp::{McpConfig, PluginEngine};
use oatty_registry::find_by_group_and_cmd;
use oatty_registry::{CommandRegistry, CommandSpec};
use oatty_registry_gen::io::ManifestInput;
use oatty_registry_gen::io::generate_catalog;
use oatty_types::{DirectoryEntry, Effect, EnvVar, Msg, WorkflowRunControl, WorkflowRunEvent, WorkflowRunRequest, WorkflowRunStatus};
use oatty_types::{ExecOutcome, command::CommandExecution};
use oatty_util::build_request_body;
use oatty_util::exec_remote_from_shell_command;
use oatty_util::fetch_static;
use oatty_util::lex_shell_like;
use serde_json::Map;
use serde_json::Value;
use serde_json::from_str;
use std::collections::VecDeque;
use std::fs::read_dir;
use std::fs::read_to_string;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::vec;
use tokio::{sync::mpsc, task::JoinHandle};
use url::Url;

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

    /// Make an HTTP request to the Oatty API.
    ///
    /// Carries:
    /// - [`CommandSpec`]: API request metadata (including a path, method, and base URL)
    /// - `serde_json::Map`: JSON body
    ///
    /// # Example
    /// ```rust,ignore
    /// use your_crate::Cmd;
    /// use oatty_registry::{CommandSpec, HttpCommandSpec};
    /// use oatty_types::CommandExecution;
    /// use std::collections::HashMap;
    /// use serde_json::{Map, Value};
    ///
    /// let http = HttpCommandSpec::new("GET", "/apps", "https://api.example.com", Vec::new(), None);
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
    ExecuteHttp {
        spec: CommandSpec,
        input: String,
        next_range_override: Option<String>,
        request_id: u64,
    },
    /// Fetch provider-backed suggestion values asynchronously.
    FetchProviderValues {
        provider_id: String,
        cache_key: String,
        args: Map<String, Value>,
    },
    /// Invoke an MCP tool via the plugin engine.
    ExecuteMcp(CommandSpec, Map<String, Value>, u64),
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
    SendMsg(Msg),
    ReadFileContents(PathBuf),
    ListDirectoryContents(PathBuf),
    ReadRemoteFileContents(Url),
    ImportRegistryCatalog(String),
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
    let mut effect_queue: VecDeque<Effect> = effects.into();

    while let Some(effect) = effect_queue.pop_front() {
        let effect_commands = match effect {
            Effect::CopyToClipboardRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
            Effect::CopyLogsRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
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
            Effect::PluginsValidateAdd => Some(vec![Cmd::PluginsValidate]),
            Effect::PluginsSave => Some(vec![Cmd::PluginsSave]),
            Effect::SendToPalette(spec) => {
                let result = handle_send_to_palette(app, spec);
                effect_queue.extend(app.rebuild_palette_suggestions());
                result
            }
            Effect::Run {
                hydrated_command,
                range_override,
                request_hash,
            } => run_command(app, hydrated_command, range_override, request_hash),
            Effect::ProviderFetchRequested {
                provider_id,
                cache_key,
                args,
            } => Some(vec![Cmd::FetchProviderValues {
                provider_id,
                cache_key,
                args,
            }]),
            Effect::WorkflowRunRequested { request } => {
                handle_workflow_run_requested(app, *request);
                None
            }
            Effect::WorkflowRunControl { run_id, command } => {
                handle_workflow_run_control(app, &run_id, command);
                None
            }
            Effect::SendMsg(nsg) => Some(vec![Cmd::SendMsg(nsg)]),
            Effect::ReadFileContents(path) => Some(vec![Cmd::ReadFileContents(path)]),
            Effect::ListDirectoryContents(path) => Some(vec![Cmd::ListDirectoryContents(path)]),
            Effect::ReadRemoteFileContents(url) => Some(vec![Cmd::ReadRemoteFileContents(url)]),
            Effect::ImportRegistryCatalog(content) => Some(vec![Cmd::ImportRegistryCatalog(content)]),
            Effect::Log(_) | Effect::SwitchTo(_) | Effect::ShowModal(_) | Effect::CloseModal => None,
        };
        if let Some(cmds) = effect_commands {
            commands.extend(cmds);
        }
    }

    run_cmds(app, commands).await
}

/// When pressing the Enter key in the browser, populate the palette with the
/// constructed command and close the command browser.
fn handle_send_to_palette(app: &mut App, command_spec: Box<CommandSpec>) -> Option<Vec<Cmd>> {
    let CommandSpec { group, name, .. } = *command_spec;

    app.palette.set_input(format!("{} {}", group, name));
    app.palette.set_cursor(app.palette.input().len());
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
        let (immediate, background) = match command {
            Cmd::ApplyPaletteError(error) => (Some(apply_palette_error(app, error)), None),
            Cmd::ClipboardSet(text) => (Some(execute_clipboard_set(app, text)), None),
            Cmd::ExecuteHttp {
                spec,
                input,
                next_range_override,
                request_id,
            } => (None, Some(spawn_execute_http(app, spec, input, next_range_override, request_id))),
            Cmd::FetchProviderValues {
                provider_id,
                cache_key,
                args,
            } => (None, Some(spawn_fetch_provider_values(app, provider_id, cache_key, args))),
            Cmd::ExecuteMcp(spec, body, request_id) => (None, Some(spawn_execute_mcp(app, spec, body, request_id))),
            Cmd::PluginsStart(name) => (None, Some(spawn_execute_plugin_action(app, PluginAction::Start, name))),
            Cmd::PluginsStop(name) => (None, Some(spawn_execute_plugin_action(app, PluginAction::Stop, name))),
            Cmd::PluginsRestart(name) => (None, Some(spawn_execute_plugin_action(app, PluginAction::Restart, name))),
            Cmd::PluginsLoadDetail(name) => (None, Some(spawn_load_plugin_detail(app, name))),
            Cmd::LoadPlugins => (Some(execute_load_plugins(app).await), None),
            Cmd::PluginsRefresh => (Some(execute_plugins_refresh(app).await), None),
            Cmd::PluginsExportLogsDefault(name) => (Some(execute_plugins_export_default(app, name).await), None),
            Cmd::PluginsValidate => (Some(execute_plugins_validate(app)), None),
            Cmd::PluginsSave => (Some(execute_plugins_save(app).await), None),
            Cmd::SendMsg(msg) => (Some(ExecOutcome::Message(msg)), None),
            Cmd::ReadFileContents(path) => (Some(read_file_contents(path)), None),
            Cmd::ListDirectoryContents(path) => (Some(list_dir_contents(path)), None),
            Cmd::ReadRemoteFileContents(url) => (None, Some(fetch_remote_file_contents(url))),
            Cmd::ImportRegistryCatalog(inputs) => (Some(import_registry_catalog_from(app, inputs)), None),
        };

        if let Some(immediate) = immediate {
            batch.immediate.push(immediate);
        }
        if let Some(pending) = background {
            batch.pending.push(pending);
        }
    }
    batch
}
/// List directory contents
fn list_dir_contents(path: PathBuf) -> ExecOutcome {
    match read_dir(&path) {
        Ok(entries) => {
            let mut entries: Vec<DirectoryEntry> = entries
                .flatten()
                .filter(|entry| entry.file_name().as_bytes().first() != Some(&b'.'))
                .map(|entry| DirectoryEntry {
                    path: entry.path(),
                    is_directory: entry.metadata().is_ok_and(|f| f.is_dir()),
                })
                .collect();
            entries.sort_by(|a, b| {
                b.is_directory
                    .cmp(&a.is_directory)
                    .then(a.path.file_name().cmp(&b.path.file_name()))
            });
            if let Some(parent) = path.parent() {
                let mut contents: Vec<DirectoryEntry> = Vec::with_capacity(entries.len() + 1);
                contents.push(DirectoryEntry {
                    path: parent.to_path_buf(),
                    is_directory: true,
                });
                contents.append(&mut entries);
                entries = contents
            };
            ExecOutcome::DirectoryContents { entries, root_path: path }
        }
        Err(e) => ExecOutcome::Log(format!("Failed to read directory: {}", e)),
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
fn execute_clipboard_set(app: &mut App, text: String) -> ExecOutcome {
    // Perform clipboard write and log outcome
    if let Err(e) = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
        app.append_log_message(format!("Clipboard error: {}", e));
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

/// Fetches remote file contents.
fn fetch_remote_file_contents(url: Url) -> JoinHandle<ExecOutcome> {
    tokio::spawn(async move {
        match fetch_static(url.as_str()).await {
            Ok((status, content)) => {
                if status.as_u16() > 299 {
                    return ExecOutcome::Log(format!("Failed to fetch remote file: status: {} message: {}", status, content));
                }
                ExecOutcome::RemoteFileContents(content, url)
            }
            Err(e) => ExecOutcome::Log(format!("Failed to fetch remote file: {}", e)),
        }
    })
}

/// Reads file contents from a local file path.
fn read_file_contents(path: PathBuf) -> ExecOutcome {
    match read_to_string(&path) {
        Ok(contents) => ExecOutcome::FileContents(contents, path),
        Err(e) => ExecOutcome::Log(format!("Failed to read file: {}", e)),
    }
}

/// Generates a catalog from a given content.
fn import_registry_catalog_from(app: &mut App, content: String) -> ExecOutcome {
    let input = ManifestInput::new(None, Some(content));
    match generate_catalog(input) {
        Ok(catalog) => ExecOutcome::RegistryCatalog(catalog),
        Err(e) => ExecOutcome::Log(format!("Failed to generate catalog: {}", e)),
    }
}

/// Execute a plugin lifecycle action using the MCP supervisor.
fn spawn_execute_plugin_action(app: &mut App<'_>, action: PluginAction, name: String) -> JoinHandle<ExecOutcome> {
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
        ExecOutcome::PluginDetail {
            message: msg,
            detail: detail_result,
        }
    })
}

fn spawn_load_plugin_detail(app: &mut App<'_>, name: String) -> JoinHandle<ExecOutcome> {
    let plugin_engine = app.ctx.plugin_engine.clone();
    tokio::spawn(async move {
        match plugin_engine.get_plugin_detail(&name).await {
            Ok(detail) => ExecOutcome::PluginDetailLoad {
                plugin_name: name,
                result: Ok(detail),
            },
            Err(error) => ExecOutcome::PluginDetailLoad {
                plugin_name: name,
                result: Err(error.to_string()),
            },
        }
    })
}

/// Refresh plugin statuses/health and dispatch a payload through ExecOutcome.result_json.
async fn execute_plugins_refresh(app: &mut App<'_>) -> ExecOutcome {
    let plugin_engine = &*app.ctx.plugin_engine;
    let plugins = plugin_engine.list_plugins().await;

    app.browser.update_browser_filtered();

    ExecOutcome::PluginsRefresh {
        message: format!("{} plugins refreshed", plugins.len()),
        details: Some(plugins),
    }
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
        return ExecOutcome::PluginValidationErr { message: e.to_string() };
    }

    // Build server candidate based on selected transport
    let mut server = McpServer::default();
    match add_view_state.transport {
        PluginTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if base_url.is_empty() {
                return ExecOutcome::PluginValidationErr {
                    message: "Base URL is required for remote transport".into(),
                };
            } else if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                return ExecOutcome::PluginValidationErr {
                    message: "Invalid Base URL".into(),
                };
            }
            match collect_key_value_rows(&add_view_state.kv_editor.rows) {
                Ok(Some(map)) => {
                    server.headers = Some(map);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome::PluginValidationErr {
                        message: format!("Invalid headers: {}", errors.join("; ")),
                    };
                }
            }
        }
        PluginTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                return ExecOutcome::PluginValidationErr {
                    message: "Command is required for local transport".into(),
                };
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
                    return ExecOutcome::PluginValidationErr {
                        message: format!("Invalid env vars: {}", errors.join("; ")),
                    };
                }
            }
        }
    }

    ExecOutcome::PluginValidationOk {
        message: "✓ Looks good!".to_string(),
    }
}

/// Apply Add Plugin view: write server to config and refresh plugins list.
async fn execute_plugins_save(app: &mut App<'_>) -> ExecOutcome {
    let Some(add_view_state) = &app.plugins.add else {
        return ExecOutcome::default();
    };
    let name = add_view_state.name.trim().to_string();
    let original_name = add_view_state.original_name.as_deref();
    let mut server = McpServer::default();
    match add_view_state.transport {
        PluginTransport::Remote => {
            let base_url = add_view_state.base_url.trim();
            if let Ok(url) = Url::parse(base_url) {
                server.base_url = Some(url);
            } else {
                return ExecOutcome::PluginValidationErr {
                    message: "Plugin validation failed: invalid Base URL".into(),
                };
            }
            match collect_key_value_rows(&add_view_state.kv_editor.rows) {
                Ok(Some(envs)) => {
                    server.headers = Some(envs);
                }
                Ok(None) => {}
                Err(errors) => {
                    return ExecOutcome::PluginValidationErr {
                        message: format!("plugin validation failed: invalid headers: {}", errors.join("; ")),
                    };
                }
            }
        }
        PluginTransport::Local => {
            let command = add_view_state.command.trim();
            if command.is_empty() {
                return ExecOutcome::PluginValidationErr {
                    message: "Plugin validation failed: command is required".into(),
                };
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
                    return ExecOutcome::PluginValidationErr {
                        message: format!("Plugin validation failed: invalid env vars: {}", errors.join("; ")),
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
    apply_plugin_name_change(&mut cfg, original_name, &name);
    cfg.mcp_servers.insert(name.clone(), server);
    if let Err(e) = validate_config(&cfg) {
        return ExecOutcome::PluginValidationErr {
            message: format!("Plugin validation failed: {}", e),
        };
    }
    if let Err(error) = save_config_to_path(&mut cfg, &path) {
        return ExecOutcome::PluginValidationErr {
            message: format!("Failed to save MCP configuration: {error}"),
        };
    }

    let runtime_cfg = match load_config_from_path(&path) {
        Ok(config) => config,
        Err(error) => {
            return ExecOutcome::PluginValidationErr {
                message: format!("Saved plugin, but failed to reload configuration: {error}"),
            };
        }
    };

    if let Err(error) = app.ctx.plugin_engine.update_config(runtime_cfg).await {
        return ExecOutcome::PluginValidationErr {
            message: format!("Saved plugin, but failed to refresh MCP engine: {error}"),
        };
    }

    // Refresh list
    execute_load_plugins(app).await;

    // Dismiss Add view and select the newly added plugin if present
    app.plugins.add = None;
    if let Some(index) = app.plugins.table.items.iter().position(|item| item.name == name) {
        app.plugins.table.table_state.select(Some(index));
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

/// Remove the previous plugin entry when the user renames a server.
fn apply_plugin_name_change(config: &mut McpConfig, original_name: Option<&str>, desired_name: &str) {
    if let Some(previous) = original_name
        && previous != desired_name
    {
        config.mcp_servers.remove(previous);
    }
}

fn handle_workflow_run_requested(app: &mut App<'_>, request: WorkflowRunRequest) {
    let run_id = request.run_id.clone();

    let registry_snapshot = match app.ctx.command_registry.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => {
            app.logs
                .entries
                .push("Failed to obtain command registry for workflow run.".to_string());
            return;
        }
    };

    let runner = Arc::new(RegistryCommandRunner::new(registry_snapshot));
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (control_tx, control_rx) = mpsc::unbounded_channel();

    let request_clone = request.clone();
    let run_id_clone = run_id.clone();
    let runner_clone = Arc::clone(&runner);
    let event_tx_clone = event_tx.clone();

    tokio::spawn(async move {
        if let Err(error) = drive_workflow_run(request_clone, runner_clone, control_rx, event_tx_clone).await {
            let message = format!("Workflow run '{}' failed: {}", run_id_clone, error);
            let _ = event_tx.send(WorkflowRunEvent::RunStatusChanged {
                status: WorkflowRunStatus::Failed,
                message: Some(message.clone()),
            });
            let _ = event_tx.send(WorkflowRunEvent::RunCompleted {
                status: WorkflowRunStatus::Failed,
                finished_at: Utc::now(),
                error: Some(message),
            });
        }
    });

    app.workflows.register_run_control(&run_id, control_tx);
    app.register_workflow_run_stream(run_id, event_rx);
}

fn handle_workflow_run_control(app: &mut App<'_>, run_id: &str, command: WorkflowRunControl) {
    match app.workflows.run_control_sender(run_id) {
        Some(sender) => {
            if sender.send(command).is_err() {
                app.logs
                    .entries
                    .push(format!("Workflow run '{}' is no longer accepting commands.", run_id));
            }
        }
        None => {
            app.logs
                .entries
                .push(format!("No active workflow run is available for '{}'.", run_id));
        }
    }
}

/// Spawn an HTTP execution on the Tokio scheduler while updating local state
/// to show the spinner.
fn spawn_execute_http(
    app: &mut App<'_>,
    spec: CommandSpec,
    input: String,
    next_range_override: Option<String>,
    request_id: u64,
) -> JoinHandle<ExecOutcome> {
    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);

    let base_url = app
        .ctx
        .command_registry
        .lock()
        .ok()
        .and_then(|registry| registry.resolve_base_url_for_command(&spec))
        .or_else(|| spec.http().map(|http| http.base_url.clone()));

    tokio::spawn(async move { execute_http_task(active, spec, input, base_url, next_range_override, request_id).await })
}

fn spawn_fetch_provider_values(app: &App, provider_id: String, cache_key: String, args: Map<String, Value>) -> JoinHandle<ExecOutcome> {
    let registry = Arc::clone(&app.ctx.provider_registry);

    tokio::task::spawn_blocking(move || {
        let plan = ProviderFetchPlan::new(provider_id.clone(), cache_key.clone(), args);
        match registry.complete_fetch(&plan) {
            Ok(values) => ExecOutcome::ProviderValues {
                provider_id,
                cache_key,
                values,
                request_id: None,
            },
            Err(error) => ExecOutcome::Log(format!("Provider fetch failed: {error}")),
        }
    })
}

fn spawn_execute_mcp(app: &mut App<'_>, spec: CommandSpec, arguments: Map<String, Value>, request_id: u64) -> JoinHandle<ExecOutcome> {
    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);
    let engine = app.ctx.plugin_engine.clone();

    tokio::spawn(async move { execute_mcp_task(active, engine, spec, arguments, request_id).await })
}

/// Background task body for executing an HTTP request and translating it into
/// an [`ExecOutcome`].
async fn execute_http_task(
    active_exec_count: Arc<AtomicUsize>,
    spec: CommandSpec,
    input: String,
    base_url: Option<String>,
    next_range_override: Option<String>,
    request_id: u64,
) -> ExecOutcome {
    let result = exec_remote_from_shell_command(&spec, base_url, input, next_range_override, request_id).await;
    let outcome = result.unwrap_or_else(|err| ExecOutcome::Log(format!("Error: {}", err)));

    active_exec_count.fetch_sub(1, Ordering::Relaxed);

    outcome
}

async fn execute_mcp_task(
    active_exec_count: Arc<AtomicUsize>,
    engine: Arc<PluginEngine>,
    spec: CommandSpec,
    arguments: Map<String, Value>,
    request_id: u64,
) -> ExecOutcome {
    let result = engine.execute_tool(&spec, &arguments, request_id).await;
    let outcome = result.unwrap_or_else(|err| ExecOutcome::Log(format!("Error: {}", err)));

    active_exec_count.fetch_sub(1, Ordering::Relaxed);

    outcome
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
fn run_command(app: &mut App, hydrated_command: String, next_range_override: Option<String>, request_id: u64) -> Option<Vec<Cmd>> {
    let valid = validate_command(app, &hydrated_command, Arc::clone(&app.ctx.command_registry));

    match valid {
        Ok((command_spec, input)) => {
            app.append_log_message_with_level(Some("info".to_string()), format!("Running: {}", &hydrated_command));
            execute_command(command_spec, input, next_range_override, request_id)
        }
        Err(error) => Some(vec![Cmd::ApplyPaletteError(error.to_string())]),
    }
}

fn validate_command(app: &mut App, hydrated_command: &str, command_registry: Arc<Mutex<CommandRegistry>>) -> Result<(CommandSpec, String)> {
    // Step 1: Parse and validate the palette input
    let input = hydrated_command.trim().to_string();
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
        let lock = command_registry.lock().map_err(|_| anyhow!("Could not obtain lock to registry"))?;
        let commands = &lock.commands;
        find_by_group_and_cmd(commands, tokens[0].as_str(), tokens[1].as_str())?
    };
    command_spec.parse_arguments(&tokens[2..])?;
    persist_execution_context(app, &command_spec, &input);

    Ok((command_spec, input))
}

fn persist_execution_context(app: &mut App, command_spec: &CommandSpec, input: &str) {
    let trimmed_input = input.trim();
    app.palette
        .record_pending_execution(command_spec.canonical_id(), trimmed_input.to_string());
    app.palette.push_history_if_needed(trimmed_input);
}

fn execute_command(
    command_spec: CommandSpec,
    hydrated_shell_command: String,
    next_range_override: Option<String>,
    request_id: u64,
) -> Option<Vec<Cmd>> {
    match command_spec.execution() {
        CommandExecution::Http(_) => {
            let command_spec_to_run = command_spec.clone();
            Some(vec![Cmd::ExecuteHttp {
                spec: command_spec_to_run,
                input: hydrated_shell_command,
                next_range_override,
                request_id,
            }])
        }
        CommandExecution::Mcp(_) => {
            let tokens = lex_shell_like(&hydrated_shell_command);
            let (user_flags, user_args) = command_spec.parse_arguments(&tokens[2..]).ok()?;
            let mut body = build_request_body(&command_spec, user_flags);
            for (arg, value) in user_args.iter() {
                body.insert(arg.to_string(), Value::String(value.to_string()));
            }

            Some(vec![Cmd::ExecuteMcp(command_spec, body, request_id)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_plugin_name_change_removed_old_key() {
        let mut config = McpConfig::default();
        config.mcp_servers.insert("old".into(), McpServer::default());
        apply_plugin_name_change(&mut config, Some("old"), "new");

        assert!(!config.mcp_servers.contains_key("old"));
    }

    #[test]
    fn apply_plugin_name_change_keeps_existing_when_name_same() {
        let mut config = McpConfig::default();
        config.mcp_servers.insert("same".into(), McpServer::default());
        apply_plugin_name_change(&mut config, Some("same"), "same");

        assert!(config.mcp_servers.contains_key("same"));
    }
}
