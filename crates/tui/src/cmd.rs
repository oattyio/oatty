use std::thread::spawn;

use heroku_registry::CommandSpec;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::app::{self, Effect};
use heroku_types::ExecOutcome;

/// Side-effect commands executed outside of pure state updates.
#[derive(Debug)]
pub enum Cmd {
    /// Set clipboard text
    ClipboardSet(String),
    ExecuteHttp(CommandSpec, String, serde_json::Map<String, Value>),
}

/// Translate App effects to concrete commands to run.
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

/// Execute commands and record user-visible feedback in `app.logs`.
pub fn run_cmds(app: &mut app::App, commands: Vec<Cmd>) {
    for command in commands {
        match command {
            Cmd::ClipboardSet(text) => {
                // Perform clipboard write and log outcome
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
                    Ok(()) => app.logs.entries.push(format!("Copied: {}", text)),
                    Err(e) => app.logs.entries.push(format!("Clipboard error: {}", e)),
                }
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

fn execute_http(
    app: &mut app::App,
    spec: CommandSpec,
    path: String,
    body: serde_json::Map<String, Value>,
) {
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
    let resp = builder.send().await.map_err(|e| format!("Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set", e))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.as_u16() == 401 {
        return Err("Unauthorized (401). Hint: set HEROKU_API_KEY=... or configure ~/.netrc with machine api.heroku.com".into());
    }
    if status.as_u16() == 403 {
        return Err(
            "Forbidden (403). Hint: check team/app access, permissions, and role membership".into(),
        );
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
