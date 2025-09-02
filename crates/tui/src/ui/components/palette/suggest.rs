use heroku_registry::Registry;
use heroku_types::CommandSpec;
use heroku_util::{fuzzy_score, lex_shell_like, lex_shell_like_ranged};

use super::state::{ItemKind, SuggestionItem, ValueProvider};

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
}

/// Engine responsible for building command suggestions based on user input and available commands.
///
/// The suggestion engine analyzes user input tokens and generates contextually relevant
/// suggestions including commands, flags, positional arguments, and values from providers.
pub(crate) struct SuggestionEngine;

impl SuggestionEngine {
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
    pub fn build(
        registry: &Registry,
        providers: &[Box<dyn ValueProvider>],
        input: &str,
    ) -> SuggestionResult {
        let input_tokens: Vec<String> = lex_shell_like(input);

        // Suggest commands until a valid command is resolved
        if !is_command_resolved(registry, &input_tokens) {
            let items = suggest_commands(registry, &compute_command_prefix(&input_tokens));
            return SuggestionResult { items, provider_loading: false };
        }

        // Resolve command specification
        let group = input_tokens.first().unwrap_or(&String::new()).to_owned();
        let name = input_tokens.get(1).unwrap_or(&String::new()).to_owned();
        let Some(spec) = registry.commands.iter().find(|command| command.group == group && command.name == name).cloned() else {
            return SuggestionResult { items: vec![], provider_loading: false };
        };

        // Extract remaining parts after command
        let remaining_parts: &[String] = if input_tokens.len() >= 2 { &input_tokens[2..] } else { &input_tokens[0..0] };
        let (user_flags, user_args) = parse_user_flags_args(&spec, remaining_parts);
        let current_input = remaining_parts.last().map(|s| s.as_str()).unwrap_or("");
        let ends_with_space = input.ends_with(' ') || input.ends_with('\t') || input.ends_with('\n') || input.ends_with('\r');
        let current_is_flag = current_input.starts_with('-');
        let pending_flag = find_pending_flag(&spec, remaining_parts, input);

        let mut provider_loading = false;

        // If a non-boolean flag value is pending, only suggest values for it
        if let Some(flag_name) = pending_flag.clone() {
            let value_partial = flag_value_partial(remaining_parts);
            let mut items = suggest_values_for_flag(&spec, &flag_name, &value_partial, providers);
            
            // Check if provider binding exists but no provider items yet, signal loading
            let has_binding = spec
                .providers
                .iter()
                .any(|p| matches!(p.kind, heroku_types::ProviderParamKind::Flag) && p.name == flag_name);
            let provider_found = items
                .iter()
                .any(|it| matches!(it.kind, ItemKind::Value) && it.meta.as_deref() != Some("enum"));
            if has_binding && !provider_found {
                provider_loading = true;
            }
            return SuggestionResult { items, provider_loading };
        }

        // Positionals:
        // - If the user is currently typing a positional token (no trailing space),
        //   suggest values for that positional index.
        // - Otherwise, suggest for the next positional if any remain.
        let mut items: Vec<SuggestionItem> = {
            let first_flag_idx = remaining_parts
                .iter()
                .position(|t| t.starts_with("--"))
                .unwrap_or(remaining_parts.len());
            let editing_positional = !current_is_flag
                && !remaining_parts.is_empty()
                && (remaining_parts.len() - 1) < first_flag_idx
                && !spec.positional_args.is_empty();
            let editing_positional = editing_positional && !ends_with_space;
            if editing_positional {
                // The positional index under edit is the index of the last non-flag token
                let arg_index = (remaining_parts.len() - 1)
                    .min(spec.positional_args.len().saturating_sub(1));
                let mut values = suggest_positionals(&spec, arg_index, current_input, providers);
                // Suppress echoing the exact same value back; retain only different values
                values.retain(|item| item.insert_text != current_input);
                if let Some(positional_arg) = spec.positional_args.get(arg_index) {
                    let has_binding = spec
                        .providers
                        .iter()
                        .any(|provider| matches!(provider.kind, heroku_types::ProviderParamKind::Positional)
                            && provider.name == positional_arg.name);
                    let provider_found = values
                        .iter()
                        .any(|item| matches!(item.kind, ItemKind::Value) && item.meta.as_deref() != Some("enum"));
                    if has_binding && !provider_found {
                        provider_loading = true;
                    }
                }
                values
            } else if user_args.len() < spec.positional_args.len() && !current_is_flag {
                // Suggest for the next positional (no partial yet)
                let mut values = suggest_positionals(&spec, user_args.len(), "", providers);
                // For the next positional, if the current token is empty, keep all; otherwise
                // suppress identical echo (should not happen here since current is for last token).
                if !current_input.is_empty() {
                    values.retain(|item| item.insert_text != current_input);
                }
                if let Some(positional_arg) = spec.positional_args.get(user_args.len()) {
                    let has_binding = spec
                        .providers
                        .iter()
                        .any(|provider| matches!(provider.kind, heroku_types::ProviderParamKind::Positional)
                            && provider.name == positional_arg.name);
                    let provider_found = values
                        .iter()
                        .any(|item| matches!(item.kind, ItemKind::Value) && item.meta.as_deref() != Some("enum"));
                    if has_binding && !provider_found {
                        provider_loading = true;
                    }
                }
                values
            } else {
                Vec::new()
            }
        };

        // Suggest required flags if needed (or if user is typing a flag)
        if items.is_empty() {
            let required_remaining = required_flags_remaining(&spec, &user_flags);
            if required_remaining || current_is_flag {
                items.extend(collect_flag_candidates(&spec, &user_flags, current_input, true));
            }
        }

        // Suggest optional flags when required flags are satisfied
        if items.is_empty() {
            items.extend(collect_flag_candidates(&spec, &user_flags, current_input, false));
        }

        SuggestionResult { items, provider_loading }
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
fn is_command_resolved(registry: &Registry, tokens: &[String]) -> bool {
    if tokens.len() < 2 {
        return false;
    }
    let (group, name) = (&tokens[0], &tokens[1]);
    registry.commands.iter().any(|c| &c.group == group && &c.name == name)
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
fn suggest_commands(registry: &Registry, prefix: &str) -> Vec<SuggestionItem> {
    let mut items = Vec::new();
    if prefix.is_empty() {
        return items;
    }
    
    for command in &*registry.commands {
        let group = &command.group;
        let name = &command.name;
        let executable = if name.is_empty() { group.to_string() } else { format!("{} {}", group, name) };
        
        if let Some(score) = fuzzy_score(&executable, prefix) {
            items.push(SuggestionItem {
                display: format!("{:<28} [CMD] {}", executable, command.summary),
                insert_text: executable,
                kind: ItemKind::Command,
                meta: None,
                score,
            });
        }
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
fn parse_user_flags_args(spec: &CommandSpec, parts: &[String]) -> (Vec<String>, Vec<String>) {
    let mut user_flags: Vec<String> = Vec::new();
    let mut user_args: Vec<String> = Vec::new();
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
            }
        } else if token.contains('=') && token.starts_with("--") {
            let name = token.split('=').next().unwrap_or("").trim_start_matches('-');
            user_flags.push(name.to_string());
        } else {
            user_args.push(token.to_string());
        }
        i += 1;
    }
    
    (user_flags, user_args)
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
            {
                if ((j as usize) == parts.len() - 1 || parts[(j as usize) + 1].starts_with('-'))
                    && !is_flag_value_complete(input)
                {
                    return Some(name.to_string());
                }
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
fn suggest_values_for_flag(
    spec: &CommandSpec,
    flag_name: &str,
    partial: &str,
    providers: &[Box<dyn ValueProvider>],
) -> Vec<SuggestionItem> {
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
    let command_key = format!("{}:{}", spec.group, spec.name);
    for provider in providers {
        let mut values = provider.suggest(&command_key, flag_name, partial);
        items.append(&mut values);
    }
    
    items
}

/// Generates suggestions for positional arguments.
///
/// Queries value providers for suggestions and falls back to a generic
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
    spec: &CommandSpec,
    arg_count: usize,
    current: &str,
    providers: &[Box<dyn ValueProvider>],
) -> Vec<SuggestionItem> {
    let mut items: Vec<SuggestionItem> = Vec::new();
    
    if let Some(positional_arg) = spec.positional_args.get(arg_count) {
        let command_key = format!("{}:{}", spec.group, spec.name);
        
        // Query providers for dynamic suggestions
        for provider in providers {
            let mut values = provider.suggest(&command_key, &positional_arg.name, current);
            items.append(&mut values);
        }
        
        // Fall back to generic placeholder if no provider suggestions
        if items.is_empty() {
            items.push(SuggestionItem {
                display: format!("<{}> [ARG] {}", positional_arg.name, positional_arg.help.as_deref().unwrap_or(&positional_arg.name)),
                insert_text: format!("<{}>", positional_arg.name),
                kind: ItemKind::Positional,
                meta: positional_arg.help.clone(),
                score: 0,
            });
        }
    }
    
    items
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
fn required_flags_remaining(spec: &CommandSpec, user_flags: &[String]) -> bool {
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
fn collect_flag_candidates(
    spec: &CommandSpec,
    user_flags: &[String],
    current: &str,
    required_only: bool,
) -> Vec<SuggestionItem> {
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
fn is_flag_value_complete(input: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use heroku_types::{CommandFlag, PositionalArgument, ProviderBinding, ProviderConfidence, ProviderParamKind};

    #[derive(Debug)]
    struct TestProvider {
        map: std::collections::HashMap<(String, String), Vec<String>>, // ((command_key, field) -> values)
    }

    impl ValueProvider for TestProvider {
        fn suggest(&self, command_key: &str, field: &str, _partial: &str) -> Vec<SuggestionItem> {
            let mut out = Vec::new();
            if let Some(values) = self.map.get(&(command_key.to_string(), field.to_string())) {
                for v in values {
                    out.push(SuggestionItem {
                        display: v.clone(),
                        insert_text: v.clone(),
                        kind: ItemKind::Value,
                        meta: Some("test-provider".into()),
                        score: 1,
                    });
                }
            }
            out
        }
    }

    fn registry_with(commands: Vec<heroku_types::CommandSpec>) -> Registry {
        heroku_registry::Registry { commands: Arc::from(commands.into_boxed_slice()) }
    }

    #[test]
    fn suggests_commands_before_resolution() {
        let reg = registry_with(vec![
            heroku_types::CommandSpec { group: "apps".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/apps".into(), ranges: vec![], providers: vec![] },
            heroku_types::CommandSpec { group: "apps".into(), name: "info".into(), summary: "info".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/apps/{app}".into(), ranges: vec![], providers: vec![] },
        ]);
        let result = SuggestionEngine::build(&reg, &[], "ap");
        assert!(!result.items.is_empty());
        assert!(result.items.iter().any(|item| matches!(item.kind, ItemKind::Command)));
        assert!(!result.provider_loading);
    }

    #[test]
    fn suggests_flag_values_enum_and_provider() {
        // apps:info with --region enum and --app provider
        let spec = heroku_types::CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![],
            flags: vec![
                CommandFlag { name: "region".into(), short_name: None, required: false, r#type: "enum".into(), enum_values: vec!["us".into(), "eu".into()], default_value: None, description: None },
                CommandFlag { name: "app".into(), short_name: None, required: false, r#type: "string".into(), enum_values: vec![], default_value: None, description: None },
            ],
            method: "GET".into(),
            path: "/apps/{app}".into(),
            ranges: vec![],
            providers: vec![ProviderBinding { kind: ProviderParamKind::Flag, name: "app".into(), provider_id: "apps:list".into(), confidence: ProviderConfidence::High }],
        };
        let reg = registry_with(vec![
            heroku_types::CommandSpec { group: "apps".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/apps".into(), ranges: vec![], providers: vec![] },
            spec,
        ]);
        let mut map = std::collections::HashMap::new();
        map.insert(("apps:info".into(), "app".into()), vec!["demo".into(), "prod".into()]);
        let provider: Box<dyn ValueProvider> = Box::new(TestProvider { map });
        let result = SuggestionEngine::build(&reg, &[provider], "apps info --app ");
        let values: Vec<_> = result.items.iter().filter(|item| matches!(item.kind, ItemKind::Value)).collect();
        assert!(!values.is_empty());
        assert!(values.iter().any(|item| item.display == "demo"));
        assert!(!result.provider_loading);
    }

    #[test]
    fn suggests_positional_with_provider() {
        let spec = heroku_types::CommandSpec {
            group: "addons".into(),
            name: "config:update".into(),
            summary: "update".into(),
            positional_args: vec![PositionalArgument { name: "addon".into(), help: None }],
            flags: vec![],
            method: "PATCH".into(),
            path: "/addons/{addon}/config".into(),
            ranges: vec![],
            providers: vec![ProviderBinding { kind: ProviderParamKind::Positional, name: "addon".into(), provider_id: "addons:list".into(), confidence: ProviderConfidence::High }],
        };
        let reg = registry_with(vec![
            heroku_types::CommandSpec { group: "addons".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/addons".into(), ranges: vec![], providers: vec![] },
            spec,
        ]);
        let mut map = std::collections::HashMap::new();
        map.insert(("addons:config:update".into(), "addon".into()), vec!["redis-123".into()]);
        let provider: Box<dyn ValueProvider> = Box::new(TestProvider { map });
        let result = SuggestionEngine::build(&reg, &[provider], "addons config:update ");
        assert!(result.items.iter().any(|item| item.display == "redis-123"));
        assert!(!result.provider_loading);
    }

    #[test]
    fn provider_loading_signal_when_binding_present_but_no_values() {
        let spec = heroku_types::CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![],
            flags: vec![CommandFlag { name: "app".into(), short_name: None, required: false, r#type: "string".into(), enum_values: vec![], default_value: None, description: None }],
            method: "GET".into(),
            path: "/apps/{app}".into(),
            ranges: vec![],
            providers: vec![ProviderBinding { kind: ProviderParamKind::Flag, name: "app".into(), provider_id: "apps:list".into(), confidence: ProviderConfidence::High }],
        };
        let reg = registry_with(vec![
            heroku_types::CommandSpec { group: "apps".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/apps".into(), ranges: vec![], providers: vec![] },
            spec,
        ]);
        let empty_provider: Box<dyn ValueProvider> = Box::new(TestProvider { map: Default::default() });
        let result = SuggestionEngine::build(&reg, &[empty_provider], "apps info --app ");
        assert!(result.provider_loading);
    }

    #[test]
    fn no_duplicate_suggestion_when_positional_complete() {
        // Single positional command; provider returns exact current value only
        let spec = heroku_types::CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![PositionalArgument { name: "app".into(), help: None }],
            flags: vec![],
            method: "GET".into(),
            path: "/apps/{app}".into(),
            ranges: vec![],
            providers: vec![ProviderBinding { kind: ProviderParamKind::Positional, name: "app".into(), provider_id: "apps:list".into(), confidence: ProviderConfidence::High }],
        };
        let reg = registry_with(vec![
            heroku_types::CommandSpec { group: "apps".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/apps".into(), ranges: vec![], providers: vec![] },
            spec,
        ]);
        let mut map = std::collections::HashMap::new();
        map.insert(("apps:info".into(), "app".into()), vec!["heroku-prod".into()]);
        let provider: Box<dyn ValueProvider> = Box::new(TestProvider { map });
        let result = SuggestionEngine::build(&reg, &[provider], "apps info heroku-prod");
        assert!(result.items.is_empty(), "should not echo current value as sole suggestion");
    }

    #[test]
    fn multi_positional_suggest_second_arg_list() {
        // Two positional args: first filters provider1, second shows provider2
        let spec = heroku_types::CommandSpec {
            group: "pipelines".into(),
            name: "ci:run".into(),
            summary: "run".into(),
            positional_args: vec![
                PositionalArgument { name: "pipeline".into(), help: None },
                PositionalArgument { name: "branch".into(), help: None },
            ],
            flags: vec![],
            method: "POST".into(),
            path: "/pipelines/{pipeline}/ci".into(),
            ranges: vec![],
            providers: vec![
                ProviderBinding { kind: ProviderParamKind::Positional, name: "pipeline".into(), provider_id: "pipelines:list".into(), confidence: ProviderConfidence::High },
                ProviderBinding { kind: ProviderParamKind::Positional, name: "branch".into(), provider_id: "branches:list".into(), confidence: ProviderConfidence::High },
            ],
        };
        let reg = registry_with(vec![
            heroku_types::CommandSpec { group: "pipelines".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/pipelines".into(), ranges: vec![], providers: vec![] },
            heroku_types::CommandSpec { group: "branches".into(), name: "list".into(), summary: "list".into(), positional_args: vec![], flags: vec![], method: "GET".into(), path: "/branches".into(), ranges: vec![], providers: vec![] },
            spec,
        ]);
        let mut map = std::collections::HashMap::new();
        map.insert(("pipelines:ci:run".into(), "pipeline".into()), vec!["api".into(), "web".into()]);
        map.insert(("pipelines:ci:run".into(), "branch".into()), vec!["main".into(), "develop".into()]);
        let provider: Box<dyn ValueProvider> = Box::new(TestProvider { map });
        // With first positional filled and trailing space, suggest second positional list
        let result = SuggestionEngine::build(&reg, &[provider], "pipelines ci:run api ");
        let vals: Vec<_> = result.items.iter().filter(|i| matches!(i.kind, ItemKind::Value)).map(|i| i.display.clone()).collect();
        assert!(vals.contains(&"main".into()) && vals.contains(&"develop".into()));
    }
}
