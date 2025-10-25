use heroku_types::{CommandExecution, CommandSpec, ItemKind, SuggestionItem};
use heroku_util::{fuzzy_score, lex_shell_like, lex_shell_like_ranged};
use std::sync::Arc;

use heroku_engine::provider::{PendingProviderFetch, ProviderSuggestionSet, ValueProvider};

// ===== Types =====

/// Result of a suggestion build operation containing suggestion items and loading state.
///
/// This struct encapsulates the output of the suggestion engine, providing both
/// the list of available suggestions and information about whether value providers
/// are still loading data.
pub(crate) struct SuggestionResult {
    /// List of suggestion items available for the current input context
    pub items: Vec<SuggestionItem>,
    /// Whether value providers are currently loading data for suggestions
    pub provider_loading: bool,
    /// Pending provider fetches that should be dispatched by the caller.
    pub pending_fetches: Vec<PendingProviderFetch>,
}

#[derive(Default)]
struct ProviderSuggestionAggregate {
    items: Vec<SuggestionItem>,
    pending_fetches: Vec<PendingProviderFetch>,
    provider_loading: bool,
}

impl ProviderSuggestionAggregate {
    fn extend(&mut self, mut result: ProviderSuggestionSet) {
        if let Some(fetch) = result.pending_fetch.take() {
            self.provider_loading = true;
            self.pending_fetches.push(fetch);
        }
        if !result.items.is_empty() {
            self.items.extend(result.items);
        }
    }

    fn into_parts(self) -> (Vec<SuggestionItem>, Vec<PendingProviderFetch>, bool) {
        (self.items, self.pending_fetches, self.provider_loading)
    }
}

struct PositionalSuggestionContext<'a> {
    commands: &'a [CommandSpec],
    spec: &'a CommandSpec,
    remaining_parts: &'a [String],
    current_input: &'a str,
    ends_with_space: bool,
    current_is_flag: bool,
    providers: &'a [Arc<dyn ValueProvider>],
    user_args_len: usize,
}

fn build_positional_suggestions(context: PositionalSuggestionContext<'_>) -> (Vec<SuggestionItem>, Vec<PendingProviderFetch>, bool) {
    let PositionalSuggestionContext {
        commands,
        spec,
        remaining_parts,
        current_input,
        ends_with_space,
        current_is_flag,
        providers,
        user_args_len,
    } = context;

    let first_flag_idx = remaining_parts
        .iter()
        .position(|t| t.starts_with("--"))
        .unwrap_or(remaining_parts.len());
    let editing_positional = !current_is_flag
        && !remaining_parts.is_empty()
        && (remaining_parts.len() - 1) < first_flag_idx
        && !spec.positional_args.is_empty()
        && !ends_with_space;

    if editing_positional {
        let arg_index = (remaining_parts.len() - 1).min(spec.positional_args.len().saturating_sub(1));
        return SuggestionEngine::build_for_index(commands, spec, arg_index, current_input, providers, remaining_parts);
    }
    if user_args_len < spec.positional_args.len() && !current_is_flag {
        return SuggestionEngine::build_for_index(commands, spec, user_args_len, "", providers, remaining_parts);
    }

    (Vec::new(), Vec::new(), false)
}

/// Engine responsible for building command suggestions based on user input and available commands.
///
/// The suggestion engine analyzes user input tokens and generates contextually relevant
/// suggestions including commands, flags, positional arguments, and values from providers.
// ===== Engine =====
pub(crate) struct SuggestionEngine;

impl SuggestionEngine {
    // Breakout: if command is not yet resolved, return command suggestions
    fn suggest_when_unresolved(commands: &[CommandSpec], tokens: &[String]) -> Option<SuggestionResult> {
        if !is_command_resolved(commands, tokens) {
            let items = suggest_commands(commands, &compute_command_prefix(tokens));
            return Some(SuggestionResult {
                items,
                provider_loading: false,
                pending_fetches: Vec::new(),
            });
        }
        None
    }

    // Breakout: resolve spec reference from tokens
    fn resolve_spec<'a>(commands: &'a [CommandSpec], tokens: &[String]) -> Option<&'a CommandSpec> {
        let group: &str = tokens.first().map(|s| s.as_str()).unwrap_or("");
        let name: &str = tokens.get(1).map(|s| s.as_str()).unwrap_or("");
        commands.iter().find(|c| c.group == group && c.name == name)
    }

    // Breakout: handle case where a non-boolean flag value is pending
    fn suggest_for_pending_flag(
        commands: &[CommandSpec],
        spec: &CommandSpec,
        remaining_parts: &[String],
        input: &str,
        providers: &[Arc<dyn ValueProvider>],
    ) -> Option<SuggestionResult> {
        let pending_flag = find_pending_flag(spec, remaining_parts, input);
        if let Some(flag_name) = pending_flag {
            let value_partial = flag_value_partial(remaining_parts);
            let (items, pending_fetches, provider_loading) =
                suggest_values_for_flag(commands, spec, &flag_name, &value_partial, providers, remaining_parts);
            return Some(SuggestionResult {
                items,
                provider_loading,
                pending_fetches,
            });
        }
        None
    }
    fn build_for_index(
        commands: &[CommandSpec],
        spec: &CommandSpec,
        index: usize,
        current: &str,
        providers: &[Arc<dyn ValueProvider>],
        remaining_parts: &[String],
    ) -> (Vec<SuggestionItem>, Vec<PendingProviderFetch>, bool) {
        suggest_positionals(commands, spec, index, current, providers, remaining_parts)
    }
    // Breakout: extend with required/optional flags depending on context
    fn extend_flag_suggestions(
        spec: &CommandSpec,
        user_flags: &[String],
        current_input: &str,
        current_is_flag: bool,
        items: &mut Vec<SuggestionItem>,
    ) {
        if items.is_empty() {
            let required_remaining = required_flags_remaining(spec, user_flags);
            if required_remaining || current_is_flag {
                items.extend(collect_flag_candidates(spec, user_flags, current_input, true));
            }
        }
        if items.is_empty() {
            items.extend(collect_flag_candidates(spec, user_flags, current_input, false));
        }
    }

    /// Builds suggestions based on the current input, command registry, and value providers.
    ///
    /// This is the main entry point for generating suggestions. It analyzes the input
    /// to determine the current command context and generates appropriate suggestions
    /// for the next expected input.
    ///
    /// # Arguments
    ///
    /// * `registry` - The command registry containing available commands and their specifications
    /// * `providers` - List of value providers that can suggest values for flags and arguments
    /// * `input` - The current user input string to generate suggestions for
    ///
    /// # Returns
    ///
    /// A `SuggestionResult` containing the available suggestions and loading state.
    ///
    /// # Examples
    ///
    /// ```
    /// let result = SuggestionEngine::build(&registry, &providers, "apps info --app ");
    /// ```
    pub fn build(commands: &[CommandSpec], providers: &[Arc<dyn ValueProvider>], input: &str) -> SuggestionResult {
        let input_tokens: Vec<String> = lex_shell_like(input);

        if let Some(out) = Self::suggest_when_unresolved(commands, &input_tokens) {
            return out;
        }

        let Some(spec) = Self::resolve_spec(commands, &input_tokens) else {
            return SuggestionResult {
                items: vec![],
                provider_loading: false,
                pending_fetches: Vec::new(),
            };
        };

        // Extract remaining parts after command
        let remaining_parts: &[String] = if input_tokens.len() >= 2 {
            &input_tokens[2..]
        } else {
            &input_tokens[0..0]
        };
        let (user_flags, user_args, _flag_values) = parse_user_flags_args(spec, remaining_parts);
        let current_input = remaining_parts.last().map(|s| s.as_str()).unwrap_or("");
        let ends_with_space = input.ends_with(' ') || input.ends_with('\t') || input.ends_with('\n') || input.ends_with('\r');
        let current_is_flag = current_input.starts_with('-');

        if let Some(out) = Self::suggest_for_pending_flag(commands, spec, remaining_parts, input, providers) {
            return out;
        }

        // Positionals:
        // - If the user is currently typing a positional token (no trailing space),
        //   suggest values for that positional index.
        // - Otherwise, suggest for the next positional if any remain.
        let context = PositionalSuggestionContext {
            commands,
            spec,
            remaining_parts,
            current_input,
            ends_with_space,
            current_is_flag,
            providers,
            user_args_len: user_args.len(),
        };
        let (mut items, pending_fetches, mut provider_loading) = build_positional_suggestions(context);

        // Suggest required flags if needed (or if user is typing a flag)
        Self::extend_flag_suggestions(spec, &user_flags, current_input, current_is_flag, &mut items);

        if !pending_fetches.is_empty() {
            provider_loading = true;
        }

        SuggestionResult {
            items,
            provider_loading,
            pending_fetches,
        }
    }
}

/// Checks if a command has been fully resolved from the input tokens.
///
/// A command is considered resolved when both the group and name tokens
/// match a command in the registry.
///
/// # Arguments
///
/// * `registry` - The command registry to search in
/// * `tokens` - The parsed input tokens
///
/// # Returns
///
/// `true` if the command is resolved, `false` otherwise.
// ===== Command resolution helpers =====
fn is_command_resolved(commands: &[CommandSpec], tokens: &[String]) -> bool {
    if tokens.len() < 2 {
        return false;
    }
    let (group, name) = (&tokens[0], &tokens[1]);
    commands.iter().any(|c| &c.group == group && &c.name == name)
}

/// Computes the command prefix from the input tokens.
///
/// This function extracts the command portion from the input tokens,
/// formatting it as "group name" or just "group" if only one token exists.
///
/// # Arguments
///
/// * `tokens` - The parsed input tokens
///
/// # Returns
///
/// A string representing the command prefix for suggestion matching.
fn compute_command_prefix(tokens: &[String]) -> String {
    if tokens.len() >= 2 {
        format!("{} {}", tokens[0], tokens[1])
    } else {
        tokens.first().map(|s| s.as_str()).unwrap_or("").to_string()
    }
}

/// Generates command suggestions based on a prefix.
///
/// Uses fuzzy matching to find commands that match the given prefix,
/// scoring them by relevance and formatting them for display.
///
/// # Arguments
///
/// * `registry` - The command registry to search in
/// * `prefix` - The prefix to match commands against
///
/// # Returns
///
/// A vector of suggestion items for matching commands.
fn suggest_commands(commands: &[CommandSpec], prefix: &str) -> Vec<SuggestionItem> {
    let mut items = Vec::new();
    if prefix.is_empty() {
        return items;
    }

    for command in commands {
        let executable = command.canonical_id();
        let Some(score) = fuzzy_score(&executable, prefix) else {
            continue;
        };
        let (exec_type, kind) = match command.execution {
            CommandExecution::Http { .. } => ("[CMD]", ItemKind::Command),
            CommandExecution::Mcp(..) => ("[MCP]", ItemKind::MCP),
        };
        let summary = command.summary.trim();
        let meta = if summary.is_empty() { None } else { Some(summary.to_string()) };
        items.push(SuggestionItem {
            display: format!("{} {} {}", exec_type, executable.as_str(), command.summary),
            insert_text: executable,
            kind,
            meta,
            score,
        });
    }
    items
}

/// Parses user input to separate flags and positional arguments.
///
/// Analyzes the input parts to identify which tokens represent flags
/// and which represent positional arguments, handling both `--flag value`
/// and `--flag=value` syntax.
///
/// # Arguments
///
/// * `spec` - The command specification containing flag definitions
/// * `parts` - The input parts to parse
///
/// # Returns
///
/// A tuple of (user_flags, user_args) where both are vectors of strings.
// ===== Parsing helpers =====
pub(crate) fn parse_user_flags_args(
    spec: &CommandSpec,
    parts: &[String],
) -> (Vec<String>, Vec<String>, std::collections::HashMap<String, String>) {
    let mut user_flags: Vec<String> = Vec::new();
    let mut user_args: Vec<String> = Vec::new();
    let mut flag_values: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut i = 0;

    while i < parts.len() {
        let token = parts[i].as_str();
        if token.starts_with("--") {
            let name = token.trim_start_matches('-');
            user_flags.push(name.to_string());

            // Handle non-boolean flag values
            if let Some(flag) = spec.flags.iter().find(|flag| flag.name == name)
                && flag.r#type != "boolean"
                && i + 1 < parts.len()
                && !parts[i + 1].starts_with('-')
            {
                i += 1; // consume value
                flag_values.insert(name.to_string(), parts[i].clone());
            }
        } else if token.contains('=') && token.starts_with("--") {
            let name = token.split('=').next().unwrap_or("").trim_start_matches('-');
            user_flags.push(name.to_string());
            if let Some(eq) = token.find('=') {
                let val = token[eq + 1..].to_string();
                flag_values.insert(name.to_string(), val);
            }
        } else {
            user_args.push(token.to_string());
        }
        i += 1;
    }

    (user_flags, user_args, flag_values)
}

/// Finds a pending flag that requires a value.
///
/// Searches backwards through the input parts to find a flag that
/// expects a value but doesn't have one yet.
///
/// # Arguments
///
/// * `spec` - The command specification
/// * `parts` - The input parts to search through
/// * `input` - The original input string for completion checking
///
/// # Returns
///
/// `Some(flag_name)` if a pending flag is found, `None` otherwise.
fn find_pending_flag(spec: &CommandSpec, parts: &[String], input: &str) -> Option<String> {
    let mut j = (parts.len() as isize) - 1;

    while j >= 0 {
        let token = parts[j as usize].as_str();
        if token.starts_with("--") {
            let name = token.trim_start_matches('-');
            if let Some(flag) = spec.flags.iter().find(|flag| flag.name == name)
                && flag.r#type != "boolean"
                && (((j as usize) == parts.len() - 1 || parts[(j as usize) + 1].starts_with('-')) && !is_flag_value_complete(input))
            {
                return Some(name.to_string());
            }
            break;
        }
        j -= 1;
    }

    None
}

/// Extracts the partial value for a flag from the input parts.
///
/// Handles both `--flag=value` and `--flag value` syntax to extract
/// the current partial value being typed for a flag.
///
/// # Arguments
///
/// * `parts` - The input parts to search through
///
/// # Returns
///
/// The partial value string, or empty string if no value is present.
fn flag_value_partial(parts: &[String]) -> String {
    if let Some(last) = parts.last() {
        let last_part = last.as_str();
        if last_part.starts_with("--") {
            if let Some(eq) = last_part.find('=') {
                return last_part[eq + 1..].to_string();
            }
            return String::new();
        }
        return last_part.to_string();
    }
    String::new()
}

/// Generates value suggestions for a specific flag.
///
/// Combines enum values from the flag definition with dynamic values
/// from value providers to create comprehensive suggestions.
///
/// # Arguments
///
/// * `spec` - The command specification
/// * `flag_name` - The name of the flag to suggest values for
/// * `partial` - The partial value being typed
/// * `providers` - List of value providers to query
///
/// # Returns
///
/// A vector of suggestion items for the flag values.
// ===== Suggestion builders =====
fn suggest_values_for_flag(
    commands: &[CommandSpec],
    spec: &CommandSpec,
    flag_name: &str,
    partial: &str,
    providers: &[Arc<dyn ValueProvider>],
    remaining_parts: &[String],
) -> (Vec<SuggestionItem>, Vec<PendingProviderFetch>, bool) {
    let mut items: Vec<SuggestionItem> = Vec::new();

    // Add enum values from flag definition
    if let Some(flag) = spec.flags.iter().find(|flag| flag.name == flag_name) {
        for value in &flag.enum_values {
            if let Some(score) = fuzzy_score(value, partial) {
                items.push(SuggestionItem {
                    display: value.clone(),
                    insert_text: value.clone(),
                    kind: ItemKind::Value,
                    meta: Some("enum".into()),
                    score,
                });
            }
        }
    }

    // Add dynamic values from providers
    let command_key = format!("{} {}", spec.group, spec.name);
    let inputs_map = build_inputs_map_for_flag(spec, remaining_parts, flag_name);
    let mut aggregate = ProviderSuggestionAggregate::default();
    for provider in providers {
        let result = provider.suggest(commands, &command_key, flag_name, partial, &inputs_map);
        aggregate.extend(result);
    }
    let (provider_items, pending_fetches, provider_loading_flag) = aggregate.into_parts();
    let provider_loading = provider_loading_flag || !pending_fetches.is_empty();

    items.extend(provider_items);
    (items, pending_fetches, provider_loading)
}

/// Generates suggestions for positional arguments.
///
/// Queries value providers for suggestions and fall back to a generic
/// placeholder if no provider suggestions are available.
///
/// # Arguments
///
/// * `spec` - The command specification
/// * `arg_count` - The current argument position (0-based)
/// * `current` - The partial value being typed
/// * `providers` - List of value providers to query
///
/// # Returns
///
/// A vector of suggestion items for the positional argument.
fn suggest_positionals(
    commands: &[CommandSpec],
    spec: &CommandSpec,
    arg_count: usize,
    current: &str,
    providers: &[Arc<dyn ValueProvider>],
    remaining_parts: &[String],
) -> (Vec<SuggestionItem>, Vec<PendingProviderFetch>, bool) {
    let mut items: Vec<SuggestionItem> = Vec::new();
    let mut pending_fetches: Vec<PendingProviderFetch> = Vec::new();
    let mut provider_loading = false;

    if let Some(positional_arg) = spec.positional_args.get(arg_count) {
        let command_key = format!("{} {}", spec.group, spec.name);

        // Query providers for dynamic suggestions
        let inputs_map = build_inputs_map_for_positional(spec, arg_count, remaining_parts);
        let mut aggregate = ProviderSuggestionAggregate::default();
        for provider in providers {
            let result = provider.suggest(commands, &command_key, &positional_arg.name, current, &inputs_map);
            aggregate.extend(result);
        }
        let (mut provider_items, fetches, loading_flag) = aggregate.into_parts();
        pending_fetches = fetches;
        provider_loading = loading_flag || !pending_fetches.is_empty();

        if !current.is_empty() {
            provider_items.retain(|item| item.insert_text != current);
        }

        items.extend(provider_items);

        // Fall back to generic placeholder if no provider suggestions
        if items.is_empty() && current.trim().is_empty() {
            items.push(SuggestionItem {
                display: format!(
                    "<{}> [ARG] {}",
                    positional_arg.name,
                    positional_arg.help.as_deref().unwrap_or(&positional_arg.name)
                ),
                insert_text: format!("<{}>", positional_arg.name),
                kind: ItemKind::Positional,
                meta: positional_arg.help.clone(),
                score: 0,
            });
        }
    }

    (items, pending_fetches, provider_loading)
}

fn build_inputs_map_for_positional(
    spec: &CommandSpec,
    arg_count: usize,
    remaining_parts: &[String],
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut map: HashMap<String, String> = HashMap::new();
    let (_user_flags, user_args, flag_values) = parse_user_flags_args(spec, remaining_parts);
    for (i, val) in user_args.into_iter().enumerate() {
        if i < arg_count
            && let Some(pa) = spec.positional_args.get(i)
        {
            map.insert(pa.name.clone(), val);
        }
    }
    for (k, v) in flag_values.into_iter() {
        map.insert(k, v);
    }
    map
}

fn build_inputs_map_for_flag(
    spec: &CommandSpec,
    remaining_parts: &[String],
    current_flag: &str,
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut map: HashMap<String, String> = HashMap::new();
    let (_user_flags, user_args, flag_values) = parse_user_flags_args(spec, remaining_parts);
    for (i, val) in user_args.into_iter().enumerate() {
        if let Some(pa) = spec.positional_args.get(i) {
            map.insert(pa.name.clone(), val);
        }
    }
    for (k, v) in flag_values.into_iter() {
        if k != current_flag {
            map.insert(k, v);
        }
    }
    map
}

/// Checks if any required flags are still missing from user input.
///
/// Compares the user-provided flags against the command specification
/// to determine if any required flags are still needed.
///
/// # Arguments
///
/// * `spec` - The command specification
/// * `user_flags` - The flags already provided by the user
///
/// # Returns
///
/// `true` if required flags are missing, `false` if all required flags are present.
// ===== Validation and checks =====
pub(crate) fn required_flags_remaining(spec: &CommandSpec, user_flags: &[String]) -> bool {
    spec.flags
        .iter()
        .any(|flag| flag.required && !user_flags.iter().any(|user_flag| user_flag == &flag.name))
}

/// Collects flag candidates based on requirements and current input.
///
/// Filters flags based on whether they're required or optional, and
/// formats them for display with appropriate metadata.
///
/// # Arguments
///
/// * `spec` - The command specification
/// * `user_flags` - The flags already provided by the user
/// * `current` - The current partial input being typed
/// * `required_only` - Whether to only include required flags
///
/// # Returns
///
/// A vector of suggestion items for the available flags.
fn collect_flag_candidates(spec: &CommandSpec, user_flags: &[String], current: &str, required_only: bool) -> Vec<SuggestionItem> {
    let mut out: Vec<SuggestionItem> = Vec::new();

    for flag in &spec.flags {
        // Filter by requirement status
        if required_only && !flag.required {
            continue;
        }
        if !required_only && flag.required {
            continue;
        }

        // Skip already provided flags
        if user_flags.iter().any(|user_flag| user_flag == &flag.name) {
            continue;
        }

        let long = format!(
            "--{:<15} [FLAG] {}",
            flag.name,
            flag.description.as_ref().unwrap_or(&"".to_string())
        );

        // Include based on current input
        let include = if current.starts_with('-') {
            long.starts_with(current)
        } else {
            true
        };

        if include {
            out.push(SuggestionItem {
                display: format!("{:<22}{}", long, if flag.required { "  [required]" } else { "" }),
                insert_text: format!("--{}", flag.name),
                kind: ItemKind::Flag,
                meta: flag.description.clone(),
                score: 0,
            });
        }
    }

    out
}

/// Checks if a flag value input is complete.
///
/// Analyzes the input string to determine if the current flag value
/// input is complete or still in progress.
///
/// # Arguments
///
/// * `input` - The input string to analyze
///
/// # Returns
///
/// `true` if the flag value is complete, `false` if it's still being typed.
pub(crate) fn is_flag_value_complete(input: &str) -> bool {
    let tokens_ranged = lex_shell_like_ranged(input);
    let tokens: Vec<&str> = tokens_ranged.iter().map(|token| token.text).collect();
    let token_count = tokens.len();

    if token_count == 0 {
        return false;
    }

    let last_token = tokens[token_count - 1];
    if last_token == "-" || last_token == "--" {
        return false;
    }

    let mut last_flag_idx: isize = -1;
    for i in (0..token_count).rev() {
        if tokens[i].starts_with('-') {
            last_flag_idx = i as isize;
            break;
        }
    }

    if last_flag_idx == -1 {
        return true;
    }

    if last_flag_idx as usize == token_count - 1 {
        return false;
    }

    if last_flag_idx as usize == token_count - 2 {
        return input.ends_with(' ') || input.ends_with('\t') || input.ends_with('\n') || input.ends_with('\r');
    }

    true
}

// ===== Tests =====
#[cfg(test)]
mod tests {
    use super::*;
    use heroku_engine::provider::{PendingProviderFetch, ProviderFetchPlan};
    use heroku_registry::CommandRegistry;
    use heroku_types::{CommandExecution, CommandFlag, HttpCommandSpec, PositionalArgument, ServiceId};

    #[derive(Debug)]
    struct TestProvider {
        map: std::collections::HashMap<(String, String), Vec<String>>, // ((command_key, field) -> values)
    }

    impl ValueProvider for TestProvider {
        fn suggest(
            &self,
            _commands: &[CommandSpec],
            command_key: &str,
            field: &str,
            _partial: &str,
            _inputs: &std::collections::HashMap<String, String>,
        ) -> ProviderSuggestionSet {
            let mut items = Vec::new();
            if let Some(values) = self.map.get(&(command_key.to_string(), field.to_string())) {
                for value in values {
                    items.push(SuggestionItem {
                        display: value.clone(),
                        insert_text: value.clone(),
                        kind: ItemKind::Value,
                        meta: Some("test-provider".into()),
                        score: 1,
                    });
                }
            }
            ProviderSuggestionSet::ready(items)
        }
    }

    #[derive(Debug)]
    struct PendingProvider;

    impl ValueProvider for PendingProvider {
        fn suggest(
            &self,
            _commands: &[CommandSpec],
            _command_key: &str,
            _field: &str,
            _partial: &str,
            _inputs: &std::collections::HashMap<String, String>,
        ) -> ProviderSuggestionSet {
            let plan = ProviderFetchPlan::new("apps list".into(), "apps list::pending".into(), serde_json::Map::new());
            let pending = PendingProviderFetch::new(plan, true);
            ProviderSuggestionSet::with_pending(Vec::new(), pending)
        }
    }

    fn registry_with(commands: Vec<CommandSpec>) -> CommandRegistry {
        CommandRegistry {
            commands,
            workflows: vec![],
            provider_contracts: Default::default(),
        }
    }

    #[test]
    fn suggests_commands_before_resolution() {
        let reg = registry_with(vec![
            CommandSpec {
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/apps".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
                group: "apps".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
            },
            CommandSpec {
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/apps/{app}".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
                group: "apps".into(),
                name: "info".into(),
                summary: "info".into(),
                positional_args: vec![],
                flags: vec![],
            },
        ]);
        let result = SuggestionEngine::build(&reg.commands, &[], "ap");
        assert!(!result.items.is_empty());
        assert!(result.items.iter().any(|item| matches!(item.kind, ItemKind::Command)));
        assert!(!result.provider_loading);
    }

    #[test]
    fn suggests_flag_values_enum_and_provider() {
        // apps info with --region enum and --app provider
        let spec = CommandSpec {
            execution: CommandExecution::Http(HttpCommandSpec {
                method: "GET".into(),
                path: "/apps/{app}".into(),
                ranges: vec![],
                // Provider is now embedded on the field; legacy vector removed
                service_id: ServiceId::CoreApi,
                output_schema: None,
            }),
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![],
            flags: vec![
                CommandFlag {
                    name: "region".into(),
                    short_name: None,
                    required: false,
                    r#type: "enum".into(),
                    enum_values: vec!["us".into(), "eu".into()],
                    default_value: None,
                    description: None,
                    provider: None,
                },
                CommandFlag {
                    name: "app".into(),
                    short_name: None,
                    required: false,
                    r#type: "string".into(),
                    enum_values: vec![],
                    default_value: None,
                    description: None,
                    provider: Some(heroku_types::ValueProvider::Command {
                        command_id: "apps:list".into(),
                        binds: vec![],
                    }),
                },
            ],
        };
        let reg = registry_with(vec![
            CommandSpec {
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/apps".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
                group: "apps".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
            },
            spec,
        ]);
        // Provider embedded on flag in the spec
        let mut map = std::collections::HashMap::new();
        map.insert(("apps info".into(), "app".into()), vec!["demo".into(), "prod".into()]);
        let provider: Arc<dyn ValueProvider> = Arc::new(TestProvider { map });
        let result = SuggestionEngine::build(&reg.commands, &[provider], "apps info --app ");
        let values: Vec<_> = result.items.iter().filter(|item| matches!(item.kind, ItemKind::Value)).collect();
        assert!(!values.is_empty());
        assert!(values.iter().any(|item| item.display == "demo"));
        assert!(!result.provider_loading);
    }

    #[test]
    fn suggests_positional_with_provider() {
        let spec = CommandSpec {
            group: "addons".into(),
            name: "config:update".into(),
            summary: "update".into(),
            positional_args: vec![PositionalArgument {
                name: "addon".into(),
                help: None,
                provider: Some(heroku_types::ValueProvider::Command {
                    command_id: "addons:list".into(),
                    binds: vec![],
                }),
            }],
            flags: vec![],
            execution: CommandExecution::Http(HttpCommandSpec {
                method: "PATCH".into(),
                path: "/addons/{addon}/config".into(),
                ranges: vec![],
                // No legacy providers vector
                service_id: ServiceId::CoreApi,
                output_schema: None,
            }),
        };
        let reg = registry_with(vec![
            CommandSpec {
                group: "addons".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/addons".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
            },
            spec,
        ]);
        // Provider embedded on positional in the spec
        let mut map = std::collections::HashMap::new();
        map.insert(("addons config:update".into(), "addon".into()), vec!["redis-123".into()]);
        let provider: Arc<dyn ValueProvider> = Arc::new(TestProvider { map });
        let result = SuggestionEngine::build(&reg.commands, &[provider], "addons config:update ");
        assert!(result.items.iter().any(|item| item.display == "redis-123"));
        assert!(!result.provider_loading);
    }

    #[test]
    fn provider_loading_signal_when_binding_present_but_no_values() {
        let spec = CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![],
            flags: vec![CommandFlag {
                name: "app".into(),
                short_name: None,
                required: false,
                r#type: "string".into(),
                enum_values: vec![],
                default_value: None,
                description: None,
                provider: Some(heroku_types::ValueProvider::Command {
                    command_id: "apps:list".into(),
                    binds: vec![],
                }),
            }],
            execution: CommandExecution::Http(HttpCommandSpec {
                method: "GET".into(),
                path: "/apps/{app}".into(),
                ranges: vec![],
                service_id: ServiceId::CoreApi,
                output_schema: None,
            }),
        };
        let reg = registry_with(vec![
            CommandSpec {
                group: "apps".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/apps".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
            },
            spec,
        ]);
        // provider already embedded on flag
        let empty_provider: Arc<dyn ValueProvider> = Arc::new(TestProvider { map: Default::default() });
        let result = SuggestionEngine::build(&reg.commands, &[empty_provider], "apps info --app ");
        assert!(!result.provider_loading);
    }

    #[test]
    fn provider_loading_true_when_pending_fetch_requested() {
        let spec = CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![],
            flags: vec![CommandFlag {
                name: "app".into(),
                short_name: None,
                required: false,
                r#type: "string".into(),
                enum_values: vec![],
                default_value: None,
                description: None,
                provider: Some(heroku_types::ValueProvider::Command {
                    command_id: "apps:list".into(),
                    binds: vec![],
                }),
            }],
            execution: CommandExecution::Http(HttpCommandSpec {
                method: "GET".into(),
                path: "/apps/{app}".into(),
                ranges: vec![],
                service_id: ServiceId::CoreApi,
                output_schema: None,
            }),
        };
        let reg = registry_with(vec![
            CommandSpec {
                group: "apps".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/apps".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
            },
            spec,
        ]);
        let provider: Arc<dyn ValueProvider> = Arc::new(PendingProvider);
        let result = SuggestionEngine::build(&reg.commands, &[provider], "apps info --app ");
        assert!(result.provider_loading);
        assert!(!result.pending_fetches.is_empty());
    }

    #[test]
    fn no_duplicate_suggestion_when_positional_complete() {
        // Single positional command; provider returns exact current value only
        let spec = CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![PositionalArgument {
                name: "app".into(),
                help: None,
                provider: Some(heroku_types::ValueProvider::Command {
                    command_id: "apps:list".into(),
                    binds: vec![],
                }),
            }],
            flags: vec![],
            execution: CommandExecution::Http(HttpCommandSpec {
                method: "GET".into(),
                path: "/apps/{app}".into(),
                ranges: vec![],
                // No legacy providers vector
                service_id: ServiceId::CoreApi,
                output_schema: None,
            }),
        };
        let reg = registry_with(vec![
            CommandSpec {
                group: "apps".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/apps".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
            },
            spec,
        ]);
        let mut map = std::collections::HashMap::new();
        map.insert(("apps info".into(), "app".into()), vec!["heroku-prod".into()]);
        let provider: Arc<dyn ValueProvider> = Arc::new(TestProvider { map });
        let result = SuggestionEngine::build(&reg.commands, &[provider], "apps info heroku-prod");
        assert!(result.items.is_empty(), "should not echo current value as sole suggestion");
    }

    #[test]
    fn multi_positional_suggest_second_arg_list() {
        // Two positional args: first filters provider1, second shows provider2
        let spec = CommandSpec {
            group: "pipelines".into(),
            name: "ci:run".into(),
            summary: "run".into(),
            positional_args: vec![
                PositionalArgument {
                    name: "pipeline".into(),
                    help: None,
                    provider: Some(heroku_types::ValueProvider::Command {
                        command_id: "pipelines:list".into(),
                        binds: vec![],
                    }),
                },
                PositionalArgument {
                    name: "branch".into(),
                    help: None,
                    provider: Some(heroku_types::ValueProvider::Command {
                        command_id: "branches:list".into(),
                        binds: vec![],
                    }),
                },
            ],
            flags: vec![],
            execution: CommandExecution::Http(HttpCommandSpec {
                method: "POST".into(),
                path: "/pipelines/{pipeline}/ci".into(),
                ranges: vec![],
                // No legacy providers vector
                service_id: ServiceId::CoreApi,
                output_schema: None,
            }),
        };
        let reg = registry_with(vec![
            CommandSpec {
                group: "pipelines".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/pipelines".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
            },
            CommandSpec {
                group: "branches".into(),
                name: "list".into(),
                summary: "list".into(),
                positional_args: vec![],
                flags: vec![],
                execution: CommandExecution::Http(HttpCommandSpec {
                    method: "GET".into(),
                    path: "/branches".into(),
                    ranges: vec![],
                    service_id: ServiceId::CoreApi,
                    output_schema: None,
                }),
            },
            spec,
        ]);
        let mut map = std::collections::HashMap::new();
        map.insert(("pipelines ci:run".into(), "pipeline".into()), vec!["api".into(), "web".into()]);
        map.insert(("pipelines ci:run".into(), "branch".into()), vec!["main".into(), "develop".into()]);
        let provider: Arc<dyn ValueProvider> = Arc::new(TestProvider { map });
        // With first positional filled and trailing space, suggest second positional list
        let result = SuggestionEngine::build(&reg.commands, &[provider], "pipelines ci:run api ");
        let vals: Vec<_> = result
            .items
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Value))
            .map(|i| i.display.clone())
            .collect();
        assert!(vals.contains(&"main".into()) && vals.contains(&"develop".into()));
    }
}
