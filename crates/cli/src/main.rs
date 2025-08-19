use anyhow::{Context, Result};
use clap::ArgMatches;
use tracing::Level;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let registry = heroku_registry::Registry::from_embedded_schema()?;
    let cli = with_workflow_cli(registry.build_clap());
    let matches = cli.get_matches();

    // No subcommands => TUI
    if matches.subcommand_name().is_none() {
        return heroku_tui::run().map(|_| ());
    }

    // Special workflow subcommands behind FEATURE_WORKFLOWS=1
    if let Some(("workflow", sub)) = matches.subcommand() {
        return run_workflow_cmd(&registry, sub).await;
    }

    run_command(&registry, &matches).await
}

fn init_tracing() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_max_level(Level::INFO)
        .try_init();
}

async fn run_command(registry: &heroku_registry::Registry, matches: &ArgMatches) -> Result<()> {
    let (group, sub) = matches
        .subcommand()
        .context("expected a resource group subcommand")?;
    let sub_matches = sub;
    let (cmd_name, cmd_matches) = sub_matches
        .subcommand()
        .context("expected a command under the group")?;

    let spec = registry.find_by_group_and_cmd(group, cmd_name)?;
    let dry_run = matches.get_flag("dry-run");

    // Collect positional values
    use std::collections::HashMap;
    let mut pos_values: HashMap<String, String> = HashMap::new();
    for key in &spec.positional_args {
        if let Some(val) = cmd_matches.get_one::<String>(key) {
            pos_values.insert(key.clone(), val.to_string());
        }
    }

    // Collect flags as body fields
    let mut body = serde_json::Map::new();
    for f in &spec.flags {
        if f.r#type == "boolean" {
            if cmd_matches.get_flag(&f.name) {
                body.insert(f.name.clone(), serde_json::Value::Bool(true));
            }
        } else if let Some(val) = cmd_matches.get_one::<String>(&f.name) {
            body.insert(f.name.clone(), serde_json::Value::String(val.to_string()));
        }
    }
    let body_value = if body.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(body))
    };

    let client = heroku_api::HerokuClient::new_from_env()?;
    let method = match spec.method.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        other => anyhow::bail!("unsupported method: {}", other),
    };
    let path = resolve_path(&spec.path, &pos_values);
    let mut builder = client.request(method, &path);
    if let Some(ref b) = body_value {
        builder = builder.json(b);
    }

    if dry_run {
        // Build request to inspect
        let req = builder.build()?;
        let url = req.url().to_string();
        let method = req.method().to_string();
        // Basic headers set; redact secrets
        let mut headers_out = serde_json::Map::new();
        for (name, value) in req.headers().iter() {
            let val = value.to_str().unwrap_or("");
            let line = format!("{}: {}", name.as_str(), val);
            let redacted = heroku_util::redact_sensitive(&line);
            // Extract after ': '
            let out_val = redacted
                .splitn(2, ':')
                .nth(1)
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
            headers_out.insert(
                name.as_str().to_string(),
                serde_json::Value::String(out_val),
            );
        }
        let out = serde_json::json!({
            "method": method,
            "url": url,
            "headers": headers_out,
            "body": body_value
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    // Execute
    let resp = builder.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    println!("{}\n{}", status, text);
    Ok(())
}

fn resolve_path(template: &str, pos: &std::collections::HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in pos {
        let needle = format!("{{{}}}", k);
        out = out.replace(&needle, v);
    }
    out
}

fn feature_workflows() -> bool { std::env::var("FEATURE_WORKFLOWS").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false) }

fn with_workflow_cli(root: clap::Command) -> clap::Command {
    if !feature_workflows() { return root; }
    use clap::{Arg, ArgAction, Command};
    let wf = Command::new("workflow")
        .about("Manage workflows")
        .subcommand(Command::new("list").about("List workflows in workflows/ directory"))
        .subcommand(
            Command::new("preview")
                .about("Preview workflow plan")
                .arg(Arg::new("file").long("file").short('f').action(ArgAction::Set).help("Path to workflow YAML/JSON"))
                .arg(Arg::new("name").long("name").action(ArgAction::Set).help("Workflow name within file")),
        )
        .subcommand(
            Command::new("run")
                .about("Run workflow")
                .arg(Arg::new("file").long("file").short('f').action(ArgAction::Set))
                .arg(Arg::new("name").long("name").action(ArgAction::Set))
                .arg(Arg::new("dry-run").long("dry-run").action(ArgAction::SetTrue)),
        );
    root.subcommand(wf)
}

async fn run_workflow_cmd(registry: &heroku_registry::Registry, m: &ArgMatches) -> Result<()> {
    if !feature_workflows() { anyhow::bail!("workflows are disabled; set FEATURE_WORKFLOWS=1"); }
    match m.subcommand() {
        Some(("list", _)) => {
            let dir = std::path::Path::new("workflows");
            if dir.exists() {
                for entry in std::fs::read_dir(dir)? {
                    let e = entry?;
                    println!("{}", e.path().display());
                }
            } else {
                println!("No workflows directory found");
            }
        }
        Some(("preview", sub)) => {
            let file = sub.get_one::<String>("file").map(|s| s.as_str()).unwrap_or("workflows/create_app_and_db.yaml");
            let wf_file = heroku_engine::load_workflow_from_file(file)?;
            let name = sub.get_one::<String>("name").map(|s| s.as_str()).or_else(|| wf_file.workflows.keys().next().map(|s| s.as_str())).ok_or_else(|| anyhow::anyhow!("no workflow name provided and file has none"))?;
            let wf = wf_file.workflows.get(name).ok_or_else(|| anyhow::anyhow!("workflow '{}' not found in file", name))?;
            let plan = heroku_engine::dry_run_plan(wf, registry).await?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        Some(("run", sub)) => {
            let file = sub.get_one::<String>("file").map(|s| s.as_str()).unwrap_or("workflows/create_app_and_db.yaml");
            let wf_file = heroku_engine::load_workflow_from_file(file)?;
            let name = sub.get_one::<String>("name").map(|s| s.as_str()).or_else(|| wf_file.workflows.keys().next().map(|s| s.as_str())).ok_or_else(|| anyhow::anyhow!("no workflow name provided and file has none"))?;
            let wf = wf_file.workflows.get(name).ok_or_else(|| anyhow::anyhow!("workflow '{}' not found in file", name))?;
            let dry = sub.get_flag("dry-run");
            if dry { let plan = heroku_engine::dry_run_plan(wf, registry).await?; println!("{}", serde_json::to_string_pretty(&plan)?); }
            else { println!("Workflow run not implemented yet; use --dry-run"); }
        }
        _ => { println!("Available subcommands: list, preview, run"); }
    }
    Ok(())
}
