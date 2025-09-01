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
use crate::{app::{self, Effect}};
use heroku_registry::CommandSpec;
use heroku_types::{ExecOutcome, Pagination};
use serde_json::Value;
use std::sync::{
    atomic::{Ordering},
};
use tokio::task::spawn;
use reqwest::header::{HeaderMap, HeaderName, CONTENT_RANGE};

/// Represents side-effectful system commands executed outside of pure state updates.
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
    ExecuteHttp(Box<CommandSpec>, String, serde_json::Map<String, Value>),
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
    let mut out = Vec::new();
    for eff in effects {
        match eff {
            Effect::CopyCommandRequested => {
                if let Some(spec) = app.builder.selected_command() {
                    let cmd = crate::preview::cli_preview(spec, app.builder.input_fields());
                    out.push(Cmd::ClipboardSet(cmd));
                }
            }
            Effect::CopyLogsRequested(text) => {
                out.push(Cmd::ClipboardSet(text));
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
            Cmd::ClipboardSet(text) => {
                // Perform clipboard write and log outcome
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
                    Ok(()) => (),
                    Err(e) => app.logs.entries.push(format!("Clipboard error: {}", e)),
                }
                // Limit log size
                let log_len = app.logs.entries.len();
                if log_len > 500 {
                    let _ = app.logs.entries.drain(0..log_len - 500);
                }
            }
            Cmd::ExecuteHttp(spec, path, body) => {
                execute_http(app, *spec, path, body);
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
fn execute_http(app: &mut app::App, spec: CommandSpec, path: String, body: serde_json::Map<String, Value>) {
    // Live request: spawn async task and show throbber
    app.executing = true;
    app.throbber_idx = 0;
    let tx = app.exec_sender.clone();
    let active = app.active_exec_count.clone();
    active.fetch_add(1, Ordering::Relaxed);
    spawn(async move {
        let outcome = exec_remote(spec, path, body).await;
        match outcome {
            Ok(out) => {
                let _ = tx.send(out);
            }
            Err(err) => {
                let _ = tx.send(heroku_types::ExecOutcome {
                    log: format!("Error: {}", err),
                    result_json: None,
                    open_table: false,
                    pagination: None
                });
            }
        }
        // Mark one execution completed
        active.fetch_sub(1, Ordering::Relaxed);
    });
}

/// Perform an asynchronous REST API call against the Heroku platform.
///
/// Handles constructing the request, injecting authentication, sending, and
/// decoding JSON responses into a structured [`ExecOutcome`].
///
/// # Example
/// ```rust,ignore
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
    // Build Range header from special range fields if provided via inputs
    let field = body.get("range-field").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty());
    let start = body.get("range-start").and_then(|v| v.as_str()).unwrap_or("").trim();
    let end = body.get("range-end").and_then(|v| v.as_str()).unwrap_or("").trim();
    let order = body.get("order").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty());
    let max = body.get("max").and_then(|v| v.as_str()).and_then(|s| s.parse::<usize>().ok());

    if let Some(field) = field {
        // Compose range segment like "start..end" (allow one side empty as per API semantics)
        let range_seg = format!("{}..{}", start, end);
        let mut range_header = format!("{} {}", field, range_seg);
        // Append optional order/max parameters
        if let Some(ord) = order { range_header.push_str(&format!("; order={};", ord)); }
        if let Some(m) = max { range_header.push_str(&format!(" max={};", m)); }
        builder = builder.header("Range", range_header);
    }

    // Filter out special range-only fields from JSON body
    let mut body_filtered = body.clone();
    for k in ["range-field", "range-start", "range-end", "order", "max"] { let _ = body_filtered.remove(k); }
    if !body_filtered.is_empty() {
        builder = builder.json(&serde_json::Value::Object(body_filtered));
    }
    let resp = builder.send().await.map_err(|e| {
        format!(
            "Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set",
            e
        )
    })?;
    
    let status = resp.status();
    let headers = resp.headers().clone();
    let mut pagination = parse_content_range(&headers);
    // Attach Next-Range if present for client iteration
    if let Some(ref mut p) = pagination
        && let Some(nr) = parse_next_range(&headers) {
        p.next_range = Some(nr.0);
        if let Some(order) = nr.1 { p.order = Some(order); }
        // If max present in Next-Range and not in Content-Range, prefer it
        if let Some(max) = nr.2 && p.max == 0 { p.max = max; }
    }
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
        pagination
    })
}

// Parse Content-Range header into Pagination struct, returning None on failure
fn parse_content_range(headers: &HeaderMap) -> Option<Pagination> {
    // Get header value as string
    let value = headers.get(CONTENT_RANGE).and_then(|v| v.to_str().ok())?;

    // Split into tokens separated by ';' (e.g., "name app7a..app9x; max=200; order=desc;")
    let parts: Vec<&str> = value.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
    let range_part = parts.first()?;

    // Extract field and range (e.g., "name app7a..app9x" -> "name", "app7a..app9x")
    let (field, range) = range_part.split_once(' ')?;
    let field = field.to_lowercase(); // Normalize "NAME" to "name"

    // Split range into start and end (e.g., "app7a..app9x" -> ["app7a", "app9x"])
    let range_parts: Vec<&str> = range.split("..").collect();
    let range_start = range_parts.first().filter(|s| !s.is_empty())?.to_string();
    let range_end = range_parts.get(1).filter(|s| !s.is_empty())?.to_string();

    // Parse optional k=v params like max=200, order=desc
    let mut max: Option<usize> = None;
    let mut order: Option<String> = None;
    for kv in parts.iter().skip(1) {
        if let Some(v) = kv.strip_prefix("max=")
            && let Ok(n) = v.trim_end_matches(';').parse::<usize>() { max = Some(n); }
        else if let Some(v) = kv.strip_prefix("order=") {
            order = Some(v.trim_end_matches(';').to_lowercase());
        }
    }

    Some(Pagination {
        range_start,
        range_end,
        field,
        max: max.unwrap_or(200),
        order,
        next_range: None,
    })
}

// Parse Next-Range header. Returns (raw, order, max)
fn parse_next_range(headers: &HeaderMap) -> Option<(String, Option<String>, Option<usize>)> {
    let name = HeaderName::from_static("next-range");
    let raw = headers.get(name).and_then(|v| v.to_str().ok())?.to_string();
    // Best-effort parse order/max for UI hints
    let mut order: Option<String> = None;
    let mut max: Option<usize> = None;
    for kv in raw.split(';').map(str::trim) {
        if let Some(v) = kv.strip_prefix("order=") {
            order = Some(v.trim_end_matches(';').to_lowercase());
        } else if let Some(v) = kv.strip_prefix("max=") {
            if let Ok(n) = v.trim_end_matches(';').parse::<usize>() { max = Some(n); }
        }
    }
    Some((raw, order, max))
}
