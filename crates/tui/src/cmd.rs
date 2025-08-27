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
use crate::app::{self, Effect};
use heroku_registry::CommandSpec;
use heroku_types::ExecOutcome;
use serde_json::Value;
use std::thread::spawn;
use tokio::runtime::Runtime;

/// Represents side-effectful system commands executed outside of pure state updates.
///
/// These commands bridge between the application's functional state model
/// and imperative actions (I/O, networking, system integration).
#[derive(Debug)]
pub enum Cmd {
    /// Write text into the system clipboard.
    ///
    /// # Example
    /// ```
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
    /// ```
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
    ExecuteHttp(CommandSpec, String, serde_json::Map<String, Value>),
}

/// Convert application [`Effect`]s into actual [`Cmd`] instances.
///
/// This enables a clean separation: effects represent "what should happen",
/// while commands describe "how it should happen".
///
/// # Example
/// ```
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
    let mut out = Vec::new();
    for eff in effects {
        match eff {
            Effect::CopyCommandRequested => {
                if let Some(spec) = app.builder.selected_command() {
                    let cmd = crate::preview::cli_preview(spec, app.builder.input_fields());
                    out.push(Cmd::ClipboardSet(cmd));
                }
            }
        }
    }
    out
}

/// Execute a sequence of commands and update application logs.
///
/// Each command corresponds to a user-visible side effect, such as writing
/// content to the clipboard or making a network call. Logs are appended with
/// human-readable results.
///
/// # Example
/// ```
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
            Cmd::ClipboardSet(text) => {
                // Perform clipboard write and log outcome
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
                    Ok(()) => app.logs.entries.push(format!("Copied: {}", text)),
                    Err(e) => app.logs.entries.push(format!("Clipboard error: {}", e)),
                }
                // Limit log size
                let log_len = app.logs.entries.len();
                if log_len > 500 {
                    let _ = app.logs.entries.drain(0..log_len - 500);
                }
            }
            Cmd::ExecuteHttp(spec, path, body) => {
                execute_http(app, spec, path, body);
            }
        }
    }
}

/// Spawn a background thread to execute a Heroku API request.
///
/// Updates the application state to indicate execution has begun, attaches
/// a channel receiver for results, and spawns a worker thread that runs a
/// Tokio runtime for the async HTTP call.
///
/// # Example
/// ```
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
fn execute_http(app: &mut app::App, spec: CommandSpec, path: String, body: serde_json::Map<String, Value>) {
    // Live request: spawn background task and show throbber
    let (tx, rx) = std::sync::mpsc::channel::<heroku_types::ExecOutcome>();
    app.exec_receiver = Some(rx);
    app.executing = true;
    app.throbber_idx = 0;

    spawn(move || {
        let runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                let _ = tx.send(heroku_types::ExecOutcome {
                    log: format!("Error: failed to start runtime: {}", e),
                    result_json: None,
                    open_table: false,
                });
                return;
            }
        };

        let outcome = runtime.block_on(exec_remote(spec, path, body));

        match outcome {
            Ok(out) => {
                let _ = tx.send(out);
            }
            Err(err) => {
                let _ = tx.send(heroku_types::ExecOutcome {
                    log: format!("Error: {}", err),
                    result_json: None,
                    open_table: false,
                });
            }
        }
    });
}

/// Perform an asynchronous REST API call against the Heroku platform.
///
/// Handles constructing the request, injecting authentication, sending, and
/// decoding JSON responses into a structured [`ExecOutcome`].
///
/// # Example
/// ```
/// use your_crate::exec_remote;
/// use heroku_registry::CommandSpec;
/// use serde_json::Map;
///
/// # async fn demo() {
/// let spec = CommandSpec { method: "GET".into(), path: "/apps".into() };
/// let outcome = exec_remote(spec, "/apps".into(), Map::new()).await;
/// match outcome {
///     Ok(result) => println!("Log: {}", result.log),
///     Err(err) => eprintln!("API call failed: {}", err),
/// }
/// # }
/// ```
///
/// # Errors
/// Returns `Err(String)` if authentication, network, or permissions fail.
async fn exec_remote(
    spec: CommandSpec,
    path: String,
    body: serde_json::Map<String, Value>,
) -> Result<ExecOutcome, String> {
    let client = heroku_api::HerokuClient::new_from_env().map_err(|e| {
        format!(
            "Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc",
            e
        )
    })?;
    let method = match spec.method.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        other => return Err(format!("unsupported method: {}", other)),
    };
    let mut builder = client.request(method, &path);
    if !body.is_empty() {
        builder = builder.json(&serde_json::Value::Object(body.clone()));
    }
    let resp = builder.send().await.map_err(|e| {
        format!(
            "Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set",
            e
        )
    })?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.as_u16() == 401 {
        return Err(
            "Unauthorized (401). Hint: set HEROKU_API_KEY=... or configure ~/.netrc with machine api.heroku.com".into(),
        );
    }
    if status.as_u16() == 403 {
        return Err("Forbidden (403). Hint: check team/app access, permissions, and role membership".into());
    }
    let log = format!("{}\n{}", status, text);
    let mut result_json = None;
    let mut open_table = false;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        open_table = true;
        result_json = Some(json);
    }
    Ok(heroku_types::ExecOutcome {
        log,
        result_json,
        open_table,
    })
}
