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
use heroku_types::{ExecOutcome, Pagination};
use reqwest::header::{CONTENT_RANGE, HeaderMap, HeaderName};
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

    if let (Some(spec), Some(path), Some(mut body)) =
        (app.last_spec.clone(), app.last_path.clone(), app.last_body.clone())
    {
        // Inject raw next-range override for Range header
        body.insert("next-range".into(), serde_json::Value::String(next_raw.clone()));

        // Append to history for Prev/First navigation
        app.pagination_history.push(Some(next_raw));

        commands.push(Cmd::ExecuteHttp(Box::new(spec), path, body));
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

    if let (Some(spec), Some(path), Some(mut body)) =
        (app.last_spec.clone(), app.last_path.clone(), app.last_body.clone())
    {
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

        commands.push(Cmd::ExecuteHttp(Box::new(spec), path, body));
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

    if let (Some(spec), Some(path), Some(mut body)) =
        (app.last_spec.clone(), app.last_path.clone(), app.last_body.clone())
    {
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

        commands.push(Cmd::ExecuteHttp(Box::new(spec), path, body));
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
            Cmd::ExecuteHttp(spec, path, body) => execute_http(app, *spec, path, body),
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
                    pagination: None,
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
/// # Arguments
/// * `spec` - The command specification containing HTTP method and path
/// * `path` - The API endpoint path
/// * `body` - The request body as a JSON map
///
/// # Returns
/// A result containing either the execution outcome or an error message
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

    let method = parse_http_method(&spec.method)?;
    let mut builder = client.request(method, &path);

    // Build and apply Range header
    builder = apply_range_headers(builder, &body);

    // Filter out special range-only fields from JSON body
    let filtered_body = filter_range_fields(body);
    if !filtered_body.is_empty() {
        builder = builder.json(&serde_json::Value::Object(filtered_body));
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

    // Handle Next-Range header for 206 responses
    if status.as_u16() == 206 {
        handle_next_range_header(&mut pagination, &headers);
    }

    let text = resp.text().await.unwrap_or_default();

    // Handle common error status codes
    if let Some(error_msg) = handle_error_status(status.as_u16()) {
        return Err(error_msg);
    }

    let log = format!("{}\n{}", status, text);
    let (result_json, open_table) = parse_response_json(&text);

    Ok(heroku_types::ExecOutcome {
        log,
        result_json,
        open_table,
        pagination,
    })
}

/// Parse HTTP method string into reqwest Method enum.
///
/// # Arguments
/// * `method_str` - The HTTP method as a string (e.g., "GET", "POST")
///
/// # Returns
/// A result containing the parsed method or an error message
fn parse_http_method(method_str: &str) -> Result<reqwest::Method, String> {
    match method_str.to_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        "DELETE" => Ok(reqwest::Method::DELETE),
        "PATCH" => Ok(reqwest::Method::PATCH),
        other => Err(format!("unsupported method: {}", other)),
    }
}

/// Apply Range headers to the request builder based on body parameters.
///
/// Handles both raw next-range overrides and composed range headers with
/// optional max/order parameters.
///
/// # Arguments
/// * `builder` - The request builder to modify
/// * `body` - The request body containing range parameters
///
/// # Returns
/// The modified request builder with appropriate headers
fn apply_range_headers(
    builder: reqwest::RequestBuilder,
    body: &serde_json::Map<String, Value>,
) -> reqwest::RequestBuilder {
    // Raw Next-Range override takes precedence
    if let Some(next_raw) = body.get("next-range").and_then(|v| v.as_str()) {
        return builder.header("Range", next_raw);
    }

    // Compose range header from individual components
    let field = body
        .get("range-field")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let start = body.get("range-start").and_then(|v| v.as_str()).unwrap_or("").trim();

    let end = body.get("range-end").and_then(|v| v.as_str()).unwrap_or("").trim();

    let order = body
        .get("order")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let max = body
        .get("max")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<usize>().ok());

    if let Some(field) = field {
        let range_seg = format!("{}..{}", start, end);
        let mut range_header = format!("{} {}", field, range_seg);

        // Append optional max/order parameters
        if let Some(m) = max {
            range_header.push_str(&format!("; max={}", m));
        }
        if let Some(ord) = order {
            range_header.push_str(&format!(", order={};", ord));
        }

        builder.header("Range", range_header)
    } else {
        builder
    }
}

/// Filter out special range-only fields from the request body.
///
/// These fields are used for header construction and should not be sent
/// in the JSON body of the request.
///
/// # Arguments
/// * `body` - The original request body
///
/// # Returns
/// A new body map with range fields removed
fn filter_range_fields(body: serde_json::Map<String, Value>) -> serde_json::Map<String, Value> {
    let mut filtered = body;

    for key in ["range-field", "range-start", "range-end", "order", "max", "next-range"] {
        let _ = filtered.remove(key);
    }

    filtered
}

/// Handle Next-Range header for 206 Partial Content responses.
///
/// Updates the pagination struct with the next range value if present
/// in the response headers.
///
/// # Arguments
/// * `pagination` - The pagination struct to update
/// * `headers` - The response headers
fn handle_next_range_header(pagination: &mut Option<Pagination>, headers: &HeaderMap) {
    let next_range_header = HeaderName::from_static("next-range");

    if let Some(p) = pagination.as_mut() {
        if let Some(value) = headers.get(next_range_header) {
            p.next_range = value.to_str().ok().map(|s| s.to_string());
        }
    }
}

/// Handle common HTTP error status codes with user-friendly messages.
///
/// # Arguments
/// * `status_code` - The HTTP status code
///
/// # Returns
/// Some error message if the status code indicates an error, None otherwise
fn handle_error_status(status_code: u16) -> Option<String> {
    match status_code {
        401 => Some(
            "Unauthorized (401). Hint: set HEROKU_API_KEY=... or configure ~/.netrc with machine api.heroku.com".into(),
        ),
        403 => Some("Forbidden (403). Hint: check team/app access, permissions, and role membership".into()),
        _ => None,
    }
}

/// Parse response text as JSON and determine if table should be opened.
///
/// # Arguments
/// * `text` - The response text to parse
///
/// # Returns
/// A tuple of (parsed_json, should_open_table)
fn parse_response_json(text: &str) -> (Option<serde_json::Value>, bool) {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(json) => (Some(json), true),
        Err(_) => (None, false),
    }
}

/// Parse Content-Range header into Pagination struct.
///
/// Extracts field name, range boundaries, max count, and order from
/// the Content-Range header format.
///
/// # Arguments
/// * `headers` - The response headers containing Content-Range
///
/// # Returns
/// Some Pagination struct if parsing succeeds, None on failure
///
/// # Example
/// Header: "name app7a..app9x; max=200; order=desc;"
/// Result: Pagination with field="name", range_start="app7a", range_end="app9x", max=200, order="desc"
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
        if let Some(v) = kv.strip_prefix("max=") {
            if let Ok(n) = v.trim_end_matches(';').parse::<usize>() {
                max = Some(n);
            }
        } else if let Some(v) = kv.strip_prefix("order=") {
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

/// Fetch a JSON array from the Heroku API at the given path.
///
/// Returns Ok(Vec<Value>) when the response body parses to a JSON array.
/// On error or non-array response, returns Err with a user-friendly message.
///
/// # Arguments
/// * `path` - The API endpoint path to fetch from
///
/// # Returns
/// A result containing either the parsed JSON array or an error message
///
/// # Example
/// ```rust,ignore
/// use your_crate::fetch_json_array;
///
/// # async fn demo() {
/// let result = fetch_json_array("/apps").await;
/// match result {
///     Ok(array) => println!("Found {} apps", array.len()),
///     Err(e) => eprintln!("Failed to fetch apps: {}", e),
/// }
/// # }
/// ```
pub(crate) async fn fetch_json_array(path: &str) -> Result<Vec<Value>, String> {
    let client = heroku_api::HerokuClient::new_from_env().map_err(|e| {
        format!(
            "Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc",
            e
        )
    })?;

    let resp = client.request(reqwest::Method::GET, path).send().await.map_err(|e| {
        format!(
            "Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set",
            e
        )
    })?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_else(|_| String::from("<no body>"));

    if !status.is_success() {
        return Err(format!("{}\n{}", status, text));
    }

    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(arr)) => Ok(arr),
        Ok(_) => Err("Response is not a JSON array".into()),
        Err(e) => Err(format!("Invalid JSON: {}", e)),
    }
}
