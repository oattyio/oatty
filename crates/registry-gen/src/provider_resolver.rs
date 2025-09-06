//! Provider resolution (two-pass) for registry generation.
//!
//! This module verifies and assigns ValueProviders to flags and positional
//! arguments only after the full command list is known. It builds an index of
//! `<group>:<name>` → CommandSpec presence so provider candidates can be
//! verified with 100% confidence. Heuristics are still used to propose
//! candidates, but only verified providers are assigned.

use std::collections::{HashMap, HashSet};

use heroku_types::{CommandFlag, CommandSpec, PositionalArgument, ValueProvider};

/// Resolve ValueProviders with a two-pass strategy.
///
/// Pass 1 occurs in `schema::derive_commands_from_schema`, which constructs all
/// `CommandSpec`s. This function performs Pass 2: build a `<group>:<name>`
/// index from the constructed commands, then populate per-field providers where
/// a verified `"<group>:list"` exists.
pub fn resolve_and_infer_providers(commands: &mut [CommandSpec]) {
    let index = build_command_index(commands);
    let list_groups = groups_with_list(&index);

    for spec in commands.iter_mut() {
        // Only set per-field providers; no legacy bindings are persisted.
        apply_flag_providers(&mut spec.flags, &list_groups, &index);
        apply_positional_providers(&mut spec.positional_args, &spec.path, &list_groups, &index);
    }
}

/// Build an index of `<group>:<name>`.
fn build_command_index(commands: &[CommandSpec]) -> HashSet<String> {
    commands
        .iter()
        .map(|c| format!("{}:{}", c.group, c.name))
        .collect()
}

/// Find groups that have a `list` command defined.
fn groups_with_list(index: &HashSet<String>) -> HashSet<String> {
    index
        .iter()
        .filter_map(|k| k.split_once(':'))
        .filter(|(_, name)| *name == "list")
        .map(|(group, _)| group.to_string())
        .collect()
}

/// Apply verified flag providers onto `CommandFlag` structs.
fn apply_flag_providers(flags: &mut [CommandFlag], list_groups: &std::collections::HashSet<String>, index: &std::collections::HashSet<String>) {
    let synonyms: HashMap<&str, &str> = HashMap::from([
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
    ]);
    for flag in flags.iter_mut() {
        if let Some(group) = map_flag_to_group(&flag.name, &synonyms) {
            let provider_id = format!("{}:{}", group, "list");
            if list_groups.contains(&group) && index.contains(&provider_id) {
                flag.provider = Some(ValueProvider::Command { command_id: provider_id });
            }
        }
    }
}

/// Apply verified positional providers onto `PositionalArgument` structs.
fn apply_positional_providers(positionals: &mut [PositionalArgument], path: &str, list_groups: &std::collections::HashSet<String>, index: &std::collections::HashSet<String>) {
    let mut previous_concrete: Option<String> = None;
    for segment in path.trim_start_matches('/').split('/') {
        if segment.starts_with('{') && segment.ends_with('}') {
            let name = segment.trim_start_matches('{').trim_end_matches('}').trim().to_string();
            if let Some(prev) = &previous_concrete {
                let group = normalize_group(prev);
                let provider_id = format!("{}:{}", group, "list");
                if list_groups.contains(&group) && index.contains(&provider_id) {
                    if let Some(arg) = positionals.iter_mut().find(|a| a.name == name) {
                        arg.provider = Some(ValueProvider::Command { command_id: provider_id });
                    }
                }
            }
        } else if !segment.is_empty() && !is_version_segment(segment) {
            previous_concrete = Some(segment.to_string());
        }
    }
}

/// Infer positional providers by looking at path placeholders and the previous
/// concrete segment, then verify the corresponding `<group>:list` exists.
// No longer returns bindings; handled directly in apply_positional_providers

/// Infer flag providers from flag names using a conservative synonyms table,
/// then verify the corresponding `<group>:list` exists.
// No longer returns bindings; handled directly in apply_flag_providers

/// Normalize a group name (currently only remaps "config-vars" → "config").
fn normalize_group(group: &str) -> String {
    if group == "config-vars" { "config".to_string() } else { group.to_string() }
}

/// Detect simple API version segments like `v1`, `v2`.
fn is_version_segment(s: &str) -> bool {
    let s = s.trim();
    s.len() > 1 && s.starts_with('v') && s[1..].chars().all(|c| c.is_ascii_digit())
}

/// Map a flag name to a plural group via synonyms or conservative pluralization.
fn map_flag_to_group(flag: &str, synonyms: &HashMap<&str, &str>) -> Option<String> {
    let key = flag.to_lowercase();
    if let Some(&group) = synonyms.get(key.as_str()) {
        return Some(group.to_string());
    }
    conservative_plural(&key)
}

/// Conservative pluralization used for group matching.
fn conservative_plural(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if s.ends_with('s') {
        return Some(s.to_string());
    }
    if s.ends_with('y')
        && s.len() > 1
        && !matches!(s.chars().nth(s.len() - 2).unwrap(), 'a' | 'e' | 'i' | 'o' | 'u')
    {
        return Some(format!("{}ies", &s[..s.len() - 1]));
    }
    if s.ends_with('x') || s.ends_with("ch") || s.ends_with("sh") {
        return Some(format!("{}es", s));
    }
    Some(format!("{}s", s))
}
