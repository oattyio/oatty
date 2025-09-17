use std::{error::Error, str::FromStr};

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// Represents the type of suggestion item.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    /// A command name (e.g., "apps:list")
    Command,
    /// A flag or option (e.g., "--app", "--region")
    Flag,
    /// A value for a flag or positional argument
    Value,
    /// A positional argument (e.g., app name, dyno name)
    Positional,
}

/// Represents a single suggestion item in the palette.
#[derive(Clone, Debug)]
pub struct SuggestionItem {
    /// The text to display in the suggestion list
    pub display: String,
    /// The text to insert when the suggestion is selected
    pub insert_text: String,
    /// The type of suggestion (command, flag, value, etc.)
    pub kind: ItemKind,
    /// Optional metadata to display (e.g., flag description)
    pub meta: Option<String>,
    /// Score for ranking suggestions (higher is better)
    pub score: i64,
}

/// Declares how values for a parameter can be populated.
///
/// A ValueProvider typically references another command that can be executed
/// to fetch candidate values (e.g., using an `apps:list` command to populate
/// the values for an `--app` flag or a positional `app`). Additional variants
/// can be added later (e.g., static lists, plugins) without changing callers
/// that treat this as opaque metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum ValueProvider {
    /// Use another command identified by `<group>:<name>` to supply values,
    /// optionally binding required provider inputs to consumer inputs.
    ///
    /// Example: `Command { command_id: "apps:list".into(), binds: vec![] }`
    /// Example with bindings: `Command { command_id: "addons:list".into(), binds: vec![Bind { provider_key: "app".into(), from: "app".into() }] }`
    Command { command_id: String, binds: Vec<Bind> },
}

/// Declares a mapping from a provider's required input to a consumer field name.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Bind {
    /// The provider's input key (e.g., a path placeholder like `app`)
    pub provider_key: String,
    /// The consumer field name to source the value from (positional or flag name)
    pub from: String,
}

/// Represents a command-line flag or option for a Heroku CLI command.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct CommandFlag {
    /// The name of the flag (e.g., "app", "region", "stack")
    pub name: String,
    // the shortened command name of the flag (e.g. "a", "r" or "s")
    pub short_name: Option<String>,
    /// Whether this flag is required for the command to execute
    pub required: bool,
    /// The data type of the flag value (e.g., "string", "boolean", "integer")
    #[serde(default)]
    pub r#type: String,
    /// Valid enum values for this flag (empty if not an enum)
    #[serde(default)]
    pub enum_values: Vec<String>,
    /// Default value for this flag (None if no default)
    #[serde(default)]
    pub default_value: Option<String>,
    /// Human-readable description of what this flag does
    #[serde(default)]
    pub description: Option<String>,
    /// Optional ValueProvider that supplies dynamic values for this flag.
    ///
    /// When present, UIs and engines can query this provider to fetch
    /// candidate values for prompting and autocompletion.
    #[serde(default)]
    pub provider: Option<ValueProvider>,
}
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Default, Serialize, Deserialize, Encode, Decode)]
pub enum ServiceId {
    #[default]
    CoreApi, // https://api.heroku.com
    DataApi,        // https://api.data.heroku.com
    DataApiStaging, // https://heroku-data-api-staging.herokuapp.com
}

impl ToServiceIdInfo for ServiceId {
    fn env_var(&self) -> &str {
        match self {
            Self::CoreApi => "HEROKU_API_BASE",
            Self::DataApi | Self::DataApiStaging => "HEROKU_DATA_API_BASE",
        }
    }
    fn default_base_url(&self) -> &str {
        match self {
            Self::CoreApi => "https://api.heroku.com",
            Self::DataApi => "https://api.data.heroku.com",
            Self::DataApiStaging => "https://heroku-data-api-staging.herokuapp.com",
        }
    }
    fn accept_headers(&self) -> &str {
        match self {
            Self::CoreApi => "application/vnd.heroku+json; version=3",
            Self::DataApi | Self::DataApiStaging => "application/vnd.heroku+json; version=3",
        }
    }
}

impl FromStr for ServiceId {
    type Err = ParseServiceIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "core-api" => Ok(Self::CoreApi),
            "data-api" => Ok(Self::DataApi),
            "data-api-staging" => Ok(Self::DataApiStaging),
            _ => Err(ParseServiceIdError),
        }
    }
}

pub trait ToServiceIdInfo {
    fn env_var(&self) -> &str;
    fn default_base_url(&self) -> &str;
    fn accept_headers(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseServiceIdError;

impl std::fmt::Display for ParseServiceIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid service id; expected 'core' or 'data'")
    }
}

impl Error for ParseServiceIdError {}

/// Represents a complete Heroku CLI command specification.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct CommandSpec {
    /// Resource group for the command (e.g., "apps")
    #[serde(default)]
    pub group: String,
    /// The full command name in format "resource:action" (e.g., "apps:list")
    pub name: String,
    /// Brief description of what the command does
    pub summary: String,
    /// Ordered list of positional arguments with inline help
    #[serde(default)]
    pub positional_args: Vec<PositionalArgument>,
    /// List of optional and required flags for this command
    #[serde(default)]
    pub flags: Vec<CommandFlag>,
    /// HTTP method used by this command (GET, POST, DELETE, etc.)
    pub method: String,
    /// API endpoint path (e.g., "/apps" or "/apps/{app}/dynos")
    pub path: String,
    /// Supported range fields for pagination/sorting (e.g., ["id", "name",
    /// "updated_at"])
    #[serde(default)]
    pub ranges: Vec<String>,
    /// endpoint
    #[serde(default)]
    pub service_id: ServiceId,
}

/// Represents a positional argument for a command, including its name and help
/// text.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct PositionalArgument {
    /// The name of the positional argument (e.g., "app")
    pub name: String,
    /// Optional help/description for this positional argument
    #[serde(default)]
    pub help: Option<String>,
    /// Optional ValueProvider that supplies dynamic values for this positional.
    #[serde(default)]
    pub provider: Option<ValueProvider>,
}

/// Represents a single input field for a command parameter.
///
/// This struct contains all the metadata and state for a command parameter
/// including its type, validation rules, current value, and UI state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    /// The name of the field (e.g., "app", "region", "stack")
    pub name: String,
    /// Whether this field is required for the command to execute
    pub required: bool,
    /// Whether this field represents a boolean value (checkbox)
    pub is_bool: bool,
    /// The current value entered by the user
    pub value: String,
    /// Valid enum values for this field (empty if not an enum)
    pub enum_values: Vec<String>,
    /// Current selection index for enum fields
    pub enum_idx: Option<usize>,
}

/// Result of an asynchronous command execution.
///
/// This struct contains the outcome of a command execution including
/// logs, results, and any UI state changes that should occur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecOutcome {
    /// Log message from the command execution
    pub log: String,
    /// JSON result from the command (if any)
    pub result_json: Option<Value>,
    /// Whether to automatically open the table modal
    pub open_table: bool,
    /// Pagination info from the response header when available
    pub pagination: Option<Pagination>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Pagination {
    /// The start value of the returned range
    pub range_start: String,
    /// The end value of the returned range
    pub range_end: String,
    /// The property used to sort (e.g., "id", "name")
    pub field: String,
    /// The server page size limit used for this response (defaults to 200)
    pub max: usize,
    /// The sort order for the range ("asc" or "desc") if known
    #[serde(default)]
    pub order: Option<String>,
    /// Raw value of the Next-Range header for requesting the next page
    #[serde(default)]
    pub next_range: Option<String>,
}

/// Messages that can be sent to update the application state.
///
/// This enum defines all the possible user actions and system events
/// that can trigger state changes in the application.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Execute the current command
    Run,
    /// Copy the current command to clipboard
    CopyToClipboard(String),
    /// Periodic UI tick (e.g., throbbers)
    Tick,
    /// Terminal resized
    Resize(u16, u16),
    /// Background execution completed with outcome
    ExecCompleted(ExecOutcome),
    // Logs interactions
    /// Move log selection cursor up
    LogsUp,
    /// Move log selection cursor down
    LogsDown,
    /// Extend selection upwards (Shift+Up)
    LogsExtendUp,
    /// Extend selection downwards (Shift+Down)
    LogsExtendDown,
    /// Open details for the current selection
    LogsOpenDetail,
    /// Close details view and return to list
    LogsCloseDetail,
    /// Copy current selection (redacted)
    LogsCopy,
    /// Toggle pretty/raw for single API response
    LogsTogglePretty,
}

/// Side effects that can be triggered by state changes.
///
/// This enum defines actions that should be performed as a result
/// of state changes, such as copying to clipboard or showing notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Effect {
    /// Request to copy the current command to clipboard
    CopyToClipboardRequested(String),
    /// Request to copy the current logs selection (already rendered/redacted)
    CopyLogsRequested(String),
    /// Request the next page using the Raw Next-Range header
    NextPageRequested(String),
    /// Request the previous page using the prior Range header, if any
    PrevPageRequested,
    /// Request the first page using the initial Range header (or none)
    FirstPageRequested,
    /// Load MCP plugins from config into PluginsState
    PluginsLoadRequested,
    /// Refresh plugin statuses/health
    PluginsRefresh,
    /// Start the selected plugin
    PluginsStart(String),
    /// Stop the selected plugin
    PluginsStop(String),
    /// Restart the selected plugin
    PluginsRestart(String),
    /// Open logs drawer for a plugin
    PluginsOpenLogs(String),
    /// Refresh logs for open logs drawer (follow mode)
    PluginsRefreshLogs(String),
    /// Export logs for a plugin to a default location (redacted)
    PluginsExportLogsDefault(String),
    /// Open environment editor for a plugin
    PluginsOpenSecrets(String),
    /// Save environment changes for a plugin (key/value pairs)
    PluginsSaveEnv {
        name: String,
        rows: Vec<(String, String)>,
    },
    /// Open add plugin view
    PluginsOpenAdd,
    /// Open the secrets view
    PluginsValidateAdd,
    /// Apply add plugin patch
    PluginsApplyAdd,
    // Cancel adding a new plugin
    PluginsCancel,
    // Change the main view
    SwitchTo(Route),
    // Display a modal view
    ShowModal(Modal),
    // Hide any open modals
    CloseModal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Palette,
    Browser,
    Plugins,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Help,
    Secrets,
    Results,
}

pub struct ValidationResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_spec_round_trip_minimal() {
        let json = r#"{
            "name": "apps:list",
            "summary": "List apps",
            "method": "GET",
            "path": "/apps"
        }"#;

        let spec: CommandSpec = serde_json::from_str(json).expect("deserialize CommandSpec");
        assert_eq!(spec.group, "");
        assert_eq!(spec.name, "apps:list");
        assert_eq!(spec.summary, "List apps");
        assert!(spec.positional_args.is_empty());
        assert!(spec.flags.is_empty());
        assert_eq!(spec.method, "GET");
        assert_eq!(spec.path, "/apps");

        let back = serde_json::to_string(&spec).expect("serialize CommandSpec");
        let spec2: CommandSpec = serde_json::from_str(&back).expect("round-trip deserialize");
        assert_eq!(spec2.name, spec.name);
        assert_eq!(spec2.method, spec.method);
        assert_eq!(spec2.path, spec.path);
        assert_eq!(spec2.positional_args.len(), spec.positional_args.len());
        assert_eq!(spec2.flags.len(), 0);
    }

    #[test]
    fn command_flag_defaults() {
        let json = r#"{
            "name": "region",
            "required": false
        }"#;
        let flag: CommandFlag = serde_json::from_str(json).expect("deserialize CommandFlag");
        assert_eq!(flag.name, "region");
        assert!(!flag.required);
        assert_eq!(flag.r#type, "");
        assert_eq!(flag.enum_values, Vec::<String>::new());
        assert!(flag.default_value.is_none());
        assert!(flag.description.is_none());
    }
}
