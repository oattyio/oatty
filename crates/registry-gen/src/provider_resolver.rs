//! Provider resolution (two-pass) for registry generation.
//!
//! This module verifies and assigns ValueProviders to flags and positional
//! arguments only after the full command list is known. It builds an index of
//! `<group>:<name>` → CommandSpec presence so provider candidates can be
//! verified with 100% confidence. Heuristics are still used to propose
//! candidates, but only verified providers are assigned.
//!
//! # Architecture
//!
//! The resolution process follows a two-pass strategy:
//! 1. **Pass 1**: Commands are constructed from the schema in `schema::derive_commands_from_schema`
//! 2. **Pass 2**: This module builds a command index and assigns verified providers
//!
//! # Key Concepts
//!
//! - **Command Index**: A mapping of `<group>:<name>` to command presence for verification
//! - **List Groups**: Groups that have a corresponding `list` command for value provision
//! - **Provider Bindings**: Mappings from consumer inputs to provider parameters
//! - **Placeholder Resolution**: Matching path placeholders to available consumer inputs

use std::collections::{HashMap, HashSet};

use heroku_types::{Bind, CommandFlag, CommandSpec, ValueProvider};

/// Represents the outcome of attempting to bind provider parameters to consumer inputs.
#[derive(Debug)]
enum BindingOutcome {
    /// Provider has no placeholders or required flags that need binding
    NoPlaceholders,
    /// All required bindings were successfully resolved
    Satisfied(Vec<Bind>),
    /// Some required bindings could not be satisfied with available inputs
    Unsatisfied,
}

/// Resolve ValueProviders with a two-pass strategy.
///
/// This function performs the second pass of provider resolution, building a command
/// index and assigning verified providers to flags and positional arguments.
///
/// # Process
///
/// 1. Build a command index mapping `<group>:<name>` to command presence
/// 2. Identify groups that have corresponding `list` commands
/// 3. Precompute provider metadata for efficient binding evaluation
/// 4. Apply verified providers to flags and positional arguments
///
/// # Arguments
///
/// * `commands` - Mutable slice of command specifications to process
pub fn resolve_and_infer_providers(commands: &mut [CommandSpec]) {
    let (command_index, _command_id_to_index) = build_command_index(commands);
    let list_groups = find_groups_with_list_commands(&command_index);

    let provider_metadata = precompute_provider_metadata(commands);

    for command_spec in commands.iter_mut() {
        apply_flag_providers(&mut command_spec.flags, &list_groups, &command_index);
        apply_positional_providers(
            command_spec,
            &command_index,
            &provider_metadata.placeholders,
            &provider_metadata.required_flags,
        );
    }
}

/// Metadata about providers used for efficient binding evaluation.
struct ProviderMetadata {
    /// Maps provider command IDs to their path placeholders
    placeholders: HashMap<String, Vec<String>>,
    /// Maps provider command IDs to their required flags
    required_flags: HashMap<String, Vec<String>>,
}

/// Build an index of `<group>:<name>` command identifiers.
///
/// Returns a tuple containing:
/// - A set of all command identifiers for fast lookup
/// - A map from command identifiers to their index in the commands slice
fn build_command_index(commands: &[CommandSpec]) -> (HashSet<String>, HashMap<String, usize>) {
    let mut command_identifiers = HashSet::new();
    let mut command_id_to_index = HashMap::new();

    for (index, command) in commands.iter().enumerate() {
        let command_id = format!("{}:{}", command.group, command.name);
        command_identifiers.insert(command_id.clone());
        command_id_to_index.insert(command_id, index);
    }

    (command_identifiers, command_id_to_index)
}

/// Precompute provider metadata for efficient binding evaluation.
///
/// This function extracts placeholders and required flags from all commands
/// to avoid repeated computation during provider resolution.
fn precompute_provider_metadata(commands: &[CommandSpec]) -> ProviderMetadata {
    let placeholders: HashMap<String, Vec<String>> = commands
        .iter()
        .map(|command| {
            let command_id = format!("{}:{}", command.group, command.name);
            let extracted_placeholders = extract_path_placeholders(&command.path);
            (command_id, extracted_placeholders)
        })
        .collect();

    let required_flags: HashMap<String, Vec<String>> = commands
        .iter()
        .map(|command| {
            let command_id = format!("{}:{}", command.group, command.name);
            let required_flag_names: Vec<String> = command
                .flags
                .iter()
                .filter(|flag| flag.required)
                .map(|flag| flag.name.clone())
                .collect();
            (command_id, required_flag_names)
        })
        .collect();

    ProviderMetadata {
        placeholders,
        required_flags,
    }
}

/// Find groups that have a corresponding `list` command defined.
///
/// This function identifies which groups can provide value lists for autocompletion
/// by looking for commands with the name "list" in each group.
fn find_groups_with_list_commands(command_index: &HashSet<String>) -> HashSet<String> {
    command_index
        .iter()
        .filter_map(|command_id| command_id.split_once(':'))
        .filter(|(_, command_name)| *command_name == "list")
        .map(|(group_name, _)| group_name.to_string())
        .collect()
}

/// Apply verified flag providers to command flags.
///
/// This function examines each flag and assigns a value provider if:
/// 1. The flag name maps to a known group via synonyms
/// 2. The group has a corresponding `list` command
/// 3. The `list` command exists in the command index
///
/// # Arguments
///
/// * `flags` - Mutable slice of command flags to process
/// * `list_groups` - Set of groups that have `list` commands
/// * `command_index` - Set of all available command identifiers
fn apply_flag_providers(flags: &mut [CommandFlag], list_groups: &HashSet<String>, command_index: &HashSet<String>) {
    let flag_to_group_synonyms = create_flag_to_group_synonyms();

    for flag in flags.iter_mut() {
        if let Some(group_name) = map_flag_name_to_group(&flag.name, &flag_to_group_synonyms) {
            let list_provider_id = format!("{}:{}", group_name, "list");

            if list_groups.contains(&group_name) && command_index.contains(&list_provider_id) {
                flag.provider = Some(ValueProvider::Command {
                    command_id: list_provider_id,
                    binds: vec![],
                });
            }
        }
    }
}

/// Create a mapping from flag names to their corresponding group names.
///
/// This mapping handles common singular-to-plural transformations and special cases
/// for Heroku CLI command groups.
fn create_flag_to_group_synonyms() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("app", "apps"),
        ("addon", "addons"),
        ("pipeline", "pipelines"),
        ("team", "teams"),
        ("space", "spaces"),
        ("dyno", "dynos"),
        ("release", "releases"),
        ("collaborator", "collaborators"),
        ("region", "regions"),
        ("stack", "stacks"),
    ])
}

/// Apply verified positional providers to command positional arguments.
///
/// This function walks through the command path, identifies placeholders, and assigns
/// value providers based on the previous concrete path segment. It attempts to compute
/// bindings from earlier consumer inputs when possible.
///
/// # Arguments
///
/// * `command_spec` - The command specification to process
/// * `list_groups` - Set of groups that have `list` commands
/// * `command_index` - Set of all available command identifiers
/// * `provider_placeholders` - Precomputed placeholder mappings
/// * `provider_required_flags` - Precomputed required flag mappings
fn apply_positional_providers(
    command_spec: &mut CommandSpec,
    command_index: &HashSet<String>,
    provider_placeholders: &HashMap<String, Vec<String>>,
    provider_required_flags: &HashMap<String, Vec<String>>,
) {
    let positional_name_to_index = build_positional_index(command_spec);
    let path_segments = parse_path_segments(&command_spec.path);

    let mut previous_concrete_segment: Option<String> = None;

    for segment in path_segments {
        match segment {
            PathSegment::Placeholder(placeholder_name) => {
                if let Some(previous_segment) = &previous_concrete_segment {
                    process_placeholder(
                        command_spec,
                        &placeholder_name,
                        previous_segment,
                        &positional_name_to_index,
                        command_index,
                        provider_placeholders,
                        provider_required_flags,
                    );
                }
            }
            PathSegment::Concrete(segment_name) => {
                if !is_version_segment(&segment_name) {
                    previous_concrete_segment = Some(segment_name);
                }
            }
        }
    }
}

/// Represents a parsed segment from a command path.
#[derive(Debug)]
enum PathSegment {
    /// A placeholder segment like `{app_id}`
    Placeholder(String),
    /// A concrete segment like `apps` or `config-vars`
    Concrete(String),
}

/// Build a mapping from positional argument names to their indices.
fn build_positional_index(command_spec: &CommandSpec) -> HashMap<String, usize> {
    command_spec
        .positional_args
        .iter()
        .enumerate()
        .map(|(index, positional)| (positional.name.clone(), index))
        .collect()
}

/// Parse a command path into segments, distinguishing between placeholders and concrete segments.
fn parse_path_segments(path: &str) -> Vec<PathSegment> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                let placeholder_name = segment.trim_start_matches('{').trim_end_matches('}').trim().to_string();
                PathSegment::Placeholder(placeholder_name)
            } else {
                PathSegment::Concrete(segment.to_string())
            }
        })
        .collect()
}

/// Find candidate providers for a given group, considering both scoped and unscoped options.
///
/// This function looks for list commands that could provide values for the target placeholder,
/// considering both simple group-based providers (e.g., "addons:list") and scoped providers
/// (e.g., "app:addons:list") that require additional parameters.
fn find_provider_candidates(
    normalized_group: &str,
    command_spec: &CommandSpec,
    _positional_name_to_index: &HashMap<String, usize>,
    command_index: &HashSet<String>,
) -> Vec<String> {
    let mut candidates = Vec::new();

    // First, try the simple unscoped provider
    let simple_provider = format!("{}:{}", normalized_group, "list");
    if command_index.contains(&simple_provider) {
        candidates.push(simple_provider);
    }

    // Then, look for scoped providers by examining earlier path segments
    let path_segments = parse_path_segments(&command_spec.path);
    let mut concrete_segments = Vec::new();

    for segment in path_segments {
        match segment {
            PathSegment::Concrete(segment_name) => {
                if !is_version_segment(&segment_name) {
                    concrete_segments.push(segment_name);
                }
            }
            PathSegment::Placeholder(_) => {
                // Continue collecting concrete segments
            }
        }
    }

    // Look for scoped providers by checking if earlier segments can be used as group names
    // for commands with name "<target_group>:list"
    for (i, segment) in concrete_segments.iter().enumerate() {
        if i < concrete_segments.len() - 1 {
            // Don't use the last segment (it's the target group)
            // Try using this segment as a group name with "<target_group>:list" as the command name
            let scoped_provider = format!("{}:{}:{}", segment, normalized_group, "list");
            if command_index.contains(&scoped_provider) {
                candidates.push(scoped_provider);
            }
        }
    }

    candidates
}

/// Process a placeholder segment and assign the best available provider.
///
/// This function finds provider candidates by checking both simple and scoped providers,
/// then selects the best one based on binding success. Scoped providers (e.g.,
/// "apps:addons:list") are preferred over simple providers (e.g., "addons:list") when
/// they can successfully bind to earlier consumer arguments.
fn process_placeholder(
    command_spec: &mut CommandSpec,
    placeholder_name: &str,
    previous_segment: &str,
    positional_name_to_index: &HashMap<String, usize>,
    command_index: &HashSet<String>,
    provider_placeholders: &HashMap<String, Vec<String>>,
    provider_required_flags: &HashMap<String, Vec<String>>,
) {
    let normalized_group = normalize_group_name(previous_segment);

    // Try to find the best provider by checking both scoped and unscoped options
    let provider_candidates =
        find_provider_candidates(&normalized_group, command_spec, positional_name_to_index, command_index);

    // Find the best provider that can be bound
    let mut best_provider: Option<(String, Vec<Bind>)> = None;

    for candidate_id in provider_candidates {
        let binding_outcome = compute_provider_bindings(
            &candidate_id,
            placeholder_name,
            positional_name_to_index,
            command_spec,
            provider_placeholders,
            provider_required_flags,
        );

        match binding_outcome {
            BindingOutcome::Satisfied(bindings) => {
                // Prefer providers with bindings over those without
                if best_provider.is_none() || best_provider.as_ref().unwrap().1.is_empty() {
                    best_provider = Some((candidate_id, bindings));
                }
            }
            BindingOutcome::NoPlaceholders => {
                // Only use this if we don't have a better option
                if best_provider.is_none() {
                    best_provider = Some((candidate_id, vec![]));
                }
            }
            BindingOutcome::Unsatisfied => {
                // Skip this candidate
            }
        }
    }

    if let Some((provider_id, binds)) = best_provider
        && let Some(positional_arg) = command_spec
            .positional_args
            .iter_mut()
            .find(|arg| arg.name == placeholder_name)
    {
        positional_arg.provider = Some(ValueProvider::Command {
            command_id: provider_id,
            binds,
        });
    }
}

/// Attempt to compute bindings for a provider by matching the provider's required
/// path placeholders to consumer fields available before the target positional.
///
/// This function determines if a provider can be bound to consumer inputs by:
/// 1. Checking if the provider has any placeholders or required flags
/// 2. Building a set of available consumer inputs (positionals before target)
/// 3. Attempting to match provider requirements to available inputs using synonyms
///
/// # Arguments
///
/// * `provider_id` - The command ID of the provider (e.g., "apps:list")
/// * `target_positional_name` - The name of the positional argument being processed
/// * `positional_name_to_index` - Mapping of positional names to their indices
/// * `consumer_command` - The command specification that will consume the provider
/// * `provider_placeholders` - Precomputed placeholder mappings
/// * `provider_required_flags` - Precomputed required flag mappings
///
/// # Returns
///
/// * `BindingOutcome::NoPlaceholders` - Provider has no requirements
/// * `BindingOutcome::Satisfied(bindings)` - All requirements can be satisfied
/// * `BindingOutcome::Unsatisfied` - Some requirements cannot be satisfied
fn compute_provider_bindings(
    provider_id: &str,
    target_positional_name: &str,
    positional_name_to_index: &HashMap<String, usize>,
    consumer_command: &CommandSpec,
    provider_placeholders: &HashMap<String, Vec<String>>,
    provider_required_flags: &HashMap<String, Vec<String>>,
) -> BindingOutcome {
    let provider_placeholders = provider_placeholders.get(provider_id).cloned().unwrap_or_default();
    let provider_required_flags = provider_required_flags.get(provider_id).cloned().unwrap_or_default();

    if provider_placeholders.is_empty() && provider_required_flags.is_empty() {
        return BindingOutcome::NoPlaceholders;
    }

    let available_inputs = build_available_inputs(target_positional_name, positional_name_to_index, consumer_command);

    if available_inputs.is_empty() {
        return BindingOutcome::Unsatisfied;
    }

    let name_synonyms = create_name_synonyms_mapping();

    // Attempt to bind path placeholders
    let mut bindings = Vec::new();

    for placeholder_name in provider_placeholders {
        if let Some(binding) = find_binding_for_placeholder(&placeholder_name, &available_inputs, &name_synonyms) {
            bindings.push(binding);
        } else {
            return BindingOutcome::Unsatisfied;
        }
    }

    // Attempt to bind required flags
    if let Err(()) = bind_required_flags(
        &provider_required_flags,
        &available_inputs,
        consumer_command,
        &name_synonyms,
        &mut bindings,
    ) {
        return BindingOutcome::Unsatisfied;
    }

    BindingOutcome::Satisfied(bindings)
}

/// Build a set of available consumer inputs (positionals before the target).
fn build_available_inputs(
    target_positional_name: &str,
    positional_name_to_index: &HashMap<String, usize>,
    consumer_command: &CommandSpec,
) -> HashSet<String> {
    let target_index = match positional_name_to_index.get(target_positional_name) {
        Some(&index) => index,
        None => return HashSet::new(),
    };

    consumer_command
        .positional_args
        .iter()
        .enumerate()
        .filter(|(index, _)| *index < target_index)
        .map(|(_, positional)| positional.name.clone())
        .collect()
}

/// Create a mapping of name synonyms for flexible binding matching.
fn create_name_synonyms_mapping() -> HashMap<&'static str, &'static [&'static str]> {
    HashMap::from([
        ("app", &["app", "app_id"][..]),
        ("app_id", &["app_id", "app"][..]),
        ("addon", &["addon", "addon_id"][..]),
        ("addon_id", &["addon_id", "addon"][..]),
        ("team", &["team", "team_name"][..]),
        ("team_name", &["team_name", "team"][..]),
        ("pipeline", &["pipeline"][..]),
        ("space", &["space", "space_id"][..]),
        ("space_id", &["space_id", "space"][..]),
        ("region", &["region"][..]),
        ("stack", &["stack"][..]),
    ])
}

/// Find a binding for a placeholder by checking synonyms against available inputs.
fn find_binding_for_placeholder(
    placeholder_name: &str,
    available_inputs: &HashSet<String>,
    name_synonyms: &HashMap<&str, &[&str]>,
) -> Option<Bind> {
    let candidate_names: Vec<String> = name_synonyms
        .get(placeholder_name)
        .map(|synonyms| synonyms.iter().map(|s| s.to_string()).collect())
        .unwrap_or_else(|| vec![placeholder_name.to_string()]);

    candidate_names
        .iter()
        .find(|candidate| available_inputs.contains(*candidate))
        .map(|found_name| Bind {
            provider_key: placeholder_name.to_string(),
            from: found_name.clone(),
        })
}

/// Attempt to bind required provider flags to available consumer inputs.
///
/// Returns `Ok(())` if all required flags can be bound, `Err(())` otherwise.
fn bind_required_flags(
    required_flags: &[String],
    available_inputs: &HashSet<String>,
    consumer_command: &CommandSpec,
    name_synonyms: &HashMap<&str, &[&str]>,
    bindings: &mut Vec<Bind>,
) -> Result<(), ()> {
    let allowed_flag_names: HashSet<&str> = HashSet::from([
        "app",
        "app_id",
        "pipeline",
        "team",
        "team_name",
        "addon",
        "addon_id",
        "space",
        "space_id",
        "region",
        "stack",
    ]);

    let consumer_required_flag_names: HashSet<String> = consumer_command
        .flags
        .iter()
        .filter(|flag| flag.required)
        .map(|flag| flag.name.clone())
        .collect();

    for required_flag in required_flags {
        if !allowed_flag_names.contains(required_flag.as_str()) {
            continue; // Skip non-core flag names
        }

        let candidate_names: Vec<String> = name_synonyms
            .get(required_flag.as_str())
            .map(|synonyms| synonyms.iter().map(|s| s.to_string()).collect())
            .unwrap_or_else(|| vec![required_flag.clone()]);

        // Try to bind from available positional inputs first
        if let Some(found_name) = candidate_names
            .iter()
            .find(|candidate| available_inputs.contains(*candidate))
        {
            bindings.push(Bind {
                provider_key: required_flag.clone(),
                from: found_name.clone(),
            });
            continue;
        }

        // Try to bind from consumer required flags
        if let Some(found_name) = candidate_names
            .iter()
            .find(|candidate| consumer_required_flag_names.contains(*candidate))
        {
            bindings.push(Bind {
                provider_key: required_flag.clone(),
                from: found_name.clone(),
            });
            continue;
        }

        // Could not satisfy this required flag
        return Err(());
    }

    Ok(())
}

/// Extract placeholder names from a command path.
///
/// This function parses a path like `/apps/{app_id}/config-vars/{key}` and returns
/// the placeholder names `["app_id", "key"]`.
fn extract_path_placeholders(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter_map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                let placeholder_name = segment.trim_start_matches('{').trim_end_matches('}').trim().to_string();
                Some(placeholder_name)
            } else {
                None
            }
        })
        .collect()
}

/// Normalize a group name for provider resolution.
///
/// This function handles special cases where the group name in the path doesn't
/// directly correspond to the group name used for list commands.
///
/// # Examples
///
/// - `"config-vars"` → `"config"` (config-vars list command is under config group)
/// - `"apps"` → `"apps"` (no change needed)
fn normalize_group_name(group_name: &str) -> String {
    match group_name {
        "config-vars" => "config".to_string(),
        _ => group_name.to_string(),
    }
}

/// Detect API version segments in command paths.
///
/// This function identifies version segments like `v1`, `v2`, etc. that should
/// be ignored when determining the group for provider resolution.
///
/// # Examples
///
/// - `"v1"` → `true`
/// - `"v2"` → `true`
/// - `"apps"` → `false`
/// - `"v"` → `false` (too short)
fn is_version_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    trimmed.len() > 1 && trimmed.starts_with('v') && trimmed[1..].chars().all(|c| c.is_ascii_digit())
}

/// Map a flag name to its corresponding group name.
///
/// This function first checks the synonyms mapping, then falls back to
/// conservative pluralization rules.
///
/// # Arguments
///
/// * `flag_name` - The name of the flag to map
/// * `synonyms` - Mapping of flag names to group names
///
/// # Returns
///
/// * `Some(group_name)` if a mapping is found
/// * `None` if no mapping can be determined
fn map_flag_name_to_group(flag_name: &str, synonyms: &HashMap<&str, &str>) -> Option<String> {
    let normalized_flag_name = flag_name.to_lowercase();

    if let Some(&group_name) = synonyms.get(normalized_flag_name.as_str()) {
        return Some(group_name.to_string());
    }

    apply_conservative_pluralization(&normalized_flag_name)
}

/// Apply conservative pluralization rules for group name inference.
///
/// This function attempts to pluralize a singular noun to match common
/// Heroku CLI group naming patterns. It uses conservative rules to avoid
/// incorrect pluralizations.
///
/// # Rules
///
/// 1. If already ends with 's', return as-is
/// 2. If ends with 'y' (not preceded by vowel), change to 'ies'
/// 3. If ends with 'x', 'ch', or 'sh', add 'es'
/// 4. Otherwise, add 's'
///
/// # Arguments
///
/// * `singular_name` - The singular form to pluralize
///
/// # Returns
///
/// * `Some(plural_name)` if pluralization is possible
/// * `None` if the input is empty or invalid
fn apply_conservative_pluralization(singular_name: &str) -> Option<String> {
    if singular_name.is_empty() {
        return None;
    }

    if singular_name.ends_with('s') {
        return Some(singular_name.to_string());
    }

    if singular_name.ends_with('y') && singular_name.len() > 1 {
        let second_to_last_char = singular_name.chars().nth(singular_name.len() - 2).unwrap();
        if !matches!(second_to_last_char, 'a' | 'e' | 'i' | 'o' | 'u') {
            return Some(format!("{}ies", &singular_name[..singular_name.len() - 1]));
        }
    }

    if singular_name.ends_with('x') || singular_name.ends_with("ch") || singular_name.ends_with("sh") {
        return Some(format!("{}es", singular_name));
    }

    Some(format!("{}s", singular_name))
}
