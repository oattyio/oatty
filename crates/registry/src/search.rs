//! In-memory command search utilities.
//!
//! This module provides layered command discovery over the in-memory registry.
//! Search candidates are normalized into reusable metadata documents and ranked
//! with `nucleo-matcher`, while deterministic exact/token/prefix bonuses keep
//! results stable and predictable for command-oriented queries.

use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32String};
use oatty_types::{CommandSpec, SearchResult, command::McpCommandSpec};
use thiserror::Error;

use crate::CommandRegistry;

const DEFAULT_RESULT_LIMIT: usize = 20;
const FUZZY_SCORE_MULTIPLIER: i64 = 100;
const CANONICAL_PREFIX_SCORE_BONUS: i64 = 700;
const CANONICAL_SUBSTRING_SCORE_BONUS: i64 = 250;
const GROUP_TOKEN_SCORE_BONUS: i64 = 425;
const TOKEN_MATCH_SCORE_BONUS: i64 = 225;
const SUMMARY_TOKEN_SCORE_BONUS: i64 = 100;

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
    search_index_cache: Arc<Mutex<SearchIndexCache>>,
    result_limit: usize,
}

/// Cached vendor-agnostic search index keyed by a registry fingerprint.
#[derive(Debug, Default)]
struct SearchIndexCache {
    fingerprint: Option<u64>,
    entries: Arc<[SearchIndexEntry]>,
}

/// Parsed search query reused across candidate scoring.
#[derive(Debug, Clone)]
struct ParsedSearchQuery {
    normalized_text: String,
    tokens: Vec<String>,
    pattern: Pattern,
}

/// Precomputed candidate data used for scoring and projection.
#[derive(Debug, Clone)]
struct SearchIndexEntry {
    metadata: SearchCandidateMetadata,
    result: Option<SearchResult>,
}

/// Structured search metadata derived from a command specification.
#[derive(Debug, Clone)]
struct SearchCandidateMetadata {
    catalog_identifier: Option<usize>,
    canonical_id: String,
    canonical_id_lower: String,
    normalized_canonical_id_lower: String,
    canonical_tokens: Vec<String>,
    group_tokens: Vec<String>,
    summary_tokens: Vec<String>,
    search_tokens: Vec<String>,
    search_document_utf32: Utf32String,
}

/// Lightweight matcher wrapper that owns the reusable `nucleo` scratch space.
#[derive(Debug)]
struct CommandSearchMatcher {
    matcher: Matcher,
}

impl Default for CommandSearchMatcher {
    fn default() -> Self {
        let mut configuration = MatcherConfig::DEFAULT;
        configuration.prefer_prefix = true;
        Self {
            matcher: Matcher::new(configuration),
        }
    }
}

impl SearchHandle {
    /// Creates a new search handle bound to the provided command registry.
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>) -> Self {
        Self {
            command_registry,
            search_index_cache: Arc::new(Mutex::new(SearchIndexCache::default())),
            result_limit: DEFAULT_RESULT_LIMIT,
        }
    }

    /// Executes a structured command search and returns ranked results.
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, SearchError> {
        self.search_with_vendor(query, None).await
    }

    /// Executes a structured command search scoped to a single vendor before ranking and truncation.
    pub async fn search_with_vendor(&self, query: &str, vendor: Option<&str>) -> Result<Vec<SearchResult>, SearchError> {
        let registry_guard = self.command_registry.lock().map_err(|error| SearchError::Lock(error.to_string()))?;
        let parsed_query = parse_search_query(query, vendor);
        let search_index = self.get_or_build_search_index(&registry_guard)?;
        if parsed_query.normalized_text.is_empty() {
            if vendor.is_none() {
                return Ok(Vec::new());
            }

            return Ok(list_vendor_scoped_results(
                search_index.as_ref(),
                &registry_guard,
                vendor,
                self.result_limit,
            ));
        }

        if matches!(
            registry_guard.resolve_exact_search_hit(query, vendor),
            Some(crate::ExactSearchHit::Unique(_))
        ) {
            let exact_matches = search_index
                .iter()
                .filter(|entry| entry_matches_vendor(&registry_guard, entry, vendor))
                .filter(|entry| entry_is_exact_canonical_match(entry, &parsed_query.normalized_text))
                .collect::<Vec<&SearchIndexEntry>>();
            if let [exact_match] = exact_matches.as_slice() {
                return Ok(vec![exact_match.result.clone().expect("search results include projections")]);
            }
        }

        let mut matcher = CommandSearchMatcher::default();
        let mut scored_results = search_index
            .iter()
            .filter(|entry| entry_matches_vendor(&registry_guard, entry, vendor))
            .filter_map(|entry| {
                let score = matcher.score_candidate(&parsed_query, &entry.metadata)?;
                Some((score, entry.result.clone().expect("search results include projections")))
            })
            .collect::<Vec<(i64, SearchResult)>>();

        scored_results.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.canonical_id.cmp(&right.1.canonical_id)));

        Ok(scored_results
            .into_iter()
            .take(self.result_limit)
            .map(|(_, result)| result)
            .collect())
    }

    /// Returns a cached search index for the current registry snapshot.
    fn get_or_build_search_index(&self, registry: &CommandRegistry) -> Result<Arc<[SearchIndexEntry]>, SearchError> {
        let fingerprint = compute_registry_search_fingerprint(registry);
        let mut cache = self
            .search_index_cache
            .lock()
            .map_err(|error| SearchError::Lock(error.to_string()))?;

        if cache.fingerprint != Some(fingerprint) {
            cache.entries = build_search_index(registry).into();
            cache.fingerprint = Some(fingerprint);
        }

        Ok(Arc::clone(&cache.entries))
    }
}

impl CommandSearchMatcher {
    /// Score a single candidate using fuzzy ranking plus deterministic boosts.
    fn score_candidate(&mut self, query: &ParsedSearchQuery, candidate: &SearchCandidateMetadata) -> Option<i64> {
        let fuzzy_score = query
            .pattern
            .score(candidate.search_document_utf32.slice(..), &mut self.matcher)
            .unwrap_or(0) as i64;
        let matched_search_tokens = query
            .tokens
            .iter()
            .filter(|query_token| {
                candidate
                    .search_tokens
                    .iter()
                    .any(|candidate_token| token_matches(candidate_token, query_token))
            })
            .count() as i64;
        let canonical_prefix_matches = query
            .tokens
            .iter()
            .filter(|query_token| {
                candidate.canonical_tokens.iter().any(|candidate_token| {
                    candidate_token.starts_with(query_token.as_str()) || singularize(candidate_token) == singularize(query_token)
                })
            })
            .count() as i64;
        let canonical_substring_matches = query
            .tokens
            .iter()
            .filter(|query_token| candidate.normalized_canonical_id_lower.contains(query_token.as_str()))
            .count() as i64;
        let group_matches = query
            .tokens
            .iter()
            .filter(|query_token| {
                candidate
                    .group_tokens
                    .iter()
                    .any(|group_token| token_matches(group_token, query_token))
            })
            .count() as i64;
        let summary_matches = query
            .tokens
            .iter()
            .filter(|query_token| {
                candidate
                    .summary_tokens
                    .iter()
                    .any(|summary_token| token_matches(summary_token, query_token))
            })
            .count() as i64;
        let deterministic_score = matched_search_tokens * TOKEN_MATCH_SCORE_BONUS
            + canonical_prefix_matches * CANONICAL_PREFIX_SCORE_BONUS
            + canonical_substring_matches * CANONICAL_SUBSTRING_SCORE_BONUS
            + group_matches * GROUP_TOKEN_SCORE_BONUS
            + summary_matches * SUMMARY_TOKEN_SCORE_BONUS;

        if fuzzy_score == 0 && deterministic_score == 0 {
            return None;
        }

        Some(fuzzy_score * FUZZY_SCORE_MULTIPLIER + deterministic_score)
    }
}

/// Creates a search handle for the provided command registry.
pub fn create_search_handle(command_registry: Arc<Mutex<CommandRegistry>>) -> SearchHandle {
    SearchHandle::new(command_registry)
}

/// Suggest nearest canonical command IDs for an arbitrary query string.
pub fn suggest_nearest_canonical_ids(registry: &CommandRegistry, query: &str, limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let parsed_query = parse_search_query(query, None);
    if parsed_query.normalized_text.is_empty() {
        return Vec::new();
    }

    let mut matcher = CommandSearchMatcher::default();
    let mut scored_matches = registry
        .commands
        .iter()
        .map(|command_spec| build_search_index_entry(registry, None, None, command_spec))
        .filter_map(|entry| {
            if entry.metadata.normalized_canonical_id_lower == parsed_query.normalized_text
                || entry.metadata.canonical_id_lower == parsed_query.normalized_text
            {
                return Some((i64::MAX / 4, entry.metadata.canonical_id));
            }

            let score = matcher.score_candidate(&parsed_query, &entry.metadata)?;
            Some((score, entry.metadata.canonical_id))
        })
        .collect::<Vec<(i64, String)>>();

    scored_matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    scored_matches
        .into_iter()
        .map(|(_, canonical_id)| canonical_id)
        .take(limit)
        .collect()
}

fn parse_search_query(query: &str, vendor: Option<&str>) -> ParsedSearchQuery {
    let vendor_token = vendor.map(|vendor_name| vendor_name.to_ascii_lowercase());
    let tokens = tokenize_query(query)
        .into_iter()
        .filter(|token| vendor_token.as_ref().is_none_or(|vendor_name| token != vendor_name))
        .collect::<Vec<String>>();
    let normalized_text = tokens.join(" ");
    let pattern = Pattern::new(&normalized_text, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);

    ParsedSearchQuery {
        normalized_text,
        tokens,
        pattern,
    }
}

fn build_search_index(registry: &CommandRegistry) -> Vec<SearchIndexEntry> {
    registry
        .commands
        .iter()
        .enumerate()
        .map(|(index, command)| build_search_index_entry(registry, Some(index), Some(command.summary.clone()), command))
        .collect()
}

fn build_search_index_entry(
    registry: &CommandRegistry,
    index: Option<usize>,
    summary: Option<String>,
    command: &CommandSpec,
) -> SearchIndexEntry {
    let canonical_id = command.canonical_id();
    let metadata = build_search_candidate_metadata(registry, command, &canonical_id);
    let result = index.map(|index| SearchResult {
        index,
        canonical_id,
        summary: summary.unwrap_or_default(),
        execution_type: determine_execution_type(command).to_string(),
        http_method: command.http().map(|http_spec| http_spec.method.clone()),
        vendor: registry.command_vendor(command),
    });

    SearchIndexEntry { metadata, result }
}

fn build_search_candidate_metadata(registry: &CommandRegistry, command: &CommandSpec, canonical_id: &str) -> SearchCandidateMetadata {
    let catalog_identifier = registry.resolve_catalog_identifier_for_command(command);
    let normalized_canonical_id = normalize_identifier(canonical_id).to_ascii_lowercase();
    let search_document = build_command_search_document(registry, command, canonical_id, catalog_identifier);
    let search_document_lower = search_document.to_ascii_lowercase();

    SearchCandidateMetadata {
        catalog_identifier,
        canonical_id: canonical_id.to_string(),
        canonical_id_lower: canonical_id.to_ascii_lowercase(),
        normalized_canonical_id_lower: normalized_canonical_id.clone(),
        canonical_tokens: tokenize_query(&normalized_canonical_id),
        group_tokens: tokenize_query(&command.group),
        summary_tokens: tokenize_query(&command.summary),
        search_tokens: tokenize_query(&search_document_lower),
        search_document_utf32: Utf32String::from(search_document),
    }
}

fn build_command_search_document(
    registry: &CommandRegistry,
    command: &CommandSpec,
    canonical_id: &str,
    catalog_identifier: Option<usize>,
) -> String {
    let mut search_document = String::new();
    append_normalized_value(&mut search_document, canonical_id);
    append_normalized_value(&mut search_document, &command.group);
    append_normalized_value(&mut search_document, &command.name);
    append_normalized_value(&mut search_document, &command.summary);

    for positional_argument in &command.positional_args {
        append_normalized_value(&mut search_document, &positional_argument.name);
        append_optional_normalized_value(&mut search_document, positional_argument.help.as_deref());
    }

    for flag in &command.flags {
        append_normalized_value(&mut search_document, &flag.name);
        append_optional_normalized_value(&mut search_document, flag.description.as_deref());
    }

    append_catalog_metadata(&mut search_document, registry, catalog_identifier);
    append_mcp_metadata(&mut search_document, command.mcp());

    search_document
}

fn append_catalog_metadata(search_document: &mut String, registry: &CommandRegistry, catalog_identifier: Option<usize>) {
    if let Some(catalog) = catalog_identifier.and_then(|identifier| registry.config.catalogs.as_ref()?.get(identifier)) {
        append_normalized_value(search_document, &catalog.title);
        append_normalized_value(search_document, &catalog.description);
        if let Some(vendor_name) = CommandRegistry::catalog_vendor_value(catalog) {
            append_normalized_value(search_document, vendor_name);
        }
    }
}

fn append_mcp_metadata(search_document: &mut String, mcp_command: Option<&McpCommandSpec>) {
    let Some(mcp_command) = mcp_command else {
        return;
    };

    append_normalized_value(search_document, &mcp_command.plugin_name);
    append_normalized_value(search_document, &mcp_command.tool_name);
    append_optional_normalized_value(search_document, mcp_command.auth_summary.as_deref());
    append_optional_normalized_value(search_document, mcp_command.render_hint.as_deref());
}

fn entry_is_exact_canonical_match(entry: &SearchIndexEntry, normalized_query: &str) -> bool {
    entry.metadata.canonical_id_lower == normalized_query || entry.metadata.normalized_canonical_id_lower == normalized_query
}

fn token_matches(candidate_token: &str, query_token: &str) -> bool {
    candidate_token == query_token || candidate_token.starts_with(query_token) || singularize(candidate_token) == singularize(query_token)
}

fn singularize(value: &str) -> &str {
    value.strip_suffix('s').unwrap_or(value)
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
    normalize_identifier(query)
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn append_normalized_value(buffer: &mut String, value: &str) {
    let normalized_value = normalize_identifier(value);
    let trimmed = normalized_value.trim();
    if trimmed.is_empty() {
        return;
    }

    if !buffer.is_empty() {
        buffer.push(' ');
    }
    buffer.push_str(trimmed);
}

fn append_optional_normalized_value(buffer: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_normalized_value(buffer, value);
    }
}

fn entry_matches_vendor(registry: &CommandRegistry, entry: &SearchIndexEntry, vendor: Option<&str>) -> bool {
    let Some(vendor_name) = vendor else {
        return true;
    };

    let Some(catalogs) = registry.config.catalogs.as_ref() else {
        return true;
    };

    let saw_manifest_metadata = catalogs.iter().any(|catalog| catalog.manifest.is_some());
    if !saw_manifest_metadata {
        return true;
    }

    entry
        .metadata
        .catalog_identifier
        .and_then(|catalog_identifier| registry.config.catalogs.as_ref()?.get(catalog_identifier))
        .is_some_and(|catalog| catalog.is_enabled && CommandRegistry::catalog_vendor_matches(catalog, vendor_name))
}

fn list_vendor_scoped_results(
    search_index: &[SearchIndexEntry],
    registry: &CommandRegistry,
    vendor: Option<&str>,
    result_limit: usize,
) -> Vec<SearchResult> {
    let mut results = search_index
        .iter()
        .filter(|entry| entry_matches_vendor(registry, entry, vendor))
        .filter_map(|entry| entry.result.clone())
        .collect::<Vec<SearchResult>>();

    results.sort_by(|left, right| left.canonical_id.cmp(&right.canonical_id));
    results.truncate(result_limit);
    results
}

/// Returns true when the canonical command belongs to the requested vendor.
pub fn canonical_id_matches_vendor(registry: &CommandRegistry, canonical_id: &str, vendor_name: &str) -> bool {
    registry.canonical_id_matches_vendor(canonical_id, vendor_name)
}

/// Returns true when a specific command belongs to the requested vendor.
pub fn command_matches_vendor(registry: &CommandRegistry, command: &CommandSpec, vendor_name: &str) -> bool {
    registry.command_matches_vendor(command, vendor_name)
}

fn normalize_identifier(value: &'_ str) -> Cow<'_, str> {
    if value.bytes().any(|byte| matches!(byte, b'_' | b'-' | b'.' | b':' | b'/' | b'\\')) {
        Cow::Owned(value.replace(['_', '-', '.', ':', '/', '\\'], " "))
    } else {
        Cow::Borrowed(value)
    }
}

fn compute_registry_search_fingerprint(registry: &CommandRegistry) -> u64 {
    let mut hasher = DefaultHasher::new();

    for command in &registry.commands {
        command.group.hash(&mut hasher);
        command.name.hash(&mut hasher);
        command.summary.hash(&mut hasher);
        command.catalog_identifier.hash(&mut hasher);

        for positional_argument in &command.positional_args {
            positional_argument.name.hash(&mut hasher);
            positional_argument.help.hash(&mut hasher);
        }

        for flag in &command.flags {
            flag.name.hash(&mut hasher);
            flag.short_name.hash(&mut hasher);
            flag.required.hash(&mut hasher);
            flag.r#type.hash(&mut hasher);
            flag.enum_values.hash(&mut hasher);
            flag.default_value.hash(&mut hasher);
            flag.description.hash(&mut hasher);
        }

        if let Some(http_command) = command.http() {
            http_command.method.hash(&mut hasher);
            http_command.path.hash(&mut hasher);
        }
        if let Some(mcp_command) = command.mcp() {
            mcp_command.plugin_name.hash(&mut hasher);
            mcp_command.tool_name.hash(&mut hasher);
            mcp_command.auth_summary.hash(&mut hasher);
            mcp_command.render_hint.hash(&mut hasher);
        }
    }

    if let Some(catalogs) = registry.config.catalogs.as_ref() {
        for catalog in catalogs {
            catalog.title.hash(&mut hasher);
            catalog.description.hash(&mut hasher);
            catalog.vendor.hash(&mut hasher);
            catalog.is_enabled.hash(&mut hasher);

            if let Some(manifest) = catalog.manifest.as_ref() {
                manifest.vendor.hash(&mut hasher);
                for command in &manifest.commands {
                    command.canonical_id().hash(&mut hasher);
                }
            }
        }
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    use indexmap::IndexSet;
    use oatty_types::command::HttpCommandSpec;
    use oatty_types::manifest::{RegistryCatalog, RegistryManifest};
    use oatty_types::{CommandFlag, McpCommandSpec};

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

        let vercel_catalog = build_catalog("Vercel", "vercel", vec![vercel_projects.clone()]);
        let render_catalog = build_catalog("Render", "render", vec![render_services.clone()]);

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects, render_services]);
        registry.config = RegistryConfig {
            catalogs: Some(vec![vercel_catalog, render_catalog]),
        };

        Arc::new(Mutex::new(registry))
    }

    fn build_catalog(title: &str, vendor: &str, commands: Vec<CommandSpec>) -> RegistryCatalog {
        RegistryCatalog {
            title: title.to_string(),
            description: format!("{title} platform API"),
            vendor: Some(vendor.to_string()),
            manifest_path: String::new(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec![format!("https://api.{vendor}.com")],
            base_url_index: 0,
            manifest: Some(RegistryManifest {
                commands,
                provider_contracts: Default::default(),
                vendor: vendor.to_string(),
            }),
            is_enabled: true,
        }
    }

    fn build_heterogeneous_registry() -> Arc<Mutex<CommandRegistry>> {
        let deployment_create = CommandSpec::new_http(
            "deployments".to_string(),
            "create".to_string(),
            "Create a new deployment from the current revision.".to_string(),
            Vec::new(),
            vec![CommandFlag {
                name: "git-ref".to_string(),
                short_name: None,
                description: Some("Git ref to build and deploy.".to_string()),
                r#type: "string".to_string(),
                required: false,
                default_value: None,
                enum_values: Vec::new(),
                provider: None,
            }],
            HttpCommandSpec::new("POST", "/v13/deployments", None, None),
            0,
        );
        let deployment_info = CommandSpec::new_http(
            "deployments".to_string(),
            "info".to_string(),
            "Retrieve information about a deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v13/deployments/{id}", None, None),
            0,
        );
        let domain_verify = CommandSpec::new_http(
            "projects".to_string(),
            "domains:verify".to_string(),
            "Verify project domain ownership with DNS challenge details.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v1/projects/domains/verify", None, None),
            0,
        );
        let check_rerequest = CommandSpec::new_http(
            "deployments".to_string(),
            "checks:rerequest:create".to_string(),
            "Rerequest a failed deployment check run.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v1/deployments/checks/rerequest", None, None),
            0,
        );
        let services_scale = CommandSpec::new_http(
            "services".to_string(),
            "scale".to_string(),
            "Scale a service to the requested instance count.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("PATCH", "/v1/services/{id}/scale", None, None),
            1,
        );
        let tokens_rotate = CommandSpec::new_http(
            "tokens".to_string(),
            "rotate".to_string(),
            "Rotate an access token without changing downstream clients.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v1/tokens/{id}/rotate", None, None),
            1,
        );
        let github_issue_sync = CommandSpec::new_mcp(
            "github".to_string(),
            "issues:sync".to_string(),
            "Synchronize GitHub issues into the local tracker.".to_string(),
            Vec::new(),
            Vec::new(),
            McpCommandSpec {
                plugin_name: "github".to_string(),
                tool_name: "sync_issues".to_string(),
                auth_summary: Some("GitHub OAuth required".to_string()),
                output_schema: None,
                render_hint: Some("results".to_string()),
            },
        );

        let commands = vec![
            deployment_create.clone(),
            deployment_info.clone(),
            domain_verify.clone(),
            check_rerequest.clone(),
            services_scale.clone(),
            tokens_rotate.clone(),
            github_issue_sync.clone(),
        ];

        let mut registry = CommandRegistry::default().with_commands(commands);
        registry.config = RegistryConfig {
            catalogs: Some(vec![
                build_catalog(
                    "Vercel",
                    "vercel",
                    vec![deployment_create, deployment_info, domain_verify, check_rerequest],
                ),
                build_catalog("Render", "render", vec![services_scale, tokens_rotate]),
            ]),
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

    #[tokio::test]
    async fn search_returns_empty_for_blank_query_without_vendor_scope() {
        let registry = build_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("   ").await.expect("search succeeds");

        assert!(results.is_empty(), "expected no matches for blank query");
    }

    #[test]
    fn suggest_nearest_canonical_ids_ranks_expected_match_first() {
        let registry = build_registry();
        let registry_guard = registry.lock().expect("registry lock");
        let suggestions = suggest_nearest_canonical_ids(&registry_guard, "projects project:list", 3);
        assert!(!suggestions.is_empty(), "expected non-empty suggestions");
        assert_eq!(suggestions[0], "projects projects:list");
    }

    #[tokio::test]
    async fn search_with_vendor_filters_before_result_limit() {
        let deployment_create = CommandSpec::new_http(
            "deployments".to_string(),
            "create".to_string(),
            "Create a new deployment with all the required and intended data.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v13/deployments", None, None),
            0,
        );

        let mut noise_commands = Vec::new();
        for index in 0..25 {
            noise_commands.push(CommandSpec::new_http(
                format!("deployments{index}"),
                "create".to_string(),
                "Create deployment resources and redeploy previous deployment revisions".to_string(),
                Vec::new(),
                Vec::new(),
                HttpCommandSpec::new("POST", format!("/deployments/{index}"), None, None),
                1,
            ));
        }

        let mut commands = vec![deployment_create.clone()];
        commands.extend(noise_commands.clone());

        let mut registry = CommandRegistry::default().with_commands(commands);
        registry.config = RegistryConfig {
            catalogs: Some(vec![
                build_catalog("Vercel", "vercel", vec![deployment_create]),
                build_catalog("Render", "render", noise_commands),
            ]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle
            .search_with_vendor("vercel deployment create redeploy previous deployment", Some("vercel"))
            .await
            .expect("search succeeds");

        assert!(!results.is_empty(), "expected a vendor-scoped result");
        assert_eq!(results[0].canonical_id, "deployments create");
    }

    #[tokio::test]
    async fn search_with_vendor_handles_verbose_natural_language_query() {
        let deployment_create = CommandSpec::new_http(
            "deployments".to_string(),
            "create".to_string(),
            "Create a new deployment with all the required and intended data. Additionally, a deployment id can be specified to redeploy a previous deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v13/deployments", None, None),
            0,
        );
        let deployment_check_runs_create = CommandSpec::new_http(
            "deployments".to_string(),
            "check-runs:create".to_string(),
            "Creates a new check run for a deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v1/deployments/check-runs", None, None),
            0,
        );
        let deployment_checks_rerequest_create = CommandSpec::new_http(
            "deployments".to_string(),
            "checks:rerequest:create".to_string(),
            "Rerequest a selected check that has failed.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v1/deployments/checks/rerequest", None, None),
            0,
        );
        let deployment_info = CommandSpec::new_http(
            "deployments".to_string(),
            "info".to_string(),
            "Retrieves information for a deployment either by supplying its ID or hostname.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v13/deployments/{idOrUrl}", None, None),
            0,
        );
        let files_create = CommandSpec::new_http(
            "files".to_string(),
            "create".to_string(),
            "Before you create a deployment you need to upload the required files for that deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v2/files", None, None),
            0,
        );
        let projects_promote_create = CommandSpec::new_http(
            "projects".to_string(),
            "promote:create".to_string(),
            "Allows users to promote a deployment to production. Note: This does NOT rebuild the deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v1/projects/promote", None, None),
            0,
        );

        let commands = vec![
            deployment_create.clone(),
            deployment_check_runs_create.clone(),
            deployment_checks_rerequest_create.clone(),
            deployment_info.clone(),
            files_create.clone(),
            projects_promote_create.clone(),
        ];

        let mut registry = CommandRegistry::default().with_commands(commands.clone());
        registry.config = RegistryConfig {
            catalogs: Some(vec![build_catalog("Vercel", "vercel", commands)]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle
            .search_with_vendor(
                "vercel deployment create new deployment redeploy clone existing deployment build from git ref",
                Some("vercel"),
            )
            .await
            .expect("search succeeds");

        assert!(!results.is_empty(), "expected a vendor-scoped result");
        assert_eq!(results[0].canonical_id, "deployments create");
    }

    #[tokio::test]
    async fn search_exact_canonical_hit_short_circuits() {
        let registry = build_heterogeneous_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("deployments checks:rerequest:create").await.expect("search succeeds");

        assert_eq!(results.len(), 1, "expected direct canonical match only");
        assert_eq!(results[0].canonical_id, "deployments checks:rerequest:create");
    }

    #[tokio::test]
    async fn search_ranks_nested_command_names_from_spaced_query() {
        let registry = build_heterogeneous_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("project domain verify").await.expect("search succeeds");

        assert!(!results.is_empty(), "expected nested command match");
        assert_eq!(results[0].canonical_id, "projects domains:verify");
    }

    #[tokio::test]
    async fn search_treats_deployment_as_a_resource_hint() {
        let deployment_create = CommandSpec::new_http(
            "deployments".to_string(),
            "create".to_string(),
            "Create a deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v13/deployments", None, None),
            0,
        );
        let deployment_delete = CommandSpec::new_http(
            "deployments".to_string(),
            "delete".to_string(),
            "Delete a deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("DELETE", "/v13/deployments/{id}", None, None),
            0,
        );
        let deployment_info = CommandSpec::new_http(
            "deployments".to_string(),
            "info".to_string(),
            "Retrieve deployment details.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v13/deployments/{id}", None, None),
            0,
        );

        let commands = vec![deployment_create, deployment_delete, deployment_info];
        let mut registry = CommandRegistry::default().with_commands(commands.clone());
        registry.config = RegistryConfig {
            catalogs: Some(vec![build_catalog("Vercel", "vercel", commands)]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle.search("deployment delete").await.expect("search succeeds");

        assert!(!results.is_empty(), "expected search results");
        assert_eq!(results[0].canonical_id, "deployments delete");
    }

    #[tokio::test]
    async fn search_matches_provider_specific_verbs_without_custom_aliases() {
        let registry = build_heterogeneous_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("token rotate").await.expect("search succeeds");

        assert!(!results.is_empty(), "expected token rotation result");
        assert_eq!(results[0].canonical_id, "tokens rotate");
    }

    #[tokio::test]
    async fn search_can_rank_mcp_and_http_commands_together() {
        let registry = build_heterogeneous_registry();
        let handle = SearchHandle::new(registry);

        let results = handle.search("github issues sync").await.expect("search succeeds");

        assert!(!results.is_empty(), "expected mixed execution result");
        assert_eq!(results[0].canonical_id, "github issues:sync");
        assert_eq!(results[0].execution_type, "mcp");
    }

    #[tokio::test]
    async fn search_with_vendor_matches_commands_when_catalog_identifier_is_stale() {
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/projects", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![render_projects.clone()]);
        registry.config = RegistryConfig {
            catalogs: Some(vec![build_catalog("Render", "render", vec![render_projects])]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle
            .search_with_vendor("render projects", Some("render"))
            .await
            .expect("search succeeds");

        assert!(!results.is_empty(), "expected vendor-scoped search results");
        assert_eq!(results[0].canonical_id, "projects list");
    }

    #[tokio::test]
    async fn search_with_vendor_returns_scoped_results_for_vendor_only_query() {
        let deployment_create = CommandSpec::new_http(
            "deployments".to_string(),
            "create".to_string(),
            "Create a Vercel deployment.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("POST", "/v13/deployments", None, None),
            0,
        );
        let project_list = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let service_scale = CommandSpec::new_http(
            "services".to_string(),
            "scale".to_string(),
            "Scale a Render service.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("PATCH", "/v1/services/{id}/scale", None, None),
            1,
        );

        let commands = vec![deployment_create.clone(), project_list.clone(), service_scale.clone()];
        let mut registry = CommandRegistry::default().with_commands(commands);
        registry.config = RegistryConfig {
            catalogs: Some(vec![
                build_catalog("Vercel", "vercel", vec![deployment_create, project_list]),
                build_catalog("Render", "render", vec![service_scale]),
            ]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle.search_with_vendor("vercel", Some("vercel")).await.expect("search succeeds");

        assert_eq!(results.len(), 2, "expected only vendor-scoped results");
        assert_eq!(results[0].canonical_id, "deployments create");
        assert_eq!(results[1].canonical_id, "projects list");
    }

    #[tokio::test]
    async fn search_with_vendor_distinguishes_duplicate_canonical_ids_by_catalog_identity() {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/services", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects.clone(), render_projects.clone()]);
        registry.config = RegistryConfig {
            catalogs: Some(vec![
                build_catalog("Vercel", "vercel", vec![vercel_projects]),
                build_catalog("Render", "render", vec![render_projects]),
            ]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle
            .search_with_vendor("render projects", Some("render"))
            .await
            .expect("search succeeds");

        assert_eq!(results.len(), 1, "expected only the requested vendor command");
        assert_eq!(results[0].canonical_id, "projects list");
        assert_eq!(results[0].summary, "List Render projects.");
    }

    #[tokio::test]
    async fn search_exact_duplicate_canonical_id_returns_all_matching_vendors() {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects.".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/services", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects.clone(), render_projects.clone()]);
        registry.config = RegistryConfig {
            catalogs: Some(vec![
                build_catalog("Vercel", "vercel", vec![vercel_projects]),
                build_catalog("Render", "render", vec![render_projects]),
            ]),
        };

        let handle = SearchHandle::new(Arc::new(Mutex::new(registry)));
        let results = handle.search("projects list").await.expect("search succeeds");

        assert_eq!(results.len(), 2, "duplicate canonical ids should remain visible");
        assert_eq!(results[0].canonical_id, "projects list");
        assert_eq!(results[1].canonical_id, "projects list");
        assert_eq!(results[0].vendor.as_deref(), Some("vercel"));
        assert_eq!(results[1].vendor.as_deref(), Some("render"));
    }

    #[tokio::test]
    async fn search_keeps_relevant_commands_ahead_of_noise() {
        let registry = build_heterogeneous_registry();
        {
            let mut registry_guard = registry.lock().expect("registry lock");
            for index in 0..40 {
                registry_guard.commands.push(CommandSpec::new_http(
                    format!("misc{index}"),
                    "list".to_string(),
                    "Enumerate unrelated maintenance records.".to_string(),
                    Vec::new(),
                    Vec::new(),
                    HttpCommandSpec::new("GET", format!("/v1/misc/{index}"), None, None),
                    0,
                ));
            }
        }

        let handle = SearchHandle::new(registry);
        let results = handle.search("service scale").await.expect("search succeeds");

        assert!(!results.is_empty(), "expected scaled service result");
        assert_eq!(results[0].canonical_id, "services scale");
    }

    #[test]
    fn normalize_identifier_splits_colon_and_slash_separators() {
        assert_eq!(normalize_identifier("deployments:create/v1").as_ref(), "deployments create v1");
    }
}
