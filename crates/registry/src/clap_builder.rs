use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::{CommandFlag, CommandSpec, Registry};

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
/// use heroku_registry::{Registry, build_clap};
///
/// let registry = Registry::from_embedded_schema().unwrap();
/// let _clap_command = build_clap(&registry);
/// ```
pub fn build_clap(registry: Arc<Mutex<Registry>>) -> ClapCommand {
    let mut root = create_root_command();
    let Some(lock) = registry.lock().ok() else {
        return root;
    };
    let commands = &lock.commands;
    let groups = group_commands_by_resource(commands);

    for (group, cmds) in groups {
        let group_command = build_group_command(&group, cmds);
        root = root.subcommand(group_command);
    }

    root
}

/// Creates the root command with global flags.
///
/// This function creates the main "heroku" command with global flags that apply
/// to all subcommands. The global flags include:
///
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
/// use heroku_registry::clap_builder::create_root_command;
///
/// let root = create_root_command();
/// assert_eq!(root.get_name(), "heroku");
/// ```
fn create_root_command() -> ClapCommand {
    ClapCommand::new("heroku")
        .about("Heroku CLI (experimental)")
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

/// Groups commands by their resource prefix (before ':').
///
/// This function analyzes all commands in the registry and groups them by their
/// resource type. For example, commands like "apps:list", "apps:create", and
/// "apps:destroy" would all be grouped under "apps".
///
/// Commands without a colon separator are grouped under "misc".
///
/// # Arguments
///
/// * `registry` - The registry containing all command specifications
///
/// # Returns
///
/// A BTreeMap where keys are resource group names and values are vectors of
/// command specifications belonging to that group.
///
/// # Examples
///
/// ```rust,ignore
/// use heroku_registry::{Registry, clap_builder::group_commands_by_resource};
///
/// let registry = Registry::from_embedded_schema()?;
/// let groups = group_commands_by_resource(&registry);
///
/// // Commands like "apps:list" will be in groups["apps"]
/// // Commands like "dynos:restart" will be in groups["dynos"]
/// ```
fn group_commands_by_resource(commands: &[CommandSpec]) -> BTreeMap<String, Vec<&CommandSpec>> {
    let mut groups: BTreeMap<String, Vec<&CommandSpec>> = BTreeMap::new();
    for cmd in commands {
        let mut parts = cmd.name.splitn(2, ':');
        let group = parts.next().unwrap_or("misc").to_string();
        groups.entry(group).or_default().push(cmd);
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
/// use heroku_registry::clap_builder::build_group_command;
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

/// Builds a single subcommand with its arguments and flags.
///
/// This function creates a complete subcommand (e.g., "list", "create") with
/// all its associated arguments, flags, and help text. The subcommand name is
/// extracted from the command specification by taking the part after the colon.
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
/// use heroku_registry::clap_builder::build_subcommand;
///
/// let subcmd = build_subcommand(&command_spec);
/// // For a command named "apps:list", this creates a "list" subcommand
/// ```
fn build_subcommand(cmd: &CommandSpec) -> ClapCommand {
    let subname = cmd.name.split_once(':').map(|x| x.1).unwrap_or("run").to_string();
    let static_sub_name: &'static str = Box::leak(subname.into_boxed_str());
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
/// use heroku_registry::clap_builder::add_positional_arguments;
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
/// use heroku_registry::clap_builder::add_flags;
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
/// use heroku_registry::clap_builder::build_flag_argument;
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
/// use heroku_registry::clap_builder::add_enum_values;
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
/// use heroku_registry::clap_builder::add_default_value;
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
/// use heroku_registry::clap_builder::generate_help_text;
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
