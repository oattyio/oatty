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
    /// Boolean flags are enabled by presence; their value element is ignored.
    #[schemars(
        description = "Named flag/value pairs as [name, value]. Value may be string/number/boolean/object/array. For boolean flags, presence enables the flag and value is ignored."
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
