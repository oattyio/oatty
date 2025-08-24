use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a command-line flag or option for a Heroku CLI command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandFlag {
    /// The name of the flag (e.g., "app", "region", "stack")
    pub name: String,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Resource group for the command (e.g., "apps")
    #[serde(default)]
    pub group: String,
    /// The full command name in format "resource:action" (e.g., "apps:list")
    pub name: String,
    /// Brief description of what the command does
    pub summary: String,
    /// Ordered list of positional argument names
    #[serde(default)]
    pub positional_args: Vec<String>,
    /// Help text for each positional argument, keyed by argument name
    #[serde(default)]
    pub positional_help: HashMap<String, String>,
    /// List of optional and required flags for this command
    #[serde(default)]
    pub flags: Vec<CommandFlag>,
    /// HTTP method used by this command (GET, POST, DELETE, etc.)
    pub method: String,
    /// API endpoint path (e.g., "/apps" or "/apps/{app}/dynos")
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

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
        assert!(spec.positional_help.is_empty());
        assert!(spec.flags.is_empty());
        assert_eq!(spec.method, "GET");
        assert_eq!(spec.path, "/apps");

        let back = serde_json::to_string(&spec).expect("serialize CommandSpec");
        let spec2: CommandSpec = serde_json::from_str(&back).expect("round-trip deserialize");
        assert_eq!(spec2.name, spec.name);
        assert_eq!(spec2.method, spec.method);
        assert_eq!(spec2.path, spec.path);
        assert_eq!(spec2.positional_args, spec.positional_args);
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
