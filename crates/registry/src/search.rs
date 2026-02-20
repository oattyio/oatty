//! In-memory command search utilities.
//!
//! This module provides a deterministic, low-overhead search handle that queries the
//! current in-memory command registry directly. It replaces the previous index-backed
//! implementation and avoids background indexing threads.

use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use oatty_types::{CommandSpec, SearchResult};
use oatty_util::fuzzy_score;
use thiserror::Error;

use crate::CommandRegistry;

const DEFAULT_RESULT_LIMIT: usize = 20;
const COVERAGE_SCORE_MULTIPLIER: i64 = 20;
const EXACT_CANONICAL_MATCH_SCORE_BONUS: i64 = 50;
const PREFIX_CANONICAL_MATCH_SCORE_BONUS: i64 = 25;

/// Errors emitted by in-memory search operations.
#[derive(Debug, Error)]
pub enum SearchError {
    /// The command registry lock could not be acquired.
    #[error("registry lock failed: {0}")]
    Lock(String),
}

/// Handle for submitting command searches against the in-memory registry.
#[derive(Clone, Debug)]
pub struct SearchHandle {
    command_registry: Arc<Mutex<CommandRegistry>>,
    result_limit: usize,
}

impl SearchHandle {
    /// Creates a new search handle bound to the provided command registry.
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>) -> Self {
        Self {
            command_registry,
            result_limit: DEFAULT_RESULT_LIMIT,
        }
    }

    /// Executes a fuzzy command search and returns ranked results.
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, SearchError> {
        let trimmed_query = query.trim();
        if trimmed_query.is_empty() {
            return Ok(Vec::new());
        }

        let query_lower = trimmed_query.to_ascii_lowercase();
        let query_tokens = tokenize_query(trimmed_query);

        let registry_guard = self.command_registry.lock().map_err(|error| SearchError::Lock(error.to_string()))?;

        let mut scored_results = registry_guard
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                score_command_match(&registry_guard, command, &query_lower, &query_tokens).map(|score| {
                    (
                        score,
                        SearchResult {
                            index,
                            canonical_id: command.canonical_id(),
                            summary: command.summary.clone(),
                            execution_type: determine_execution_type(command).to_string(),
                            http_method: command.http().map(|http| http.method.clone()),
                        },
                    )
                })
            })
            .collect::<Vec<(i64, SearchResult)>>();

        scored_results.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.canonical_id.cmp(&right.1.canonical_id)));

        Ok(scored_results
            .into_iter()
            .take(self.result_limit)
            .map(|(_, result)| result)
            .collect())
    }
}

/// Creates a search handle for the provided command registry.
pub fn create_search_handle(command_registry: Arc<Mutex<CommandRegistry>>) -> SearchHandle {
    SearchHandle::new(command_registry)
}

fn score_command_match(registry: &CommandRegistry, command: &CommandSpec, query_lower: &str, query_tokens: &[String]) -> Option<i64> {
    if query_tokens.is_empty() {
        return None;
    }

    let haystack = build_command_search_haystack(registry, command);
    let fuzzy_score_value = query_tokens.iter().try_fold(0_i64, |accumulator, token| {
        fuzzy_score(&haystack, token).map(|token_score| accumulator + token_score)
    })?;
    let haystack_lower = haystack.to_ascii_lowercase();

    let coverage_score =
        query_tokens.iter().filter(|token| haystack_lower.contains(token.as_str())).count() as i64 * COVERAGE_SCORE_MULTIPLIER;

    let canonical_identifier_lower = command.canonical_id().to_ascii_lowercase();
    let exact_bonus = if canonical_identifier_lower.contains(query_lower) {
        EXACT_CANONICAL_MATCH_SCORE_BONUS
    } else {
        0
    };

    let prefix_bonus = if query_tokens
        .first()
        .map(|token| canonical_identifier_lower.starts_with(token))
        .unwrap_or(false)
    {
        PREFIX_CANONICAL_MATCH_SCORE_BONUS
    } else {
        0
    };

    Some(fuzzy_score_value + coverage_score + exact_bonus + prefix_bonus)
}

fn determine_execution_type(command: &CommandSpec) -> &'static str {
    if command.http().is_some() {
        return "http";
    }
    if command.mcp().is_some() {
        return "mcp";
    }
    "unknown"
}

fn tokenize_query(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn build_command_search_haystack(registry: &CommandRegistry, command: &CommandSpec) -> String {
    let mut haystack = String::new();
    let canonical_identifier = command.canonical_id();
    append_non_empty(&mut haystack, &canonical_identifier);
    append_non_empty(&mut haystack, &command.summary);
    let normalized_canonical_identifier = normalize_identifier(&canonical_identifier);
    append_optional(&mut haystack, Some(normalized_canonical_identifier.as_ref()));

    for positional_argument in &command.positional_args {
        append_non_empty(&mut haystack, &positional_argument.name);
        append_optional(&mut haystack, Some(normalize_identifier(&positional_argument.name).as_ref()));
        append_optional(&mut haystack, positional_argument.help.as_deref());
    }

    for flag in &command.flags {
        append_non_empty(&mut haystack, &flag.name);
        append_optional(&mut haystack, Some(normalize_identifier(&flag.name).as_ref()));
        append_optional(&mut haystack, flag.description.as_deref());
    }

    if let Some(catalogs) = registry.config.catalogs.as_ref()
        && let Some(catalog) = catalogs.get(command.catalog_identifier)
    {
        append_non_empty(&mut haystack, &catalog.title);
        append_non_empty(&mut haystack, &catalog.description);
        if let Some(manifest) = catalog.manifest.as_ref() {
            append_non_empty(&mut haystack, &manifest.vendor);
        }
    }

    haystack
}

fn append_non_empty(buffer: &mut String, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        if !buffer.is_empty() {
            buffer.push(' ');
        }
        buffer.push_str(trimmed);
    }
}

fn append_optional(buffer: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_non_empty(buffer, value);
    }
}

fn normalize_identifier(value: &'_ str) -> Cow<'_, str> {
    if value.bytes().any(|byte| matches!(byte, b'_' | b'-' | b'.')) {
        Cow::Owned(value.replace(['_', '-', '.'], " "))
    } else {
        Cow::Borrowed(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use indexmap::IndexSet;
    use oatty_types::command::HttpCommandSpec;
    use oatty_types::manifest::{RegistryCatalog, RegistryManifest};

    use crate::RegistryConfig;

    fn build_registry() -> Arc<Mutex<CommandRegistry>> {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "projects:list".to_string(),
            "List projects".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/projects", None, None),
            0,
        );

        let render_services = CommandSpec::new_http(
            "services".to_string(),
            "services:list".to_string(),
            "List services".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/services", None, None),
            1,
        );

        let vercel_manifest = RegistryManifest {
            commands: vec![vercel_projects.clone()],
            provider_contracts: Default::default(),
            vendor: "vercel".to_string(),
        };

        let render_manifest = RegistryManifest {
            commands: vec![render_services.clone()],
            provider_contracts: Default::default(),
            vendor: "render".to_string(),
        };

        let vercel_catalog = RegistryCatalog {
            title: "Vercel".to_string(),
            description: "Vercel platform API".to_string(),
            vendor: Some("vercel".to_string()),
            manifest_path: String::new(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec!["https://api.vercel.com".to_string()],
            base_url_index: 0,
            manifest: Some(vercel_manifest),
            is_enabled: true,
        };

        let render_catalog = RegistryCatalog {
            title: "Render".to_string(),
            description: "Render platform API".to_string(),
            vendor: Some("render".to_string()),
            manifest_path: String::new(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec!["https://api.render.com".to_string()],
            base_url_index: 0,
            manifest: Some(render_manifest),
            is_enabled: true,
        };

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects, render_services]);
        registry.config = RegistryConfig {
            catalogs: Some(vec![vercel_catalog, render_catalog]),
        };

        Arc::new(Mutex::new(registry))
    }

    #[tokio::test]
    async fn search_matches_vendor_terms() {
        let registry = build_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("vercel projects").await.expect("search succeeds");

        assert!(!results.is_empty(), "expected non-empty results for vendor query");
        assert_eq!(results[0].canonical_id, "projects projects:list");
    }

    #[tokio::test]
    async fn search_returns_empty_for_unmatched_query() {
        let registry = build_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("qqqqqq").await.expect("search succeeds");

        assert!(results.is_empty(), "expected no matches for unmatched query");
    }
}
