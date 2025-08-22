use anyhow::{anyhow, Context, Result};
use clap::{Arg, ArgAction, Command as ClapCommand};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandFlag {
    pub name: String,
    pub required: bool,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub enum_values: Vec<String>,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String, // e.g., "apps:list"
    pub summary: String,
    #[serde(default)]
    pub positional_args: Vec<String>,
    #[serde(default)]
    pub positional_help: HashMap<String, String>,
    #[serde(default)]
    pub flags: Vec<CommandFlag>,
    pub method: String, // GET/POST/DELETE/...
    pub path: String,   // e.g., "/apps" or "/apps/{app}"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    pub commands: Vec<CommandSpec>,
}

impl Registry {
    pub fn from_embedded_schema() -> Result<Self> {
        let manifest = include_str!(concat!(env!("OUT_DIR"), "/heroku-manifest.json"));
        let mut reg: Registry = serde_json::from_str(manifest).context("parse embedded manifest")?;
        if feature_workflows() {
            reg.add_workflow_commands();
        }
        Ok(reg)
    }

    pub fn build_clap(&self) -> ClapCommand {
        let mut root = ClapCommand::new("heroku")
            .about("Heroku CLI (experimental)")
            .arg(
                Arg::new("json")
                    .long("json")
                    .help("JSON output")
                    .global(true)
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("dry-run")
                    .long("dry-run")
                    .help("Do not execute, print requests")
                    .global(true)
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("verbose")
                    .long("verbose")
                    .help("Verbose logging")
                    .global(true)
                    .action(ArgAction::SetTrue),
            );

        // Group commands by resource prefix (before ':')
        let mut groups: BTreeMap<String, Vec<&CommandSpec>> = BTreeMap::new();
        for cmd in &self.commands {
            let mut parts = cmd.name.splitn(2, ':');
            let group = parts.next().unwrap_or("misc").to_string();
            groups.entry(group).or_default().push(cmd);
        }

        // Clap requires us to leak the command names which is fine
        // since we're only doing this once throughout the life
        // of the program.
        for (group, cmds) in groups {
            let static_command_name: &'static str = Box::leak(group.into_boxed_str());
            let mut g = ClapCommand::new(static_command_name);
            for cmd in cmds {
                let subname = cmd.name.splitn(2, ':').nth(1).unwrap_or("run").to_string();
                let static_sub_name: &'static str = Box::leak(subname.into_boxed_str());
                let mut sc = ClapCommand::new(static_sub_name).about(&cmd.summary);
                // positional args
                for (i, pa) in cmd.positional_args.iter().enumerate() {
                    let arg: &'static str = Box::leak(pa.clone().into_boxed_str());
                    sc = sc.arg(Arg::new(arg).required(true).index((i + 1) as usize));
                }
                // flags
                for f in &cmd.flags {
                    let name: &'static str = Box::leak(f.name.clone().into_boxed_str());
                    let mut a = Arg::new(name).long(name).required(f.required);
                    a = if f.r#type == "boolean" {
                        a.action(ArgAction::SetTrue)
                    } else {
                        a.action(ArgAction::Set)
                    };
                    if !f.enum_values.is_empty() {
                        // Leak enum strings to satisfy 'static lifetime required by Clap builders
                        let values: Vec<&'static str> = f
                            .enum_values
                            .iter()
                            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
                            .collect();
                        a = a.value_parser(clap::builder::PossibleValuesParser::new(values));
                    }
                    if f.r#type != "boolean" {
                        if let Some(def) = &f.default_value {
                            let dv: &'static str = Box::leak(def.clone().into_boxed_str());
                            a = a.default_value(dv);
                        }
                    }
                    let help_text = if let Some(desc) = &f.description {
                        desc.clone()
                    } else {
                        format!("type: {}", f.r#type)
                    };
                    sc = sc.arg(a.help(help_text));
                }
                // Store method/path in about? We'll resolve at runtime using name.
                g = g.subcommand(sc);
            }
            root = root.subcommand(g);
        }

        root
    }

    pub fn find_by_group_and_cmd(&self, group: &str, cmd: &str) -> Result<&CommandSpec> {
        let key = format!("{}:{}", group, cmd);
        self.commands
            .iter()
            .find(|c| c.name == key)
            .ok_or_else(|| anyhow!("command not found: {}", key))
    }

    fn add_workflow_commands(&mut self) {
        // Synthetic commands for local workflows. These are not HTTP calls,
        // but exposing them via the registry makes them available to the TUI.
        let mut add = |name: &str, summary: &str, flags: Vec<CommandFlag>| {
            self.commands.push(CommandSpec {
                name: name.to_string(),
                summary: summary.to_string(),
                positional_args: vec![],
                positional_help: HashMap::new(),
                flags,
                // Method/path are unused for internal commands; keep placeholders.
                method: "INTERNAL".into(),
                path: "__internal__".into(),
            });
        };

        // Common flags
        let file_flag = |required: bool| CommandFlag {
            name: "file".into(),
            required,
            r#type: "string".into(),
            enum_values: vec![],
            default_value: None,
            description: Some("Path to workflow YAML/JSON".into()),
        };
        let name_flag = |required: bool| CommandFlag {
            name: "name".into(),
            required,
            r#type: "string".into(),
            enum_values: vec![],
            default_value: None,
            description: Some("Workflow name within the file".into()),
        };
        add(
            "workflow:list",
            "List workflows in workflows/ directory",
            vec![],
        );
        add(
            "workflow:preview",
            "Preview workflow plan",
            vec![file_flag(false), name_flag(false)],
        );
        add(
            "workflow:run",
            "Run workflow (use global --dry-run)",
            vec![file_flag(false), name_flag(false)],
        );
    }
}


fn feature_workflows() -> bool {
    std::env::var("FEATURE_WORKFLOWS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}
#[cfg(test)]
mod tests {
    use crate::Registry;
    use std::collections::HashSet;

    #[test]
    fn test_registry() -> Result<(), ()> {
        let registry = Registry::from_embedded_schema().unwrap();
        let cli = registry.build_clap();
        Ok(())
    }

    #[test]
    fn manifest_non_empty_and_unique_names() {
        let registry = Registry::from_embedded_schema().expect("load registry from manifest");
        assert!(!registry.commands.is_empty(), "registry commands should not be empty");
        let mut seen = HashSet::new();
        for c in &registry.commands {
            assert!(seen.insert(&c.name), "duplicate command name detected: {}", c.name);
        }
    }
}
