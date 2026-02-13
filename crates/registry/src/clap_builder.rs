use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::{CommandFlag, CommandRegistry, CommandSpec};

/// Builds a complete Clap command tree from the registry's command
/// specifications.
///
/// This function transforms the registry's command definitions into a
/// hierarchical Clap command structure. Commands are grouped by their resource
/// prefix (before ':'), and each command includes its flags, positional
/// arguments, and help text.
///
/// The generated command tree includes global flags for JSON output,
/// and verbose logging that apply to all commands.
///
/// # Arguments
///
/// * `registry` - The registry containing all command specifications
///
/// # Returns
///
/// A configured ClapCommand that can be used for argument parsing and help
/// generation.
///
/// # Examples
///
/// ```rust
/// use std::sync::{Arc, Mutex};
/// use oatty_registry::{CommandRegistry, build_clap};
///
/// let registry = CommandRegistry::from_config().unwrap();
/// let registry = Arc::new(Mutex::new(registry));
/// let _clap_command = build_clap(Arc::clone(&registry));
/// ```
pub fn build_clap(registry: Arc<Mutex<CommandRegistry>>) -> ClapCommand {
    let mut root = create_root_command("oatty");
    let Some(lock) = registry.lock().ok() else {
        return root;
    };
    let commands = &lock.commands;
    let canonical_identifier_help = build_canonical_identifier_help(commands);
    let groups = group_commands_by_resource(commands);
    root = root.after_help(canonical_identifier_help);

    for (group, cmds) in groups {
        let group_command = build_group_command(&group, cmds);
        root = root.subcommand(group_command);
    }

    root = root.subcommand(build_workflow_root_command());
    root.subcommand(build_import_root_command())
}

/// Creates the root command with global flags.
///
/// This function creates the main command with global flags that apply
/// to all subcommands. The global flags include:
///
/// - `--version`, `-V` - Displays the version information
/// - `--help`, `-h` - Displays help information
/// - `--json` - Enables JSON output format
/// - `--verbose` - Enables verbose logging output
///
/// # Returns
///
/// A ClapCommand configured as the root command with global flags.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::create_root_command;
///
/// let root = create_root_command("example");
/// assert_eq!(root.get_name(), "example");
/// ```
fn create_root_command(product_name: &str) -> ClapCommand {
    // Clap command names require a 'static lifetime, so we leak the computed name once.
    let static_product_name: &'static str = Box::leak(product_name.to_string().into_boxed_str());
    ClapCommand::new(static_product_name)
        .version(env!("CARGO_PKG_VERSION"))
        .about(format!("{} CLI (powered by Oatty)", product_name))
        .arg(
            Arg::new("json")
                .long("json")
                .help("JSON output")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .help("Verbose logging")
                .global(true)
                .action(ArgAction::SetTrue),
        )
}

/// Builds a canonical identifier section for root CLI help output.
///
/// This produces a deterministic, newline-delimited list of identifiers in
/// `<group> <name>` format using each command specification's canonical ID.
fn build_canonical_identifier_help(commands: &[CommandSpec]) -> String {
    let mut canonical_identifiers = BTreeSet::new();
    for command in commands {
        canonical_identifiers.insert(command.canonical_id());
    }

    let mut help_lines = Vec::with_capacity(canonical_identifiers.len() + 2);
    help_lines.push("Command Spec Canonical IDs:".to_string());
    help_lines.push(String::new());
    for canonical_identifier in canonical_identifiers {
        help_lines.push(format!("  {canonical_identifier}"));
    }

    help_lines.join("\n")
}

/// Groups commands by their command specification group.
///
/// This function analyzes all commands in the registry and groups them by the
/// `group` field from each `CommandSpec`.
///
/// # Arguments
///
/// * `commands` - The command specifications to group
///
/// # Returns
///
/// A BTreeMap where keys are resource group names and values are vectors of
/// command specifications belonging to that group.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::{Registry, clap_builder::group_commands_by_resource};
///
/// let registry = Registry::from_embedded_schema()?;
/// let groups = group_commands_by_resource(&registry);
///
/// // Commands like "apps:list" will be in groups["apps"]
/// // Commands like "dynos:restart" will be in groups["dynos"]
/// ```
fn group_commands_by_resource(commands: &[CommandSpec]) -> BTreeMap<String, Vec<&CommandSpec>> {
    let mut groups: BTreeMap<String, Vec<&CommandSpec>> = BTreeMap::new();
    for command in commands {
        groups.entry(command.group.clone()).or_default().push(command);
    }

    groups
}

/// Builds a group command containing all subcommands for a specific resource.
///
/// This function creates a Clap command group (e.g., "apps", "dynos") that
/// contains all the subcommands for that resource. The group command itself
/// doesn't have any functionality but serves as a container for related
/// subcommands.
///
/// # Arguments
///
/// * `group` - The resource group name (e.g., "apps", "dynos")
/// * `cmds` - Vector of command specifications belonging to this group
///
/// # Returns
///
/// A ClapCommand configured as a group command with all its subcommands.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::build_group_command;
///
/// let group_cmd = build_group_command("apps", vec![&cmd1, &cmd2]);
/// assert_eq!(group_cmd.get_name(), "apps");
/// ```
fn build_group_command(group: &str, cmds: Vec<&CommandSpec>) -> ClapCommand {
    let static_command_name: &'static str = Box::leak(group.to_string().into_boxed_str());
    let mut group_cmd = ClapCommand::new(static_command_name);

    for cmd in cmds {
        let subcommand = build_subcommand(cmd);
        group_cmd = group_cmd.subcommand(subcommand);
    }

    group_cmd
}

fn build_workflow_root_command() -> ClapCommand {
    let list_cmd = ClapCommand::new("list").about("List workflows embedded in the registry");

    let preview_cmd = ClapCommand::new("preview")
        .about("Preview a workflow definition")
        .arg(
            Arg::new("id")
                .long("id")
                .short('i')
                .value_name("WORKFLOW_ID")
                .required_unless_present("file")
                .help("Identifier for a workflow bundled in the registry"),
        )
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("PATH")
                .help("Preview a workflow definition from a file")
                .conflicts_with("id"),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .value_parser(["yaml", "json"])
                .default_value("yaml")
                .help("Output format for the preview"),
        );

    let run_cmd = ClapCommand::new("run")
        .about("Execute a workflow")
        .arg(
            Arg::new("id")
                .long("id")
                .short('i')
                .value_name("WORKFLOW_ID")
                .required_unless_present("file")
                .help("Identifier for a workflow bundled in the registry"),
        )
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("PATH")
                .help("Execute a workflow definition from a file")
                .conflicts_with("id"),
        )
        .arg(
            Arg::new("input")
                .long("input")
                .value_name("KEY=VALUE")
                .help("Override a workflow input (repeatable)")
                .action(ArgAction::Append),
        );

    ClapCommand::new("workflow")
        .about("Workflow utilities")
        .subcommand(list_cmd)
        .subcommand(preview_cmd)
        .subcommand(run_cmd)
}

fn build_import_root_command() -> ClapCommand {
    ClapCommand::new("import")
        .about("Import a workflow or OpenAPI catalog from a file path or URL")
        .arg(
            Arg::new("source")
                .value_name("SOURCE")
                .required(true)
                .help("Local file path or HTTP(S) URL to import"),
        )
        .arg(
            Arg::new("kind")
                .long("kind")
                .value_name("KIND")
                .value_parser(["catalog", "workflow"])
                .help("Optional import kind override. Auto-detected when omitted."),
        )
        .arg(
            Arg::new("source-type")
                .long("source-type")
                .value_name("SOURCE_TYPE")
                .value_parser(["path", "url"])
                .help("Optional source location type hint. Auto-detected when omitted."),
        )
        .arg(
            Arg::new("catalog-title")
                .long("catalog-title")
                .value_name("TITLE")
                .help("Override catalog title during OpenAPI import"),
        )
        .arg(
            Arg::new("vendor")
                .long("vendor")
                .value_name("VENDOR")
                .help("Override vendor/prefix used when generating catalog commands"),
        )
        .arg(
            Arg::new("base-url")
                .long("base-url")
                .value_name("URL")
                .help("Override base URL for imported OpenAPI catalog"),
        )
        .arg(
            Arg::new("overwrite")
                .long("overwrite")
                .help("Replace existing catalog/workflow with the same identifier")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("disabled")
                .long("disabled")
                .help("Import catalog as disabled (catalog imports only)")
                .action(ArgAction::SetTrue),
        )
}

/// Builds a single subcommand with its arguments and flags.
///
/// This function creates a complete subcommand using the command
/// specification's canonical command name (e.g., "apps:list").
///
/// # Arguments
///
/// * `cmd` - The command specification containing all metadata for the
///   subcommand
///
/// # Returns
///
/// A fully configured ClapCommand representing the subcommand.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::build_subcommand;
///
/// let subcmd = build_subcommand(&command_spec);
/// // For a command named "apps:list", this creates an "apps:list" subcommand
/// ```
fn build_subcommand(cmd: &CommandSpec) -> ClapCommand {
    let static_sub_name: &'static str = Box::leak(cmd.name.clone().into_boxed_str());
    let mut subcommand = ClapCommand::new(static_sub_name).about(&cmd.summary);

    // Add positional arguments
    subcommand = add_positional_arguments(subcommand, cmd);

    // Add flags
    subcommand = add_flags(subcommand, cmd);

    subcommand
}

/// Adds positional arguments to a subcommand.
///
/// This function processes all positional arguments defined in the command
/// specification and adds them to the Clap subcommand. Positional arguments are
/// required and are assigned sequential indices starting from 1.
///
/// # Arguments
///
/// * `subcommand` - The ClapCommand to add positional arguments to
/// * `cmd` - The command specification containing positional argument
///   definitions
///
/// # Returns
///
/// The modified ClapCommand with positional arguments added.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::add_positional_arguments;
///
/// let subcommand = add_positional_arguments(subcommand, &cmd_spec);
/// // If cmd_spec has positional_args = ["app", "dyno"], this adds
/// // two required positional arguments with indices 1 and 2
/// ```
fn add_positional_arguments(mut subcommand: ClapCommand, cmd: &CommandSpec) -> ClapCommand {
    for (i, pa) in cmd.positional_args.iter().enumerate() {
        let name_static: &'static str = Box::leak(pa.name.clone().into_boxed_str());
        let mut arg = Arg::new(name_static).required(true).index(i + 1);
        if let Some(help) = &pa.help {
            let help_static: &'static str = Box::leak(help.clone().into_boxed_str());
            arg = arg.help(help_static);
        }
        subcommand = subcommand.arg(arg);
    }
    subcommand
}

/// Adds flags to a subcommand.
///
/// This function processes all flags defined in the command specification and
/// adds them to the Clap subcommand. Each flag is converted to a Clap argument
/// with appropriate properties based on its type and configuration.
///
/// # Arguments
///
/// * `subcommand` - The ClapCommand to add flags to
/// * `cmd` - The command specification containing flag definitions
///
/// # Returns
///
/// The modified ClapCommand with flags added.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::add_flags;
///
/// let subcommand = add_flags(subcommand, &cmd_spec);
/// // Adds all flags from cmd_spec.flags as long-form arguments
/// ```
fn add_flags(mut subcommand: ClapCommand, cmd: &CommandSpec) -> ClapCommand {
    for flag in &cmd.flags {
        let arg = build_flag_argument(flag);
        subcommand = subcommand.arg(arg);
    }
    subcommand
}

/// Builds a single flag argument with all its properties.
///
/// This function creates a complete Clap argument from a CommandFlag
/// specification. It handles all the different flag types (boolean, string,
/// enum) and sets up appropriate actions, validators, default values, and help
/// text.
///
/// # Arguments
///
/// * `flag` - The command flag specification containing all flag metadata
///
/// # Returns
///
/// A fully configured Clap Arg representing the flag.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::build_flag_argument;
///
/// let arg = build_flag_argument(&flag_spec);
/// // Creates a Clap argument with appropriate type, validation, and help text
/// ```
fn build_flag_argument(flag: &CommandFlag) -> Arg {
    let name: &'static str = Box::leak(flag.name.clone().into_boxed_str());
    let mut arg = Arg::new(name).long(name).required(flag.required);

    // Set action based on type
    arg = if flag.r#type == "boolean" {
        arg.action(ArgAction::SetTrue)
    } else {
        arg.action(ArgAction::Set)
    };

    // Add enum value parser if applicable
    if !flag.enum_values.is_empty() {
        arg = add_enum_values(arg, flag);
    }

    // Add default value for non-boolean flags
    if flag.r#type != "boolean" {
        arg = add_default_value(arg, flag);
    }

    // Add help text
    let help_text = generate_help_text(flag);
    arg.help(help_text)
}

/// Adds enum value validation to a flag argument.
///
/// This function adds value validation to a flag argument when the flag has
/// predefined enum values. It creates a PossibleValuesParser that restricts
/// the flag to only accept the specified enum values.
///
/// Note: This function leaks memory by converting enum values to static strings
/// to satisfy Clap's lifetime requirements. This is acceptable since the
/// command tree is typically built once during program startup.
///
/// # Arguments
///
/// * `arg` - The Clap argument to add enum validation to
/// * `flag` - The command flag containing enum value definitions
///
/// # Returns
///
/// The modified Clap argument with enum value validation added.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::add_enum_values;
///
/// let arg = add_enum_values(arg, &flag_spec);
/// // If flag_spec.enum_values = ["dev", "staging", "prod"],
/// // this restricts the argument to only accept these values
/// ```
fn add_enum_values(arg: Arg, flag: &CommandFlag) -> Arg {
    // Leak enum strings to satisfy 'static lifetime required by Clap builders
    let values: Vec<&'static str> = flag
        .enum_values
        .iter()
        .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
        .collect();
    arg.value_parser(clap::builder::PossibleValuesParser::new(values))
}

/// Adds default value to a flag argument.
///
/// This function adds a default value to a flag argument when one is specified
/// in the command flag definition. Default values are only added to non-boolean
/// flags since boolean flags use SetTrue/SetFalse actions.
///
/// Note: This function leaks memory by converting the default value to a static
/// string to satisfy Clap's lifetime requirements.
///
/// # Arguments
///
/// * `arg` - The Clap argument to add a default value to
/// * `flag` - The command flag containing the default value definition
///
/// # Returns
///
/// The modified Clap argument with default value added.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::add_default_value;
///
/// let arg = add_default_value(arg, &flag_spec);
/// // If flag_spec.default_value = Some("production"),
/// // this sets the default value for the argument
/// ```
fn add_default_value(mut arg: Arg, flag: &CommandFlag) -> Arg {
    if let Some(def) = &flag.default_value {
        let dv: &'static str = Box::leak(def.clone().into_boxed_str());
        arg = arg.default_value(dv);
    }
    arg
}

/// Generates help text for a flag.
///
/// This function creates appropriate help text for a flag argument. If the flag
/// has a custom description, it uses that. Otherwise, it generates a generic
/// help text based on the flag's type.
///
/// # Arguments
///
/// * `flag` - The command flag to generate help text for
///
/// # Returns
///
/// A string containing the help text for the flag.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_registry::clap_builder::generate_help_text;
///
/// let help = generate_help_text(&flag_spec);
/// // Returns either the custom description or "type: string"
/// ```
fn generate_help_text(flag: &CommandFlag) -> String {
    if let Some(desc) = &flag.description {
        desc.clone()
    } else {
        format!("type: {}", flag.r#type)
    }
}

#[cfg(test)]
mod tests {
    use oatty_types::{CommandExecution, command::HttpCommandSpec};

    use super::{build_canonical_identifier_help, build_import_root_command};
    use crate::CommandSpec;

    #[test]
    fn canonical_identifier_help_lists_sorted_unique_identifiers() {
        let command_specs = vec![
            CommandSpec {
                group: "apps".to_string(),
                name: "apps:list".to_string(),
                summary: String::new(),
                positional_args: Vec::new(),
                flags: Vec::new(),
                catalog_identifier: 0,
                execution: CommandExecution::Http(HttpCommandSpec::new("GET", "/apps", None)),
            },
            CommandSpec {
                group: "apps".to_string(),
                name: "apps:list".to_string(),
                summary: String::new(),
                positional_args: Vec::new(),
                flags: Vec::new(),
                catalog_identifier: 0,
                execution: CommandExecution::Http(HttpCommandSpec::new("GET", "/apps", None)),
            },
            CommandSpec {
                group: "apps".to_string(),
                name: "apps:create".to_string(),
                summary: String::new(),
                positional_args: Vec::new(),
                flags: Vec::new(),
                catalog_identifier: 0,
                execution: CommandExecution::Http(HttpCommandSpec::new("POST", "/apps", None)),
            },
        ];

        let output = build_canonical_identifier_help(&command_specs);
        let expected = "Command Spec Canonical IDs:\n\n  apps apps:create\n  apps apps:list";
        assert_eq!(output, expected);
    }

    #[test]
    fn import_command_supports_expected_arguments() {
        let command = build_import_root_command();
        let argument_ids: Vec<_> = command.get_arguments().map(|argument| argument.get_id().as_str()).collect();

        assert!(argument_ids.contains(&"source"));
        assert!(argument_ids.contains(&"kind"));
        assert!(argument_ids.contains(&"source-type"));
        assert!(argument_ids.contains(&"catalog-title"));
        assert!(argument_ids.contains(&"vendor"));
        assert!(argument_ids.contains(&"base-url"));
        assert!(argument_ids.contains(&"overwrite"));
        assert!(argument_ids.contains(&"disabled"));
    }
}
