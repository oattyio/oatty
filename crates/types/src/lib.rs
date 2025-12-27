//! Core type definitions shared across the Oatty CLI workspace.
//!
//! The `heroku-types` crate centralizes serde-friendly data structures that describe CLI commands,
//! palette suggestions, execution outcomes, and the message/effect system shared by the engine and
//! TUI crates.

pub mod suggestion {
    //! Suggestion metadata used by palette and autocompletion UIs.
    /// Identifies the kind of suggestion item presented to the user.
    #[derive(Clone, Debug, PartialEq)]
    pub enum ItemKind {
        /// A canonical command ID (for example, "apps:list").
        Command,
        /// A Canonical MCP Tool ID (for example, "brave web:search").
        MCP,
        /// A flag or option (for example, "--app" or "--region").
        Flag,
        /// A value for a flag or positional argument.
        Value,
        /// A positional argument (for example, an app name or dyno name).
        Positional,
    }

    /// High-level metadata for an autocompletion or palette suggestion.
    #[derive(Clone, Debug)]
    pub struct SuggestionItem {
        /// The text to display in the suggestion list.
        pub display: String,
        /// The text to insert when the suggestion is selected.
        pub insert_text: String,
        /// The type of suggestion (command, flag, value, or positional).
        pub kind: ItemKind,
        /// Optional metadata to display (for instance, a flag description).
        pub meta: Option<String>,
        /// Score for ranking suggestions (higher scores are preferred).
        pub score: i64,
    }
}

pub mod provider {
    //! Value-provider metadata describing how dynamic values are discovered.

    use serde::{Deserialize, Serialize};

    /// Declares how values for a parameter can be populated.
    ///
    /// A `ValueProvider` typically references another command that can be executed to fetch
    /// candidate values. For example, a palette may reuse the `apps:list` command to populate the
    /// values for an `--app` flag or a positional `app`.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub enum ValueProvider {
        /// Use another command identified by `<group>:<name>` to supply values.
        ///
        /// Required provider inputs can be bound to consumer inputs when necessary.
        Command { command_id: String, binds: Vec<Bind> },
    }

    /// Declares a mapping from a provider's required to be input to a consumer field name.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Bind {
        /// The provider's input key (for example, a path placeholder like `app`).
        pub provider_key: String,
        /// The consumer field name to source the value from (positional or flag name).
        pub from: String,
    }

    /// Describes the argument and return contracts for a provider command.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct ProviderContract {
        /// Provider arguments in the order they should be considered for auto-mapping.
        #[serde(default)]
        pub arguments: Vec<ProviderArgumentContract>,
        /// Metadata describing the returned item fields from the provider.
        #[serde(default)]
        pub returns: ProviderReturnContract,
    }

    /// Contract metadata for a single provider argument.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct ProviderArgumentContract {
        /// Name of the argument (for example, `app`).
        pub name: String,
        /// Semantic tags that the provider accepts for this argument (for example, `app_id`).
        #[serde(default)]
        pub accepts: Vec<String>,
        /// Preferred tag to use when multiple accepted tags are available.
        #[serde(default)]
        pub prefer: Option<String>,
        /// Indicates whether the argument is required by the provider.
        #[serde(default)]
        pub required: bool,
    }

    /// Declarative return metadata for provider results.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct ProviderReturnContract {
        /// Fields that the provider returns for each item.
        #[serde(default)]
        pub fields: Vec<ProviderFieldContract>,
    }

    /// Metadata about an individual provider return field.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ProviderFieldContract {
        /// Name of the field returned in the provider payload.
        pub name: String,
        /// Optional JSON type hint (for example, `string`, `array`, `object`).
        #[serde(default)]
        pub r#type: Option<String>,
        /// Semantic tags describing how the field can be used for auto-mapping.
        #[serde(default)]
        pub tags: Vec<String>,
    }
}

/// Rich workflow schema models used by the registry, engine, and TUI layers.
pub mod workflow;

pub mod manifest {
    //! Registry manifest structures embedded into the generated binary artifact.

    use std::collections::HashMap;

    use indexmap::IndexMap;
    use postcard::from_bytes;
    use serde::{Deserialize, Serialize};

    use crate::{command::CommandSpec, provider::ProviderContract, workflow::WorkflowDefinition};

    /// Registry catalog structure used by the registry, engine, and TUI layers.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, Eq, PartialEq)]
    pub struct RegistryCatalog {
        pub name: String,
        pub description: String,
        pub manifest_path: String,
        pub headers: HashMap<String, String>,
        pub base_url: String,
    }

    /// Serialized manifest housing both command specifications and workflow definitions.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
    pub struct RegistryManifest {
        /// All command specifications generated from the platform schemas.
        #[serde(default)]
        pub commands: Vec<CommandSpec>,
        /// Workflow definitions bundled alongside the command registry.
        #[serde(default)]
        pub workflows: Vec<WorkflowDefinition>,
        /// Provider argument and return contracts keyed by command identifier.
        #[serde(default)]
        pub provider_contracts: IndexMap<String, ProviderContract>,
    }

    impl TryFrom<Vec<u8>> for RegistryManifest {
        type Error = anyhow::Error;

        fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
            let manifest = from_bytes::<RegistryManifest>(&bytes)?;
            Ok(manifest)
        }
    }
}

pub mod service {
    //! Supported HTTP API services and helpers for resolving endpoints.
    use serde::{Deserialize, Serialize};
    use std::{error::Error, fmt, str::FromStr};

    /// Default accept header shared across Oatty APIs.
    const HEROKU_JSON_ACCEPT_HEADER: &str = "application/vnd.heroku+json; version=3.sdk";

    /// Identifies the backend service targeted by a generated request.
    #[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Default, Serialize, Deserialize)]
    pub enum ServiceId {
        /// Core Oatty Platform API (`https://api.heroku.com`).
        #[default]
        CoreApi,
        /// Oatty Data API (`https://api.data.heroku.com`).
        DataApi,
        /// Oatty Data API staging environment (`https://heroku-data-api-staging.herokuapp.com`).
        DataApiStaging,
    }

    impl ServiceId {
        fn base_url(&self) -> &str {
            match self {
                Self::CoreApi => "https://api.heroku.com",
                Self::DataApi => "https://api.data.heroku.com",
                Self::DataApiStaging => "https://heroku-data-api-staging.herokuapp.com",
            }
        }
    }

    /// Provides convenient accessors for service metadata.
    pub trait ToServiceIdInfo {
        /// Returns the environment variable that overrides the base URL.
        fn env_var(&self) -> &str;
        /// Returns the default base URL for the service.
        fn default_base_url(&self) -> &str;
        /// Returns the HTTP `Accept` header expected by the service.
        fn accept_headers(&self) -> &str;
    }

    impl ToServiceIdInfo for ServiceId {
        fn env_var(&self) -> &str {
            match self {
                Self::CoreApi => "HEROKU_API_BASE",
                Self::DataApi | Self::DataApiStaging => "HEROKU_DATA_API_BASE",
            }
        }

        fn default_base_url(&self) -> &str {
            self.base_url()
        }

        fn accept_headers(&self) -> &str {
            HEROKU_JSON_ACCEPT_HEADER
        }
    }

    /// Error returned when attempting to parse an unsupported service identifier.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ParseServiceIdError;

    impl fmt::Display for ParseServiceIdError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("invalid service id; expected 'core-api', 'data-api', or 'data-api-staging'")
        }
    }

    impl Error for ParseServiceIdError {}

    impl FromStr for ServiceId {
        type Err = ParseServiceIdError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "core-api" => Ok(Self::CoreApi),
                "data-api" => Ok(Self::DataApi),
                "data-api-staging" => Ok(Self::DataApiStaging),
                _ => Err(ParseServiceIdError),
            }
        }
    }
}

pub mod command {
    //! Command metadata describing CLI commands and their inputs.

    use crate::{provider::ValueProvider, service::ServiceId};
    use anyhow::Result;
    use anyhow::anyhow;

    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    pub type FlagValueMap = HashMap<String, Option<String>>;
    pub type ArgValueMap = HashMap<String, String>;
    pub type ParsedCommandArgs = (FlagValueMap, ArgValueMap);
    /// Represents a command-line flag or option for a Oatty CLI command.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct CommandFlag {
        /// The name of the flag (for example, "app", "region", or "stack").
        pub name: String,
        /// The shortened command name of the flag (for example, "a", "r", or "s").
        pub short_name: Option<String>,
        /// Whether this flag is required for the command to execute.
        pub required: bool,
        /// The data type of the flag value (for example, "string", "boolean", or "integer").
        #[serde(default)]
        pub r#type: String,
        /// Valid enum values for this flag (empty if not an enum).
        #[serde(default)]
        pub enum_values: Vec<String>,
        /// Default value for this flag (None if no default).
        #[serde(default)]
        pub default_value: Option<String>,
        /// Human-readable description of what this flag does.
        #[serde(default)]
        pub description: Option<String>,
        /// Optional `ValueProvider` that supplies dynamic values for this flag.
        #[serde(default)]
        pub provider: Option<ValueProvider>,
    }

    /// Represents a positional argument for a command, including its name and help text.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PositionalArgument {
        /// The name of the positional argument (for example, "app").
        pub name: String,
        /// Optional help or description for this positional argument.
        #[serde(default)]
        pub help: Option<String>,
        /// Optional `ValueProvider` that supplies dynamic values for this positional.
        #[serde(default)]
        pub provider: Option<ValueProvider>,
    }

    /// Shape metadata describing the structure of command outputs.
    ///
    /// This schema summary is designed for UI consumption. It retains enough detail to render the
    /// Field Picker, disambiguate auto-mapping targets, and surface the semantic meaning of leaf
    /// values without requiring the full JSON schema. Additional annotations may be layered on by
    /// workflow output contracts or provider metadata.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SchemaProperty {
        /// JSON type reported by the upstream schema (object, array, string, and so on).
        pub r#type: String,
        /// Human-readable description for the property. Empty string when omitted upstream.
        pub description: String,
        /// Nested fields when the property is an object. Keys are property names.
        #[serde(default)]
        pub properties: Option<HashMap<String, Box<Self>>>,
        /// Names of required child properties. Applies when `r#type == "object"`.
        #[serde(default)]
        pub required: Vec<String>,
        /// Schema definition for array items when `r#type == "array"`.
        #[serde(default)]
        pub items: Option<Box<Self>>,
        /// Enumerated literal values allowed for this property.
        #[serde(default)]
        pub enum_values: Vec<String>,
        /// Optional format hint supplied by the schema (for example, `uuid`, `date-time`).
        #[serde(default)]
        pub format: Option<String>,
        /// Semantic tags carried alongside the property. Currently populated by workflow
        /// annotations to influence auto-mapping heuristics.
        #[serde(default)]
        pub tags: Vec<String>,
    }

    /// Represents a complete Oatty CLI command specification.
    ///
    /// A `CommandSpec` now distinguishes between multiple execution backends via the
    /// [`CommandExecution`] enum. HTTP-based commands remain the default, while MCP-backed
    /// commands use the `Mcp` variant.
    ///
    /// # Examples
    ///
    /// Creating an HTTP-backed command:
    /// ```rust
    /// use oatty_types::{CommandExecution, CommandSpec, HttpCommandSpec, ServiceId};
    ///
    /// let http = HttpCommandSpec::new("GET", "/apps", ServiceId::CoreApi, Vec::new(), None);
    /// let spec = CommandSpec::new_http(
    ///     "apps".into(),
    ///     "apps:list".into(),
    ///     "List apps".into(),
    ///     Vec::new(),
    ///     Vec::new(),
    ///     http,
    /// );
    /// assert!(matches!(spec.execution(), CommandExecution::Http(_)));
    /// ```
    ///
    /// Creating an MCP-backed command:
    /// ```rust
    /// use oatty_types::{CommandExecution, CommandSpec, McpCommandSpec};
    ///
    /// let mcp = McpCommandSpec {
    ///     plugin_name: "demo-plugin".into(),
    ///     tool_name: "demo_tool".into(),
    ///     auth_summary: Some("Needs OAuth".into()),
    ///     output_schema: None,
    ///     render_hint: None,
    /// };
    /// let spec = CommandSpec::new_mcp(
    ///     "mcp.demo".into(),
    ///     "demo:tool".into(),
    ///     "Needs OAuth — Demo tool".into(),
    ///     Vec::new(),
    ///     Vec::new(),
    ///     mcp,
    /// );
    /// assert!(matches!(spec.execution(), CommandExecution::Mcp(_)));
    /// ```
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct CommandSpec {
        /// Resource group for the command (for example, "apps").
        #[serde(default)]
        pub group: String,
        /// The full command name in format "resource:action" (for example, "apps:list").
        pub name: String,
        /// Brief description of what the command does.
        pub summary: String,
        /// Ordered list of positional arguments with inline help.
        #[serde(default)]
        pub positional_args: Vec<PositionalArgument>,
        /// List of optional and required flags for this command.
        #[serde(default)]
        pub flags: Vec<CommandFlag>,
        /// Execution metadata describing how this command should be fulfilled.
        #[serde(default)]
        pub execution: CommandExecution,
    }

    impl CommandSpec {
        /// Access the execution configuration regardless of variant.
        pub fn execution(&self) -> &CommandExecution {
            &self.execution
        }

        /// Mutably access the execution configuration regardless of variant.
        pub fn execution_mut(&mut self) -> &mut CommandExecution {
            &mut self.execution
        }

        /// Returns the HTTP execution payload when this command targets an HTTP backend.
        pub fn http(&self) -> Option<&HttpCommandSpec> {
            match &self.execution {
                CommandExecution::Http(http) => Some(http),
                _ => None,
            }
        }

        /// Returns a mutable HTTP execution payload when this command targets an HTTP backend.
        pub fn http_mut(&mut self) -> Option<&mut HttpCommandSpec> {
            match &mut self.execution {
                CommandExecution::Http(http) => Some(http),
                _ => None,
            }
        }

        /// Returns the MCP execution payload when this command is backed by an MCP tool.
        pub fn mcp(&self) -> Option<&McpCommandSpec> {
            match &self.execution {
                CommandExecution::Mcp(mcp) => Some(mcp),
                _ => None,
            }
        }

        /// Returns a mutable MCP execution payload when this command is backed by an MCP tool.
        pub fn mcp_mut(&mut self) -> Option<&mut McpCommandSpec> {
            match &mut self.execution {
                CommandExecution::Mcp(mcp) => Some(mcp),
                _ => None,
            }
        }

        /// Returns the canonical ID for this command.
        pub fn canonical_id(&self) -> String {
            format!("{} {}", self.group, self.name)
        }

        /// Construct a new HTTP-backed command specification.
        pub fn new_http(
            group: String,
            name: String,
            summary: String,
            positional_args: Vec<PositionalArgument>,
            flags: Vec<CommandFlag>,
            http: HttpCommandSpec,
        ) -> Self {
            Self {
                group,
                name,
                summary,
                positional_args,
                flags,
                execution: CommandExecution::Http(http),
            }
        }

        /// Construct a new MCP-backed command specification.
        pub fn new_mcp(
            group: String,
            name: String,
            summary: String,
            positional_args: Vec<PositionalArgument>,
            flags: Vec<CommandFlag>,
            mcp: McpCommandSpec,
        ) -> Self {
            Self {
                group,
                name,
                summary,
                positional_args,
                flags,
                execution: CommandExecution::Mcp(mcp),
            }
        }

        /// Parses command arguments and flags from input tokens.
        ///
        /// This function processes the command line tokens after the group and subcommand,
        /// separating positional arguments from flags and validating flag syntax.
        ///
        /// # Arguments
        ///
        /// * `argument_tokens` - The tokens after the group and subcommand
        /// * `command_spec` - The command specification for validation
        ///
        /// # Returns
        ///
        /// Returns `Ok((flags, args))` where flags is a map of flag names to values
        /// and args is a vector of positional arguments, or an error if parsing fails.
        ///
        /// # Flag Parsing Rules
        ///
        /// - `--flag=value` format is supported
        /// - Boolean flags don't require values
        /// - Non-boolean flags require values (next token or after =)
        /// - Unknown flags are rejected
        pub fn parse_arguments(&self, argument_tokens: &[String]) -> Result<ParsedCommandArgs> {
            let mut user_flags: FlagValueMap = HashMap::new();
            let mut user_args: Vec<String> = Vec::new();
            let mut index = 0;

            while index < argument_tokens.len() {
                let token = &argument_tokens[index];

                if token.starts_with("--") {
                    let flag_name = token.trim_start_matches('-');

                    // Handle --flag=value format
                    if let Some(equals_pos) = flag_name.find('=') {
                        let name = &flag_name[..equals_pos];
                        let value = &flag_name[equals_pos + 1..];
                        user_flags.insert(name.to_string(), Some(value.to_string()));
                    } else {
                        // Handle --flag or --flag value format
                        if let Some(flag_spec) = self.flags.iter().find(|f| f.name == flag_name) {
                            if flag_spec.r#type == "boolean" {
                                user_flags.insert(flag_name.to_string(), None);
                            } else {
                                // Non-boolean flag requires a value
                                if index + 1 < argument_tokens.len() && !argument_tokens[index + 1].starts_with('-') {
                                    user_flags.insert(flag_name.to_string(), Some(argument_tokens[index + 1].to_string()));
                                    index += 1; // Skip the value token
                                } else {
                                    return Err(anyhow!("Flag '--{}' requires a value", flag_name));
                                }
                            }
                        } else {
                            return Err(anyhow!("Unknown flag '--{}'", flag_name));
                        }
                    }
                } else {
                    // Positional argument
                    user_args.push(token.to_string());
                }

                index += 1;
            }
            self.validate_arguments(&user_flags, &user_args)?;

            let user_args_map = self
                .positional_args
                .iter()
                .zip(user_args.iter())
                .map(|(arg, value)| (arg.name.to_string(), value.to_string()))
                .collect();
            Ok((user_flags, user_args_map))
        }

        /// Validates command arguments and flags against the command specification.
        ///
        /// This function ensures that all required positional arguments and flags are
        /// provided with appropriate values.
        ///
        /// # Arguments
        ///
        /// * `positional_arguments` - The provided positional arguments
        /// * `user_flags` - The provided flags and their values
        /// * `command_spec` - The command specification to validate against
        ///
        /// # Returns
        ///
        /// Returns `Ok(())` if validation passes, or an error message if validation fails.
        ///
        /// # Validation Rules
        ///
        /// - All required positional arguments must be provided
        /// - All required flags must be present
        /// - Non-boolean required flags must have non-empty values
        pub fn validate_arguments(&self, user_flags: &HashMap<String, Option<String>>, positional_arguments: &[String]) -> Result<()> {
            // Validate required positional arguments
            if positional_arguments.len() > self.positional_args.len() {
                return Err(anyhow!(
                    "Too many arguments provided: expected {}, got {}",
                    self.positional_args.len(),
                    positional_arguments.len()
                ));
            }
            if positional_arguments.len() < self.positional_args.len() {
                let missing_arguments: Vec<String> = self.positional_args[positional_arguments.len()..]
                    .iter()
                    .map(|arg| arg.name.to_string())
                    .collect();
                return Err(anyhow!("Missing required argument(s): {}", missing_arguments.join(", ")));
            }

            // Validate required flags
            for flag_spec in &self.flags {
                if flag_spec.required {
                    if flag_spec.r#type == "boolean" {
                        if !user_flags.contains_key(&flag_spec.name) {
                            return Err(anyhow!("Missing required flag: --{}", flag_spec.name));
                        }
                    } else {
                        match user_flags.get(&flag_spec.name) {
                            Some(Some(value)) if !value.is_empty() => {}
                            _ => {
                                return Err(anyhow!("Missing required flag value: --{} <value>", flag_spec.name));
                            }
                        }
                    }
                }
            }

            Ok(())
        }
    }

    /// Execution metadata for a command.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum CommandExecution {
        /// Command is fulfilled via an HTTP request described by [`HttpCommandSpec`].
        Http(HttpCommandSpec),
        /// Command is fulfilled by delegating to an MCP tool described by [`McpCommandSpec`].
        Mcp(McpCommandSpec),
    }

    impl Default for CommandExecution {
        fn default() -> Self {
            Self::Http(HttpCommandSpec::default())
        }
    }

    /// HTTP execution metadata.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct HttpCommandSpec {
        /// HTTP method (GET, POST, DELETE, and so on).
        pub method: String,
        /// API endpoint path (for example, "/apps" or "/apps/{app}/dynos").
        pub path: String,
        /// Supported range fields for pagination or sorting (for example, "id", "name").
        #[serde(default)]
        pub ranges: Vec<String>,
        /// Identifier for the API service that owns the endpoint.
        #[serde(default)]
        pub service_id: ServiceId,
        // Schema expected from API
        pub output_schema: Option<SchemaProperty>,
    }

    impl HttpCommandSpec {
        /// Create a new HTTP execution payload with the provided metadata.
        pub fn new(
            method: impl Into<String>,
            path: impl Into<String>,
            service_id: ServiceId,
            ranges: Vec<String>,
            output_schema: Option<SchemaProperty>,
        ) -> Self {
            Self {
                method: method.into(),
                path: path.into(),
                ranges,
                service_id,
                output_schema,
            }
        }
    }

    /// MCP execution metadata capturing plugin delegation details.
    ///
    /// Instances of this struct are typically constructed from tool discovery metadata provided by
    /// the MCP runtime. Consumers should propagate human-readable authentication requirements into
    /// `auth_summary` so that downstream UIs can display them alongside command descriptions.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct McpCommandSpec {
        /// Name of the plugin that owns the tool.
        pub plugin_name: String,
        /// Identifier for the tool within the plugin.
        pub tool_name: String,
        /// Optional summary describing authentication requirements.
        #[serde(default)]
        pub auth_summary: Option<String>,
        /// Optional JSON schema describing tool output, encoded as a string for transport.
        pub output_schema: Option<SchemaProperty>,
        /// Optional hint indicating how the UI should render results (for example, "table").
        #[serde(default)]
        pub render_hint: Option<String>,
    }

    /// Represents a single input field for a command parameter.
    ///
    /// This struct contains all the metadata and state for a command parameter including its type,
    /// validation rules, current value, and UI state.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Field {
        /// The name of the field (for example, "app", "region", or "stack").
        pub name: String,
        /// Whether this field is required for the command to execute.
        pub required: bool,
        /// Whether this field represents a boolean value (checkbox).
        pub is_bool: bool,
        /// The current value entered by the user.
        pub value: String,
        /// Valid enum values for this field (empty if not an enum).
        pub enum_values: Vec<String>,
        /// Current selection index for enum fields.
        pub enum_idx: Option<usize>,
    }
}

pub mod execution {
    //! Execution outcomes and pagination metadata produced by the engine.

    use std::{borrow::Cow, path::PathBuf};

    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use url::Url;

    use crate::{Msg, PluginDetail};

    /// Result of an asynchronous command execution.
    ///
    /// This struct contains the outcome of a command execution including logs, results, and any UI
    /// state changes that should occur.
    #[derive(Debug, Clone, Default)]
    pub enum ExecOutcome {
        /// Result from executing a file contents command containing a structured payload.
        FileContents(Cow<'static, str>, PathBuf),
        /// Result from executing a remote file contents command containing a structured payload.
        RemoteFileContents(Cow<'static, str>, Url),
        /// Result from executing a directory contents command containing a structured payload.
        DirectoryContents {
            /// Files and directories present in the requested location.
            entries: Vec<DirectoryEntry>,
            /// Directory that was enumerated.
            root_path: PathBuf,
        },
        /// Result from executing an HTTP command containing a structured payload.
        Http {
            /// HTTP status returned by the service.
            status_code: u16,
            /// Human readable summary of the response.
            log_entry: String,
            /// JSON payload parsed from the response body.
            payload: Value,
            /// Pagination metadata parsed from headers, if present.
            pagination: Option<Pagination>,
            /// Identifier correlating the request to UI events.
            request_id: u64,
        },
        /// Simple log entry
        Log(String),
        /// Simple message
        Message(Msg),
        /// Result from executing an MCP tool containing a structured payload.
        Mcp {
            /// Human readable summary for the tool execution.
            log_entry: String,
            /// Structured JSON payload returned by the tool.
            payload: Value,
            /// Identifier correlating the request to UI events.
            request_id: u64,
        },
        /// Result from fetching provider-backed values for suggestions or selectors.
        ProviderValues {
            /// Unique provider identifier requested.
            provider_id: String,
            /// Cache key representing the argument combination.
            cache_key: String,
            /// Values produced by the provider.
            values: Vec<Value>,
            /// Optional identifier correlating the request to UI events.
            request_id: Option<u64>,
        },
        /// Result from performing an action on a plugin
        /// Contains a log message and the new plugin detail object
        PluginDetail {
            /// Message summarizing the action performed.
            message: String,
            /// Updated plugin detail (if available).
            detail: Option<PluginDetail>,
        },
        /// Result from fetching detailed plugin information for the modal.
        PluginDetailLoad {
            /// Plugin name that was loaded.
            plugin_name: String,
            /// Result of fetching detail information.
            result: Result<PluginDetail, String>,
        },
        /// Result from refreshing the plugins.
        /// Contains a log message and the entire
        /// list of PluginDetail objects.
        PluginsRefresh {
            /// Message summarizing the refresh action.
            message: String,
            /// Complete collection of plugin details, when returned.
            details: Option<Vec<PluginDetail>>,
        },
        /// Validation plugin error result
        /// Contains the error message
        PluginValidationErr {
            /// Error message explaining the validation failure.
            message: String,
        },
        /// Validation plugin ok result
        /// Contains the success message
        PluginValidationOk {
            /// Message describing the successful validation.
            message: String,
        },
        /// Command validation error
        ValidationErr(String),
        /// Indicates successful completion but with no payload
        #[default]
        None,
    }

    /// File-system entry returned by [`ExecOutcome::DirectoryContents`].
    #[derive(Debug, Clone)]
    pub struct DirectoryEntry {
        /// Absolute or relative path to the entry.
        pub path: PathBuf,
        /// Indicates whether this entry points to a directory.
        pub is_directory: bool,
    }

    /// Pagination metadata parsed from API response headers.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, Hash, PartialEq, Eq)]
    pub struct Pagination {
        /// The start value of the returned range.
        pub range_start: String,
        /// The end value of the returned range.
        pub range_end: String,
        /// The property used to sort (for example, "id" or "name").
        pub field: String,
        /// The server page size limit used for this response (defaults to 200).
        pub max: usize,
        /// The shell command needed to fetch the next/previous page of results.
        pub hydrated_shell_command: Option<String>,
        /// The sort order for the range ("asc" or "desc") if known.
        #[serde(default)]
        pub order: Option<String>,
        /// Raw value of the Next-Range header for requesting the next page.
        #[serde(default)]
        pub next_range: Option<String>,
        /// Raw value of the Prev-Range header for requesting the previous page.
        #[serde(default)]
        pub this_range: Option<String>,
    }
}

pub mod messaging {
    //! Application-level messages and side effects.

    use crate::{
        command::CommandSpec,
        execution::ExecOutcome,
        workflow::{WorkflowRunControl, WorkflowRunEvent, WorkflowRunRequest},
    };
    use serde_json::{Map as JsonMap, Value as JsonValue};
    use std::{fmt::Display, path::PathBuf};
    use url::Url;
    /// Navigation targets within the TUI.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Route {
        /// Palette view for selecting commands.
        Palette,
        /// Browser view for inspecting command specifications.
        Browser,
        /// Plugins view for managing MCP plugins.
        Plugins,
        /// Settings view for configuring application preferences.
        Library,
        /// Workflows view for browsing workflow catalog.
        Workflows,
        /// Workflow input resolution view.
        WorkflowInputs,
        /// Workflow run view displaying live execution status.
        WorkflowRun,
    }

    /// Modal overlays that can be displayed on top of the main UI.
    #[derive(Debug, Clone)]
    pub enum Modal {
        /// File picker modal for selecting files.
        FilePicker(Vec<&'static str>),
        /// Help modal displaying shortcuts and usage tips.
        Help,
        /// Results modal showing API responses in a table.
        Results(Box<ExecOutcome>),
        /// Log details modal revealing the full log entry.
        LogDetails,
        /// Guided Input Collector for resolving workflow inputs.
        WorkflowCollector,
        /// Plugin details modal presenting plugin metadata.
        PluginDetails,
        /// Theme picker modal allowing runtime palette switching.
        ThemePicker,
        /// Confirmation modal prompting the user to confirm an action.
        Confirmation,
    }

    #[derive(Default, Debug, Clone, PartialEq, Eq)]
    pub enum Severity {
        #[default]
        Info,
        Warning,
        Error,
    }

    impl Display for Severity {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Severity::Info => write!(f, "ⓘ Info"),
                Severity::Warning => write!(f, "⚠ Warning"),
                Severity::Error => write!(f, "X Error"),
            }
        }
    }

    /// Side effects that can be triggered by state changes.
    ///
    /// This enum defines actions that should be performed as a result of state changes, such as
    /// copying to clipboard or showing notifications.
    #[derive(Debug, Clone)]
    pub enum Effect {
        /// Log a message
        Log(String),
        /// Request to run the current command in the palette
        /// with the hydrated command string and u64 hash of the request.
        Run {
            hydrated_command: String,
            range_override: Option<String>,
            request_hash: u64,
        },
        /// Request to copy the current command to the clipboard.
        CopyToClipboardRequested(String),
        /// Request to copy the current logs selection (already rendered/redacted).
        CopyLogsRequested(String),
        /// Open the selected file for processing.
        ReadFileContents(PathBuf),
        /// Fetch the remote file contents.
        ReadRemoteFileContents(Url),
        /// List the contents of a directory.
        ListDirectoryContents(PathBuf),
        /// Load MCP plugins from config into `PluginsState`.
        PluginsLoadRequested,
        /// Refresh plugin statuses and health.
        PluginsRefresh,
        /// Start the selected plugin.
        PluginsStart(String),
        /// Stop the selected plugin.
        PluginsStop(String),
        /// Restart the selected plugin.
        PluginsRestart(String),
        /// Export logs for a plugin to a default location (redacted).
        PluginsExportLogsDefault(String),
        /// Validate fields in the added plugin view.
        PluginsValidateAdd,
        /// Apply to add a plugin patch.
        PluginsSave,
        /// Load detailed information for a plugin when opening the details modal.
        PluginsLoadDetail(String),
        /// Change the main view.
        SwitchTo(Route),
        /// Display a modal view.
        ShowModal(Modal),
        /// Hide any open modals.
        CloseModal,
        /// Send the command spec to the palette.
        SendToPalette(Box<CommandSpec>),
        /// Request fetching values for a provider-backed suggestion or selector.
        ProviderFetchRequested {
            /// Canonical provider identifier (`group name`).
            provider_id: String,
            /// Cache key associated with the provider arguments.
            cache_key: String,
            /// Arguments that should be supplied to the provider request.
            args: JsonMap<String, JsonValue>,
        },
        /// Request execution of a workflow run.
        WorkflowRunRequested {
            /// Run configuration describing the workflow and context.
            request: Box<WorkflowRunRequest>,
        },
        /// Send a control command to an in-flight workflow run.
        WorkflowRunControl {
            /// Identifier of the run to target.
            run_id: String,
            /// Control command (pause, resume, cancel).
            command: WorkflowRunControl,
        },
        SendMsg(Msg),
    }

    /// Messages that can be sent to update the application state.
    ///
    /// This enum defines all the possible user actions and system events that can trigger state
    /// changes in the application.
    #[derive(Debug, Clone)]
    pub enum Msg {
        /// The user has clicked a button from the confirmation modal
        /// The usize is the corresponding widget id of the button clicked
        ConfirmationModalButtonClicked(usize),
        /// The user has dismissed the confirmation modal
        ConfirmationModalClosed,
        /// Copy the current command to the clipboard.
        CopyToClipboard(String),
        /// Periodic UI tick (for example, throbbers).
        Tick,
        /// Terminal resized.
        Resize(u16, u16),
        /// Background execution completed with an outcome.
        ExecCompleted(Box<ExecOutcome>),
        /// Move the log selection cursor up.
        LogsUp,
        /// Move the log selection cursor down.
        LogsDown,
        /// Extend selection upwards (Shift+Up).
        LogsExtendUp,
        /// Extend selection downwards (Shift+Down).
        LogsExtendDown,
        /// Open details for the current selection.
        LogsOpenDetail,
        /// Close the details view and return to list.
        LogsCloseDetail,
        /// Copy the current selection (redacted).
        LogsCopy,
        /// Toggle pretty/raw for a single API response.
        LogsTogglePretty,
        /// Provider-backed values finished loading and are ready for consumption.
        ProviderValuesReady {
            /// Canonical provider identifier (`group name`).
            provider_id: String,
            /// Cache key whose contents are now available from the registry cache.
            cache_key: String,
        },
        /// Workflow runner emitted an event for the active run.
        WorkflowRunEvent {
            /// Identifier of the run associated with the event.
            run_id: String,
            /// Event payload describing the lifecycle change.
            event: WorkflowRunEvent,
        },
    }
}

pub mod plugin {
    //! Plugin metadata, status tracking, and logging primitives shared between the MCP engine and
    //! the TUI presentation layer.

    use std::{
        fmt,
        time::{Duration, SystemTime},
    };

    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    /// High-level lifecycle state for a plugin instance.
    #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    pub enum PluginStatus {
        /// Plugin is running and healthy.
        Running,
        /// Plugin is stopped.
        #[default]
        Stopped,
        /// Plugin has warnings (for example, slow responses).
        Warning,
        /// Plugin encountered an error.
        Error,
        /// Plugin is starting up.
        Starting,
        /// Plugin is in the process of stopping.
        Stopping,
        /// Plugin status is unknown (for example, not configured).
        Unknown,
    }

    impl PluginStatus {
        /// Returns the icon used in the TUI for this status.
        pub fn icon(&self) -> &'static str {
            match self {
                PluginStatus::Running => "✓",
                PluginStatus::Stopped => "✗",
                PluginStatus::Warning => "!",
                PluginStatus::Error => "✗",
                PluginStatus::Starting => "⏳",
                PluginStatus::Stopping => "⏳",
                PluginStatus::Unknown => "?",
            }
        }

        /// Returns a human-readable description of the status.
        pub fn display(&self) -> &'static str {
            match self {
                PluginStatus::Running => "Running",
                PluginStatus::Stopped => "Stopped",
                PluginStatus::Warning => "Warning",
                PluginStatus::Error => "Error",
                PluginStatus::Starting => "Starting",
                PluginStatus::Stopping => "Stopping",
                PluginStatus::Unknown => "Unknown",
            }
        }

        /// Returns true when the plugin is currently running.
        pub fn is_running(&self) -> bool {
            matches!(self, PluginStatus::Running)
        }

        /// Returns true when the plugin is in an error state.
        pub fn is_error(&self) -> bool {
            matches!(self, PluginStatus::Error)
        }

        /// Returns true when the plugin is transitioning between states.
        pub fn is_transitional(&self) -> bool {
            matches!(self, PluginStatus::Starting | PluginStatus::Stopping)
        }
    }

    /// Transport-specific status information for a plugin connection.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    pub enum TransportStatus {
        /// Transport is connected and working.
        Connected,
        /// Transport is disconnected.
        Disconnected,
        /// Transport is establishing a connection.
        Connecting,
        /// Transport encountered an error.
        Error,
        /// Transport is not applicable (for example, the plugin is stopped).
        NotApplicable,
    }

    impl TransportStatus {
        /// Returns a human-readable description of the transport status.
        pub fn display(&self) -> &'static str {
            match self {
                TransportStatus::Connected => "Connected",
                TransportStatus::Disconnected => "Disconnected",
                TransportStatus::Connecting => "Connecting",
                TransportStatus::Error => "Error",
                TransportStatus::NotApplicable => "N/A",
            }
        }

        /// Returns true when the transport is connected.
        pub fn is_connected(&self) -> bool {
            matches!(self, TransportStatus::Connected)
        }

        /// Returns true when the transport is in an error state.
        pub fn is_error(&self) -> bool {
            matches!(self, TransportStatus::Error)
        }
    }

    /// Aggregated health information for a plugin instance.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HealthStatus {
        /// Whether the plugin is healthy.
        pub healthy: bool,
        /// Timestamp of the last health check.
        pub last_check: Option<SystemTime>,
        /// When the plugin was last started.
        pub start_time: Option<SystemTime>,
        /// Optional handshake latency in milliseconds.
        pub handshake_latency: Option<u64>,
        /// Number of consecutive failures.
        pub failure_count: u32,
        /// Most recent error message.
        pub last_error: Option<String>,
        /// Transport-level status information.
        pub transport_status: TransportStatus,
    }

    impl Default for HealthStatus {
        fn default() -> Self {
            Self {
                healthy: false,
                last_check: None,
                start_time: None,
                handshake_latency: None,
                failure_count: 0,
                last_error: None,
                transport_status: TransportStatus::Disconnected,
            }
        }
    }

    impl HealthStatus {
        /// Creates a new health status with default values.
        pub fn new() -> Self {
            Self::default()
        }

        /// Marks the plugin as healthy and clears failure tracking.
        pub fn mark_healthy(&mut self) {
            self.healthy = true;
            self.failure_count = 0;
            self.last_error = None;
            self.last_check = Some(SystemTime::now());
        }

        /// Marks the plugin as unhealthy and records the associated error message.
        pub fn mark_unhealthy(&mut self, error_message: String) {
            self.healthy = false;
            self.failure_count += 1;
            self.last_error = Some(error_message);
            self.last_check = Some(SystemTime::now());
        }

        /// Returns true when the plugin is reporting a healthy status.
        pub fn is_healthy(&self) -> bool {
            self.healthy
        }

        /// Returns the duration since the plugin started, if known.
        pub fn uptime(&self) -> Option<Duration> {
            self.start_time.map(|start| start.elapsed().unwrap_or_default())
        }

        /// Returns the duration since the last health check, if known.
        pub fn time_since_last_check(&self) -> Option<Duration> {
            self.last_check.map(|timestamp| timestamp.elapsed().unwrap_or_default())
        }
    }

    /// Authentication status for a plugin.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum AuthStatus {
        /// Authentication status is unknown (not yet checked).
        #[default]
        Unknown,
        /// Plugin is successfully authenticated.
        Authorized,
        /// Authentication is required but not provided.
        Required,
        /// Authentication failed with an error.
        Failed,
    }

    impl fmt::Display for AuthStatus {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                AuthStatus::Unknown => write!(formatter, "Unknown"),
                AuthStatus::Authorized => write!(formatter, "Authorized"),
                AuthStatus::Required => write!(formatter, "Required"),
                AuthStatus::Failed => write!(formatter, "Failed"),
            }
        }
    }

    /// Detailed information about a plugin.
    /// Summary information describing a tool exposed by a plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct PluginToolSummary {
        /// Tool identifier returned by the MCP server.
        pub name: String,
        /// Optional human-friendly title supplied by the server.
        #[serde(default)]
        pub title: Option<String>,
        /// Optional description explaining the tool's behavior.
        #[serde(default)]
        pub description: Option<String>,
        /// Optional authentication summary supplied by the CLI.
        #[serde(default)]
        pub auth_summary: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PluginDetail {
        /// Plugin name.
        pub name: String,
        /// Current status of the plugin.
        pub status: PluginStatus,
        /// Command or base URL for the plugin.
        pub command_or_url: String,
        /// Optional arguments when using stdio transport.
        pub args: Option<String>,
        /// Environment variables supplied to the plugin.
        pub env: Vec<EnvVar>,
        /// Recent logs emitted by the plugin.
        pub logs: Vec<McpLogEntry>,
        /// Aggregated health metrics.
        pub health: HealthStatus,
        /// Tags associated with the plugin.
        pub tags: Vec<String>,
        /// Transport type used to communicate with the plugin (stdio, http, etc.).
        pub transport_type: String,
        /// Whether the plugin is currently enabled via configuration.
        pub enabled: bool,
        /// Last start time, if known.
        pub last_start: Option<DateTime<Utc>>,
        /// Handshake latency in milliseconds.
        pub handshake_latency: Option<u64>,
        /// Authentication status for the plugin.
        pub auth_status: AuthStatus,
        /// Number of tools currently exposed by this plugin.
        pub tool_count: usize,
        /// Summaries for tools currently exposed by this plugin.
        #[serde(default)]
        pub tools: Vec<PluginToolSummary>,
    }

    impl PluginDetail {
        /// Creates a new plugin detail record with default values for runtime data.
        pub fn new(name: String, command_or_url: String, args: Option<String>) -> Self {
            Self {
                name,
                status: PluginStatus::Stopped,
                command_or_url,
                args,
                env: Vec::new(),
                logs: Vec::new(),
                health: HealthStatus::default(),
                tags: Vec::new(),
                transport_type: String::new(),
                enabled: true,
                last_start: None,
                handshake_latency: None,
                auth_status: AuthStatus::default(),
                tool_count: 0,
                tools: Vec::new(),
            }
        }

        /// Adds a log entry to the plugin, retaining only the most recent 1,000 entries.
        pub fn add_log(&mut self, entry: McpLogEntry) {
            self.logs.push(entry);
            if self.logs.len() > 1000 {
                self.logs.remove(0);
            }
        }

        /// Returns the most recent `count` log entries.
        pub fn recent_logs(&self, count: usize) -> Vec<&McpLogEntry> {
            self.logs.iter().rev().take(count).collect()
        }

        /// Returns true when the plugin is currently running.
        pub fn is_running(&self) -> bool {
            matches!(self.status, PluginStatus::Running)
        }

        /// Returns true when the plugin is healthy and running.
        pub fn is_healthy(&self) -> bool {
            self.is_running() && self.health.is_healthy()
        }
    }

    impl fmt::Display for PluginDetail {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Plugin: {} | Status: {} | Command/URL: {} | Auth: {}",
                self.name,
                self.status.display(),
                self.command_or_url,
                self.auth_status,
            )?;

            if let Some(latency) = self.handshake_latency.or(self.health.handshake_latency) {
                write!(formatter, " | Handshake Latency: {latency}ms")?;
            }

            if !self.tags.is_empty() {
                write!(formatter, " | Tags: [{}]", self.tags.join(", "))?;
            }

            if let Some(error) = &self.health.last_error {
                write!(formatter, " | Last Error: {error}")?;
            }

            Ok(())
        }
    }

    /// Environment variable associated with a plugin.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EnvVar {
        /// Environment variable key.
        pub key: String,
        /// Environment variable value (masked for secrets).
        pub value: String,
        /// Source of the environment variable.
        pub source: EnvSource,
        /// Whether the value is effectively resolved.
        pub effective: bool,
    }

    impl EnvVar {
        /// Creates a new environment variable record.
        pub fn new(key: String, value: String, source: EnvSource) -> Self {
            Self {
                key,
                value,
                source,
                effective: true,
            }
        }

        /// Returns a masked version of the environment variable for display purposes.
        pub fn masked(&self) -> Self {
            let masked_value = if self.is_secret() {
                "••••••••••••••••".to_string()
            } else {
                self.value.clone()
            };

            Self {
                key: self.key.clone(),
                value: masked_value,
                source: self.source.clone(),
                effective: self.effective,
            }
        }

        /// Returns true when this environment variable contains a secret value.
        pub fn is_secret(&self) -> bool {
            matches!(self.source, EnvSource::Secret)
        }
    }

    /// Source of an environment variable.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub enum EnvSource {
        /// From the configuration file (plain text).
        File,
        /// From a secret stored in the keychain.
        Secret,
        /// From the process environment.
        Env,
        /// From a raw text value
        Raw,
    }

    impl fmt::Display for EnvSource {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                EnvSource::File => write!(formatter, "file"),
                EnvSource::Secret => write!(formatter, "secret"),
                EnvSource::Env => write!(formatter, "env"),
                EnvSource::Raw => write!(formatter, "raw"),
            }
        }
    }

    /// A log entry emitted by a plugin.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct McpLogEntry {
        /// Timestamp of the log entry.
        pub timestamp: DateTime<Utc>,
        /// Log level.
        pub level: LogLevel,
        /// Log message.
        pub message: String,
        /// Source of the log (stdout, stderr, or system).
        pub source: LogSource,
        /// Plugin name that generated this log.
        pub plugin_name: String,
    }

    impl McpLogEntry {
        /// Creates a new log entry using the current time.
        pub fn new(level: LogLevel, message: String, source: LogSource, plugin_name: String) -> Self {
            Self {
                timestamp: Utc::now(),
                level,
                message,
                source,
                plugin_name,
            }
        }

        /// Creates a system log entry with informational severity.
        pub fn system(message: String, plugin_name: String) -> Self {
            Self::new(LogLevel::Info, message, LogSource::System, plugin_name)
        }

        /// Creates an error log entry.
        pub fn error(message: String, source: LogSource, plugin_name: String) -> Self {
            Self::new(LogLevel::Error, message, source, plugin_name)
        }

        /// Formats the log entry for display.
        pub fn format(&self) -> String {
            format!(
                "[{}] {} {}: {}",
                self.timestamp.format("%H:%M:%S"),
                self.level,
                self.source,
                self.message
            )
        }
    }

    /// Log level for plugin logs.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub enum LogLevel {
        /// Diagnostic log.
        Debug,
        /// Informational log.
        Info,
        /// Warning log.
        Warn,
        /// Error log.
        Error,
    }

    impl fmt::Display for LogLevel {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                LogLevel::Debug => write!(formatter, "debug"),
                LogLevel::Info => write!(formatter, "info"),
                LogLevel::Warn => write!(formatter, "warn"),
                LogLevel::Error => write!(formatter, "err"),
            }
        }
    }

    /// Source of a log entry.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    pub enum LogSource {
        /// Standard output from the plugin.
        Stdout,
        /// Standard error from the plugin.
        Stderr,
        /// System-generated log.
        System,
    }

    impl fmt::Display for LogSource {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                LogSource::Stdout => write!(formatter, "stdout"),
                LogSource::Stderr => write!(formatter, "stderr"),
                LogSource::System => write!(formatter, "system"),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn plugin_detail_creation_populates_defaults() {
            let plugin = PluginDetail::new("test".to_string(), "node test.js".to_string(), None);
            assert_eq!(plugin.name, "test");
            assert_eq!(plugin.status, PluginStatus::Stopped);
            assert_eq!(plugin.command_or_url, "node test.js");
            assert!(plugin.logs.is_empty());
        }

        #[test]
        fn env_var_masking_obscures_secret_values() {
            let env_var = EnvVar::new("GITHUB_TOKEN".to_string(), "secret123".to_string(), EnvSource::Secret);
            let masked = env_var.masked();
            assert_eq!(masked.value, "••••••••••••••••");
            assert!(masked.is_secret());
        }

        #[test]
        fn log_entry_formatting_includes_level_source_and_message() {
            let log = McpLogEntry::new(LogLevel::Info, "Plugin started".to_string(), LogSource::System, "test".to_string());

            let formatted = log.format();
            assert!(formatted.contains("info"));
            assert!(formatted.contains("system"));
            assert!(formatted.contains("Plugin started"));
        }

        #[test]
        fn plugin_detail_display_formats_summary() {
            let mut plugin = PluginDetail::new("test".to_string(), "node test.js".to_string(), None);
            plugin.status = PluginStatus::Running;
            plugin.auth_status = AuthStatus::Authorized;
            plugin.handshake_latency = Some(42);
            plugin.tags = vec!["tag1".to_string(), "tag2".to_string()];
            plugin.health.last_error = Some("Something went wrong".to_string());

            let formatted = plugin.to_string();

            assert!(formatted.contains("Plugin: test"));
            assert!(formatted.contains("Status: Running"));
            assert!(formatted.contains("Command/URL: node test.js"));
            assert!(formatted.contains("Auth: Authorized"));
            assert!(formatted.contains("Handshake Latency: 42ms"));
            assert!(formatted.contains("Tags: [tag1, tag2]"));
            assert!(formatted.contains("Last Error: Something went wrong"));
        }

        #[test]
        fn plugin_status_icons_cover_primary_states() {
            assert_eq!(PluginStatus::Running.icon(), "✓");
            assert_eq!(PluginStatus::Stopped.icon(), "✗");
            assert_eq!(PluginStatus::Warning.icon(), "!");
            assert_eq!(PluginStatus::Error.icon(), "✗");
            assert_eq!(PluginStatus::Unknown.icon(), "?");
        }

        #[test]
        fn plugin_status_transitions_flagged_correctly() {
            assert!(PluginStatus::Running.is_running());
            assert!(!PluginStatus::Stopped.is_running());
            assert!(PluginStatus::Error.is_error());
            assert!(!PluginStatus::Running.is_error());
            assert!(PluginStatus::Starting.is_transitional());
            assert!(!PluginStatus::Running.is_transitional());
        }
    }
}

pub use command::*;
pub use execution::*;
pub use messaging::*;
pub use plugin::*;
pub use provider::*;
pub use service::*;
pub use suggestion::*;
pub use workflow::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_spec_round_trip_minimal() {
        let json = r#"{
            "name": "apps:list",
            "summary": "List apps",
            "execution": {
                "kind": "http",
                "method": "GET",
                "path": "/apps"
            }
        }"#;

        let spec: CommandSpec = serde_json::from_str(json).expect("deserialize CommandSpec");
        assert_eq!(spec.group, "");
        assert_eq!(spec.name, "apps:list");
        assert_eq!(spec.summary, "List apps");
        assert!(spec.positional_args.is_empty());
        assert!(spec.flags.is_empty());
        let http = spec.http().expect("http execution present");
        assert_eq!(http.method, "GET");
        assert_eq!(http.path, "/apps");

        let back = serde_json::to_string(&spec).expect("serialize CommandSpec");
        let spec2: CommandSpec = serde_json::from_str(&back).expect("round-trip deserialize");
        assert_eq!(spec2.name, spec.name);
        let http2 = spec2.http().expect("http execution present");
        assert_eq!(http2.method, http.method);
        assert_eq!(http2.path, http.path);
        assert_eq!(spec2.positional_args.len(), spec.positional_args.len());
        assert_eq!(spec2.flags.len(), 0);
    }

    #[test]
    fn command_spec_deserializes_mcp_variant() {
        let json = r#"{
            "group": "mcp.demo",
            "name": "demo:tool",
            "summary": "Needs OAuth — Run demo tool",
            "execution": {
                "kind": "mcp",
                "plugin_name": "demo-plugin",
                "tool_name": "demo_tool",
                "auth_summary": "Needs OAuth",
                "input_schema": "{\"type\":\"object\"}"
            }
        }"#;

        let spec: CommandSpec = serde_json::from_str(json).expect("deserialize MCP CommandSpec");
        assert_eq!(spec.group, "mcp.demo");
        assert_eq!(spec.name, "demo:tool");
        let mcp = spec.mcp().expect("mcp execution present");
        assert_eq!(mcp.plugin_name, "demo-plugin");
        assert_eq!(mcp.tool_name, "demo_tool");
        assert_eq!(mcp.auth_summary.as_deref(), Some("Needs OAuth"));
        assert!(mcp.render_hint.is_none());
    }

    #[test]
    fn command_execution_defaults_to_http_variant() {
        let spec = CommandSpec {
            group: String::new(),
            name: "apps:list".into(),
            summary: "List apps".into(),
            positional_args: Vec::new(),
            flags: Vec::new(),
            execution: CommandExecution::default(),
        };

        assert!(matches!(spec.execution(), CommandExecution::Http(_)));
        let http = spec.http().expect("http execution present");
        assert!(http.method.is_empty());
        assert!(http.path.is_empty());
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
