use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for command discovery.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchRequestParam {
    /// Free-text query used for full-text command search.
    #[schemars(description = "Free-text query for command/tool discovery.")]
    pub query: String,
    /// Optional vendor filter. When provided, only commands from the matching vendor are returned.
    #[schemars(description = "Optional vendor filter to limit results to one provider.")]
    pub vendor: Option<String>,
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
        description = "Named flag/value pairs as [name, value]. For boolean flags, presence enables the flag and value is ignored."
    )]
    pub named_flags: Option<Vec<(String, String)>>,
}

/// Parameters for catalog-level summary lookups.
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandSummariesRequest {
    /// Human-readable catalog title as returned by catalog listing tools.
    #[schemars(description = "Catalog title to inspect. Use list_command_topics output values.")]
    pub catalog_title: String,
}
