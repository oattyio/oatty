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
//! - [`from_effects`] translates state-driven [`Effect`]s into [`Cmd`]s.
//! - [`run_cmds`] takes these commands and executes them, ensuring logs remain
//!   user-visible.
//! - [`execute_http`] and [`exec_remote`] handle async HTTP requests and return
//!   structured [`ExecOutcome`] for UI presentation.
//!
//! This design follows a **functional core, imperative shell** pattern:
//! state updates are pure, but commands handle side effects.

use std::sync::atomic::Ordering;

use heroku_registry::CommandSpec;
use heroku_util::exec_remote;
// Types imported as needed; HTTP helpers moved to heroku-util
use serde_json::Value;
use tokio::task::spawn;

use crate::app::{self, Effect};

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
    /// use serde_json::{Map, Value};
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
    ExecuteHttp(Box<CommandSpec>, serde_json::Map<String, Value>),
}

/// Convert application [`Effect`]s into actual [`Cmd`] instances.
///
/// This enables a clean separation: effects represent "what should happen",
/// while commands describe "how it should happen".
///
/// # Example
/// ```rust,ignore
/// # use your_crate::{from_effects, Cmd};
/// # struct DummyBuilder;
/// # impl DummyBuilder {
/// #   fn selected_command(&self) -> Option<&'static str> { Some("ls") }
/// #   fn input_fields(&self) -> Vec<&'static str> { vec![] }
/// # }
/// # struct DummyApp { builder: DummyBuilder }
/// # mod preview { pub fn cli_preview(_s: &str, _f: Vec<&str>) -> String { "ls".into() } }
/// # enum Effect { CopyCommandRequested }
/// # let mut app = DummyApp { builder: DummyBuilder };
/// # let effects = vec![Effect::CopyCommandRequested];
/// // Translates an effect into a side-effectful command
/// let cmds = from_effects(&mut app, effects);
/// assert!(matches!(cmds, Cmd::ClipboardSet(_)));
/// ```
pub fn from_effects(app: &mut app::App, effects: Vec<Effect>) -> Vec<Cmd> {
    let mut commands = Vec::new();

    for effect in effects {
        let effect_commands = match effect {
            Effect::CopyCommandRequested => handle_copy_command_requested(app),
            Effect::CopyLogsRequested(text) => Some(vec![Cmd::ClipboardSet(text)]),
            Effect::NextPageRequested(next_raw) => handle_next_page_requested(app, next_raw),
            Effect::PrevPageRequested => handle_prev_page_requested(app),
            Effect::FirstPageRequested => handle_first_page_requested(app),
        };

        commands.extend(effect_commands.unwrap());
    }

    commands
}

/// Handle the copy command requested effect by determining what to copy based on current focus.
///
/// Returns a vector containing either a clipboard command for the selected command
/// or selected table data, depending on which component has focus.
///
/// # Arguments
/// * `app` - The application state containing builder and table components
///
/// # Returns
/// A vector of commands, typically containing a single clipboard command
fn handle_copy_command_requested(app: &app::App) -> Option<Vec<Cmd>> {
    let mut commands = Vec::new();

    match (app.builder.is_visible(), app.table.grid_focus().get()) {
        (true, false) => {
            // Builder is focused - copy the selected command
            if let Some(spec) = app.builder.selected_command() {
                let command_text = crate::preview::cli_preview(spec, app.builder.input_fields());
                commands.push(Cmd::ClipboardSet(command_text));
            }
        }
        (false, true) => {
            // Table is focused - copy selected row data
            if let Ok(stringified) = serde_json::to_string(app.table.selected_data()?) {
                commands.push(Cmd::ClipboardSet(stringified));
            }
        }
        _ => {
            // Neither component focused or both focused (edge case)
            // No action needed
        }
    }

    Some(commands)
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
        body.insert("next-range".into(), serde_json::Value::String(next_raw.clone()));

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
            body.insert("next-range".into(), serde_json::Value::String(prev));
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
            body.insert("next-range".into(), serde_json::Value::String(first));
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
/// # use serde_json::Map;
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
fn execute_http(app: &mut app::App, spec: CommandSpec, body: serde_json::Map<String, Value>) {
    // Live request: spawn async task and show throbber
    app.executing = true;
    app.throbber_idx = 0;

    let tx = app.exec_sender.clone();
    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);

    spawn(async move {
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
