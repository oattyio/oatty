use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Detail level for including command input metadata in search results.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchInputsDetail {
    /// Return minimal discovery metadata (`canonical_id`, `execution_type`, `http_method`).
    #[default]
    None,
    /// Return only required input fields for minimal-token execution planning.
    RequiredOnly,
    /// Return full positional and flag schemas.
    Full,
}

/// Detail level for including output schema payloads in command detail responses.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputSchemaDetail {
    /// Return compact output field paths only (token-efficient default).
    #[default]
    Paths,
    /// Return the full output schema object (with sparse-field pruning applied).
    Full,
}

/// Detail level for including value-provider metadata in command detail responses.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMetadataDetail {
    /// Omit provider metadata to keep responses token-efficient.
    #[default]
    None,
    /// Include provider source command identifiers for required inputs.
    RequiredOnly,
    /// Include provider source identifiers and binding metadata for all provider-backed inputs.
    Full,
}

/// Parameters for command discovery.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchRequestParam {
    /// Free-text query used for full-text command search.
    #[schemars(description = "Free-text query for command/tool discovery.")]
    pub query: String,
    /// Optional vendor filter. When provided, only commands from the matching vendor are returned.
    #[schemars(description = "Optional vendor filter to limit results to one provider.")]
    pub vendor: Option<String>,
    /// Maximum number of results to return.
    ///
    /// Use smaller limits (for example 5-10) to reduce token usage when the
    /// model only needs top candidates.
    #[schemars(description = "Optional max results cap. Use small values (5-10) to reduce token usage.")]
    pub limit: Option<usize>,
    /// Include command input metadata for each search result.
    ///
    /// Use `required_only` for low-token execution planning and `full` when a
    /// complete flag/argument schema is needed.
    #[schemars(description = "Optional input metadata level: none|required_only|full.")]
    pub include_inputs: Option<SearchInputsDetail>,
}

/// Parameters for command execution tools.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RunCommandRequestParam {
    /// Canonical command identifier in `<group> <command>` format.
    #[schemars(description = "Canonical command id in '<group> <command>' format, for example: 'apps apps:list'.")]
    pub canonical_id: String,
    /// Ordered positional argument values as required by the command specification.
    #[schemars(description = "Ordered positional argument values. Use command metadata order exactly.")]
    pub positional_args: Option<Vec<String>>,
    /// Named flag/value pairs.
    ///
    /// Boolean flags honor explicit true/false values when provided.
    #[schemars(
        description = "Named flag/value pairs as [name, value]. Value may be string/number/boolean/object/array. Boolean flags accept explicit true/false values."
    )]
    pub named_flags: Option<Vec<(String, Value)>>,
}

/// Parameters for catalog-level summary lookups.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandSummariesRequest {
    /// Human-readable catalog title as returned by catalog listing tools.
    #[schemars(description = "Catalog title to inspect. Use list_command_topics output values.")]
    pub catalog_title: String,
}

/// Parameters for exact command lookup by canonical identifier.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandDetailRequest {
    /// Canonical command identifier in `<group> <command>` format.
    #[schemars(description = "Canonical command id in '<group> <command>' format, for example: 'apps apps:list'.")]
    pub canonical_id: String,
    /// Optional output schema detail level. Defaults to `paths`.
    #[schemars(description = "Optional output schema detail: paths|full. Default is paths (output_fields only).")]
    pub output_schema_detail: Option<OutputSchemaDetail>,
    /// Optional provider metadata detail level. Defaults to `none`.
    #[schemars(description = "Optional provider metadata detail: none|required_only|full. Default is none.")]
    pub include_providers: Option<ProviderMetadataDetail>,
}

/// How to interpret the catalog source location.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSourceType {
    /// Source is a local filesystem path.
    Path,
    /// Source is a remote HTTP(S) URL.
    Url,
}

/// Request payload for validating an OpenAPI source without importing it.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogValidateOpenApiRequest {
    /// Source location (path or URL).
    #[schemars(description = "OpenAPI source location (local path or HTTP(S) URL).")]
    pub source: String,
    /// Optional source type hint.
    #[schemars(description = "Optional source type hint: path or url.")]
    pub source_type: Option<CatalogSourceType>,
}

/// Request payload for previewing an OpenAPI catalog import.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogPreviewImportRequest {
    /// Source location (path or URL).
    #[schemars(description = "OpenAPI source location (local path or HTTP(S) URL).")]
    pub source: String,
    /// Optional source type hint.
    #[schemars(description = "Optional source type hint: path or url.")]
    pub source_type: Option<CatalogSourceType>,
    /// Target catalog title.
    #[schemars(description = "Catalog title to create or update.")]
    pub catalog_title: String,
    /// Optional vendor override used for generated command group prefix.
    #[schemars(description = "Optional vendor/prefix override for generated command IDs.")]
    pub vendor: Option<String>,
    /// Optional base URL override for the resulting catalog.
    #[schemars(description = "Optional base URL override for the resulting catalog.")]
    pub base_url: Option<String>,
    /// Include command preview list in response.
    #[schemars(description = "When true, include a token-capped command preview list.")]
    pub include_command_preview: Option<bool>,
}

/// Request payload for importing an OpenAPI catalog into runtime config.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogImportOpenApiRequest {
    /// Source location (path or URL).
    #[schemars(description = "OpenAPI source location (local path or HTTP(S) URL).")]
    pub source: String,
    /// Optional source type hint.
    #[schemars(description = "Optional source type hint: path or url.")]
    pub source_type: Option<CatalogSourceType>,
    /// Target catalog title.
    #[schemars(description = "Catalog title to create or update.")]
    pub catalog_title: String,
    /// Optional vendor override used for generated command group prefix.
    #[schemars(description = "Optional vendor/prefix override for generated command IDs.")]
    pub vendor: Option<String>,
    /// Optional base URL override for the resulting catalog.
    #[schemars(description = "Optional base URL override for the resulting catalog.")]
    pub base_url: Option<String>,
    /// Whether to overwrite an existing catalog with the same title.
    #[schemars(description = "Overwrite existing catalog with the same title when true.")]
    pub overwrite: Option<bool>,
    /// Whether the imported catalog should be enabled immediately.
    #[schemars(description = "Enable imported catalog immediately. Defaults to true.")]
    pub enabled: Option<bool>,
}

/// Request payload for applying deterministic command replacements to an existing catalog.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CatalogApplyPatchRequest {
    /// Existing catalog title to patch.
    #[schemars(description = "Existing catalog title to patch.")]
    pub catalog_id: String,
    /// Ordered patch operations.
    #[schemars(description = "Ordered patch operations to apply.")]
    pub operations: Vec<CatalogPatchOperationInput>,
    /// Fail when a target command is missing. Defaults to true.
    #[schemars(description = "Fail when a target command is missing. Defaults to true.")]
    pub fail_on_missing: Option<bool>,
    /// Fail when matching is ambiguous. Defaults to true.
    #[schemars(description = "Fail when command matching is ambiguous. Defaults to true.")]
    pub fail_on_ambiguous: Option<bool>,
    /// Persist by replacing the existing catalog entry. Defaults to true.
    #[schemars(description = "Persist patched catalog by replacing the existing catalog entry. Defaults to true.")]
    pub overwrite: Option<bool>,
}

/// Single command replacement operation in a patch request.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CatalogPatchOperationInput {
    /// Optional operation identifier for diagnostics.
    #[schemars(description = "Optional operation identifier for diagnostics.")]
    pub operation_id: Option<String>,
    /// Strict target command match key.
    #[schemars(description = "Strict command identity key used to select the replacement target.")]
    pub match_command: CatalogCommandMatchKeyInput,
    /// Full replacement command specification payload.
    #[schemars(description = "Full replacement command specification payload.")]
    pub replacement_command: Value,
}

/// Match key for targeting an existing command.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogCommandMatchKeyInput {
    /// Command group (for example `apps`).
    pub group: String,
    /// Command name (for example `apps:list`).
    pub name: String,
    /// HTTP method (for example `GET`).
    pub http_method: String,
    /// HTTP path (for example `/v1/apps`).
    pub http_path: String,
}

/// Request payload for enabling or disabling an existing catalog.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogSetEnabledRequest {
    /// Catalog identifier/title.
    #[schemars(description = "Catalog identifier or title.")]
    pub catalog_id: String,
    /// Desired enabled state.
    #[schemars(description = "Set catalog enabled state.")]
    pub enabled: bool,
}

/// Request payload for updating the selected base URL of an existing catalog.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogSetBaseUrlRequest {
    /// Catalog title from `list_command_topics`.
    #[schemars(description = "Catalog title from list_command_topics.")]
    pub catalog_id: String,
    /// Base URL to upsert/select for the catalog.
    #[schemars(description = "Base URL to set as selected for this catalog.")]
    pub base_url: String,
}

/// Header edit mode for catalog header mutations.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum CatalogHeaderEditMode {
    /// Insert new headers and replace existing keys.
    #[default]
    Upsert,
    /// Remove matching keys from existing headers.
    Remove,
    /// Replace all existing headers with provided entries.
    ReplaceAll,
}

/// Source hint for edited catalog headers.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum CatalogHeaderSource {
    /// Header originates from config file storage.
    #[default]
    File,
    /// Header value should be treated as secret-backed.
    Secret,
    /// Header originates from process environment materialization.
    Env,
    /// Header originates from raw literal input.
    Raw,
}

/// Header edit row used by `catalog.edit_headers`.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogHeaderEditRow {
    /// Header name.
    #[schemars(description = "Header name.")]
    pub key: String,
    /// Header value used for `upsert` and `replace_all`; ignored for `remove`.
    #[schemars(description = "Header value. Required for upsert/replace_all; ignored for remove.")]
    pub value: Option<String>,
    /// Optional source hint for the header.
    #[schemars(description = "Optional source hint: file|secret|env|raw. Defaults to raw.")]
    pub source: Option<CatalogHeaderSource>,
    /// Optional effective flag override.
    #[schemars(description = "Optional effective flag for the header value.")]
    pub effective: Option<bool>,
}

/// Request payload for mutating catalog headers.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogEditHeadersRequest {
    /// Catalog title from `list_command_topics`.
    #[schemars(description = "Catalog title from list_command_topics.")]
    pub catalog_id: String,
    /// Header mutation mode.
    #[schemars(description = "Header edit mode: upsert|remove|replace_all. Defaults to upsert.")]
    pub mode: Option<CatalogHeaderEditMode>,
    /// Header rows to apply.
    #[schemars(description = "Header rows to apply.")]
    pub headers: Vec<CatalogHeaderEditRow>,
}

/// Request payload for retrieving masked catalog headers.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogGetMaskedHeadersRequest {
    /// Catalog title from `list_command_topics`.
    #[schemars(description = "Catalog title from list_command_topics.")]
    pub catalog_id: String,
}

/// Request payload for removing an existing catalog from runtime config.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogRemoveRequest {
    /// Catalog identifier/title.
    #[schemars(description = "Catalog identifier or title.")]
    pub catalog_id: String,
    /// Remove persisted manifest artifact when true.
    #[schemars(description = "Remove persisted catalog manifest artifact when true.")]
    pub remove_manifest: Option<bool>,
}
