use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}

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
    /// Supported range fields for pagination/sorting (e.g., ["id", "name", "updated_at"])
    #[serde(default)]
    pub ranges: Vec<String>,
}

/// Represents a positional argument for a command, including its name and help text.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct PositionalArgument {
    /// The name of the positional argument (e.g., "app")
    pub name: String,
    /// Optional help/description for this positional argument
    #[serde(default)]
    pub help: Option<String>,
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

/// Represents the current focus area in the UI.
///
/// This enum tracks which part of the interface currently has focus,
/// allowing for proper keyboard navigation and input handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Focus {
    /// Search input field in the command palette
    #[default]
    Search,
    /// Command list in the builder modal
    Commands,
    /// Input fields form in the builder modal
    Inputs,
}

/// Top-level screens available for the application to display.
///
/// This represents the primary navigation state for the TUI. Modal overlays
/// (help, table, builder) remain separate toggles so they can appear atop any
/// route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Screen {
    #[default]
    Home,
    Browser,
    Builder,
    Table,
    Help,
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
}

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
