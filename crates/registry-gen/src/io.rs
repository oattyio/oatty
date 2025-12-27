use crate::openapi::transform_openapi_to_links;
use crate::schema::{derive_commands_from_schema, generate_commands};
use anyhow::{Context, Result};
use indexmap::{IndexMap, map::Entry as IndexMapEntry};
use postcard::to_stdvec;
use std::{
    fs,
    path::{Path, PathBuf},
};

use oatty_types::{
    CommandSpec, ServiceId,
    command::SchemaProperty,
    manifest::RegistryManifest,
    provider::{ProviderArgumentContract, ProviderContract, ProviderFieldContract, ProviderReturnContract},
    workflow::WorkflowDefinition,
};

pub struct ManifestInput {
    pub input: PathBuf,
    pub service_id: ServiceId,
}
/// Writes the command manifest to a file.
///
/// This function reads a JSON schema from the input path, generates commands,
/// encodes them using bincode, and writes the output to the specified path.
///
/// # Arguments
///
/// * `input` - Path to the input JSON schema file.
/// * `output` - Path to write the bincode-encoded manifest.
///
/// # Errors
///
/// Returns an error if file reading, directory creation, command generation,
/// encoding, or writing fails.
pub fn write_manifest(inputs: Vec<ManifestInput>, workflow_root: Option<PathBuf>, output: PathBuf) -> Result<()> {
    let all_commands = create_commands_from_input(inputs)?;
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    let provider_contracts = build_provider_contracts(&all_commands);
    let workflows = load_workflows(workflow_root.as_deref())?;
    let manifest = RegistryManifest {
        commands: all_commands,
        workflows,
        provider_contracts,
    };
    let bytes = to_stdvec(&manifest)?;
    fs::write(&output, &bytes)?;
    println!("wrote {} bytes to {}", bytes.len(), output.display());

    Ok(())
}

/// Writes the command manifest as JSON to a file.
///
/// This function mirrors `write_manifest` but serializes the generated commands
/// as JSON instead of bincode.
///
/// # Arguments
///
/// * `input` - Path to the input JSON schema file.
/// * `output` - Path to write the JSON manifest.
///
/// # Errors
///
/// Returns an error if file reading, directory creation, command generation,
/// encoding, or writing fails.
pub fn write_manifest_json(inputs: Vec<ManifestInput>, workflow_root: Option<PathBuf>, output: PathBuf) -> Result<()> {
    let all_commands = create_commands_from_input(inputs)?;
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    let provider_contracts = build_provider_contracts(&all_commands);
    let workflows = load_workflows(workflow_root.as_deref())?;
    let manifest = RegistryManifest {
        commands: all_commands,
        workflows,
        provider_contracts,
    };
    let json = serde_json::to_vec_pretty(&manifest).context("serialize manifest to json")?;
    fs::write(&output, &json)?;
    println!("wrote {} bytes (json) to {}", json.len(), output.display());
    Ok(())
}

/// Generates a list of `CommandSpec` from a vector of `ManifestInput`.
///
/// This function processes each `ManifestInput` item to derive commands.
/// An input may represent data in YAML or JSON format.
/// The function distinguishes between plain schemas and OpenAPI specifications,
/// applies transformations when necessary, and generates commands based on the inputs.
///
/// # Arguments
///
/// * `inputs` - A vector of `ManifestInput` instances, each containing the file path (or input location)
///   and a service ID.
///
/// # Returns
///
/// Returns `Ok(Vec<CommandSpec>)` if the inputs are successfully processed and the commands are derived.
/// Otherwise, it returns an `Err` with a description of the encountered error.
///
/// # Processing Steps:
///
/// 1. For each input, determine if the file is a YAML or JSON file using the helper function `is_yaml`.
/// 2. Read the file contents and apply transformations if the input follows the OpenAPI specification:
///   - Parse YAML into JSON-like structures using `serde_yaml::from_str`.
///   - Transform the document to include OpenAPI-style links, if applicable.
/// 3. Generate commands based on the schema or transformed OpenAPI specification using:
///   - `derive_commands_from_schema` for complex structured data.
///   - `generate_commands` for other types of schemas.
/// 4. All generated commands across inputs are collected into a vector and sorted by group and name.
///
/// # Errors
///
/// This function returns an error if any of the following operations fail:
/// - File reading (`fs::read_to_string`).
/// - Parsing YAML or JSON content.
/// - Transforming an OpenAPI document.
/// - Generating commands from the schema.
///
/// # Example
///
/// ```rust,ignore
/// let inputs = vec![
///     ManifestInput {
///         input: PathBuf::from("path/to/schema.yaml"),
///         service_id: "service1".to_string()
///     },
///     ManifestInput {
///         input: PathBuf::from("path/to/schema.json"),
///         service_id: "service2".to_string()
///     }
/// ];
/// let commands = create_commands_from_input(inputs).expect("Failed to create commands");
/// for command in commands {
///     println!("Command group: {}, name: {}", command.group, command.name);
/// }
/// ```rust,ignore
///
/// # Dependencies
///
/// The function relies on several helper functions and external dependencies:
/// * `fs::read_to_string`: Reads the schema from the file.
/// * `serde_yaml::from_str`: Parses YAML content into a JSON-like structure.
/// * `serde_json::from_str`: Parses a JSON string into a structured value.
/// * `transform_to_links_if_openapi`: Performs specific transformations for OpenAPI documents.
/// * `derive_commands_from_schema` and `generate_commands`: Derives command specifications from the input schema.
///
/// # Sorting
///
/// All generated commands are sorted by their group and name before returning.
fn create_commands_from_input(inputs: Vec<ManifestInput>) -> Result<Vec<CommandSpec>> {
    let mut all_commands = Vec::new();
    for input in inputs {
        let ManifestInput { input, service_id } = input;
        let commands = if is_yaml(&input) {
            let text = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            let doc: serde_json::Value = serde_yaml::from_str(&text).context("parse yaml as json value")?;
            let transformed = transform_to_links_if_openapi(&doc)?;
            derive_commands_from_schema(&transformed, service_id)?
        } else {
            let schema = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            if looks_like_openapi_json(&schema) {
                let doc: serde_json::Value = serde_json::from_str(&schema).context("parse json")?;
                let transformed = transform_to_links_if_openapi(&doc)?;
                derive_commands_from_schema(&transformed, service_id)?
            } else {
                generate_commands(&schema, service_id)?
            }
        };
        all_commands.extend(commands);
    }
    all_commands.sort_by_key(|c| (c.group.clone(), c.name.clone()));
    Ok(all_commands)
}
/// Builds a collection of provider contracts from a list of command specifications.
///
/// This function iterates over the given list of `CommandSpec` objects, constructing
/// a `ProviderContract` for each command by processing its arguments and return values.
/// A unique `command_id` is generated for each command based on its group and name,
/// and only commands with non-empty argument or return contracts are included in the
/// resulting collection.
///
/// # Arguments
/// - `commands`: A slice of `CommandSpec` instances representing the command specifications
///   for which provider contracts need to be created.
///
/// # Returns
/// - A vector of `ProviderContractEntry` objects, each containing a `command_id` and its
///   associated `ProviderContract`.
///
/// # Implementation Details
/// - Uses an `IndexMap` to ensure the iteration order of contracts matches their insertion
///   order.
/// - Calls helper functions `build_argument_contracts` and `build_return_contract` to
///   populate the argument and return value specifications for each command.
/// - Filters out commands that do not have any arguments or return fields in their respective
///   contracts.
///
/// # Dependencies
/// - This function requires the `IndexMap` crate for maintaining the order of inserted items.
/// - It assumes the existences of `CommandSpec`, `ProviderContract`, and `ProviderContractEntry`
///   structs, as well as the `build_argument_contracts` and `build_return_contract` functions.
///
/// # Example
/// ```rust,ignore
/// let commands = vec![
///     CommandSpec { group: "group1", name: "command1" },
///     CommandSpec { group: "group2", name: "command2" }
/// ];
///
/// let provider_contracts = build_provider_contracts(&commands);
/// assert!(!provider_contracts.is_empty());
/// ```
fn build_provider_contracts(commands: &[CommandSpec]) -> IndexMap<String, ProviderContract> {
    let mut contracts = IndexMap::new();

    for command in commands {
        // Use canonical colon-separated identifier ("group:name") for provider contracts
        let command_id = format!("{}:{}", command.group, command.name);

        let mut contract = ProviderContract::default();
        build_argument_contracts(command, &mut contract);
        build_return_contract(command, &mut contract);

        if !contract.arguments.is_empty() || !contract.returns.fields.is_empty() {
            contracts.insert(command_id, contract);
        }
    }

    contracts
}
/// Populates the `arguments` field of a `ProviderContract` based on the provided `CommandSpec`.
///
/// This function processes the HTTP path placeholders and required flags of the given
/// `CommandSpec` to determine the necessary arguments for the provider's contract. Placeholders
/// and required flags are recorded in insertion order using an [`IndexMap`], ensuring duplicate
/// names are coalesced while preserving stable ordering in the resulting metadata. Each entry is
/// converted into a `ProviderArgumentContract` with semantic tag preferences derived from the
/// argument name.
///
/// # Arguments
///
/// * `command` - A reference to a `CommandSpec` instance that contains information about the
///   command's HTTP path and flags (including their requirements).
///
/// * `contract` - A mutable reference to a `ProviderContract` where the generated argument
///   contracts will be stored.
///
/// # Details
///
/// - Extracts placeholders from the HTTP path using `extract_path_placeholders`, and adds them
///   to the list of argument names.
/// - Adds the names of any required flags from the `CommandSpec` to the list.
/// - Converts these names into `ProviderArgumentContract` objects layered with semantic tag
///   expectations derived from the argument name (for example, `app` → accepts `app_id`,
///   `app_name`).
///
/// # Behavior
///
/// The resulting `ProviderArgumentContract` objects in the `contract.arguments` field reflect
/// all required arguments for the specified command, ensuring both placeholders in the HTTP
/// path and mandatory flags are considered. Each contract carries a set of accepted semantic
/// tags and an optional preferred tag to guide downstream auto-mapping heuristics.
///
/// # Example
///
/// ```rust,ignore
/// let command_spec = CommandSpec::from_config(...);
/// let mut provider_contract = ProviderContract::default();
///
/// build_argument_contracts(&command_spec, &mut provider_contract);
///
/// assert!(!provider_contract.arguments.is_empty());
/// ```
fn build_argument_contracts(command: &CommandSpec, contract: &mut ProviderContract) {
    let mut descriptors: IndexMap<String, ProviderArgumentContract> = IndexMap::new();

    if let Some(http) = command.http() {
        for placeholder in extract_path_placeholders(&http.path) {
            upsert_argument_contract(&mut descriptors, &placeholder, true);
        }
    }

    for flag in &command.flags {
        if flag.required {
            upsert_argument_contract(&mut descriptors, &flag.name, true);
        }
    }

    contract.arguments = descriptors.into_iter().map(|(_, descriptor)| descriptor).collect();
}

fn upsert_argument_contract(descriptors: &mut IndexMap<String, ProviderArgumentContract>, argument_name: &str, required: bool) {
    match descriptors.entry(argument_name.to_string()) {
        IndexMapEntry::Occupied(mut entry) => {
            if required {
                entry.get_mut().required = true;
            }
        }
        IndexMapEntry::Vacant(entry) => {
            entry.insert(create_argument_contract(argument_name, required));
        }
    }
}

fn create_argument_contract(argument_name: &str, required: bool) -> ProviderArgumentContract {
    let (accepts, prefer) = argument_accepts_and_preference(argument_name);
    ProviderArgumentContract {
        name: argument_name.to_string(),
        accepts,
        prefer,
        required,
    }
}

fn argument_accepts_and_preference(argument_name: &str) -> (Vec<String>, Option<String>) {
    let normalized = normalize_argument_key(argument_name);
    match normalized.as_str() {
        "app" => accepts_with_preference(&["app_id", "app_name"], Some("app_id")),
        "app_id" => accepts_with_preference(&["app_id"], Some("app_id")),
        "addon" => accepts_with_preference(&["addon_id", "addon_name"], Some("addon_id")),
        "addon_id" => accepts_with_preference(&["addon_id"], Some("addon_id")),
        "pipeline" => accepts_with_preference(&["pipeline_id", "pipeline_name", "pipeline_slug"], Some("pipeline_id")),
        "pipeline_id" => accepts_with_preference(&["pipeline_id"], Some("pipeline_id")),
        "team" => accepts_with_preference(&["team_id", "team_name"], Some("team_id")),
        "team_name" => accepts_with_preference(&["team_name"], Some("team_name")),
        "space" => accepts_with_preference(&["space_id", "space_name"], Some("space_id")),
        "space_id" => accepts_with_preference(&["space_id"], Some("space_id")),
        "region" => accepts_with_preference(&["region_slug", "region", "region_name"], Some("region_slug")),
        "stack" => accepts_with_preference(&["stack_name", "stack"], Some("stack_name")),
        "user" | "collaborator" => accepts_with_preference(&["user_email", "user_id"], Some("user_email")),
        "database" => accepts_with_preference(&["database_id", "database_name"], Some("database_id")),
        other => fallback_argument_accepts(other),
    }
}

fn fallback_argument_accepts(argument_key: &str) -> (Vec<String>, Option<String>) {
    if let Some(stripped) = argument_key.strip_suffix("_id") {
        let tag = format!("{}_id", stripped);
        return (vec![tag.clone()], Some(tag));
    }
    if let Some(stripped) = argument_key.strip_suffix("_slug") {
        let tag = format!("{}_slug", stripped);
        return (vec![tag.clone()], Some(tag));
    }
    if let Some(stripped) = argument_key.strip_suffix("_name") {
        let tag = format!("{}_name", stripped);
        return (vec![tag.clone()], Some(tag));
    }
    if argument_key == "id" {
        return (vec!["id".to_string()], Some("id".to_string()));
    }
    (Vec::new(), None)
}

fn accepts_with_preference(tags: &[&str], preferred: Option<&str>) -> (Vec<String>, Option<String>) {
    let accepts = tags.iter().map(|tag| tag.to_string()).collect::<Vec<_>>();
    let prefer = preferred.map(|tag| tag.to_string());
    (accepts, prefer)
}

fn normalize_argument_key(argument_name: &str) -> String {
    let without_indices = argument_name.replace("[]", "");
    let segment = without_indices
        .rsplit_once('.')
        .map(|(_, trailing)| trailing)
        .unwrap_or(&without_indices);
    segment.replace('-', "_").to_ascii_lowercase()
}
/// Builds and assigns a return contract for a given provider contract based on the HTTP output schema specified in the command.
///
/// This function takes a `CommandSpec` and a mutable reference to a `ProviderContract`.
/// It extracts the HTTP details from the command and derives the return contract
/// by converting the HTTP output schema to a structured return contract representation.
/// If the resulting return contract contains fields, it is assigned to the `returns`
/// field of the provider contract.
///
/// # Parameters
/// - `command`: A reference to the `CommandSpec` containing the input command details,
///   including the HTTP-related data and output schema.
/// - `contract`: A mutable reference to the `ProviderContract` to which the return
///   contract will be assigned.
///
/// # Behavior
/// - If the `command` does not include HTTP configurations, the function exits early without modifying the `contract`.
/// - If the `command` includes an HTTP output schema, it is processed and converted to a return contract structure using the
///   `convert_schema_to_return_contract` function.
/// - The converted return contract is checked, and if it contains fields, it is assigned to the `returns` field of the provided `contract`.
/// - If the return contract is empty (no fields), the `contract` remains unchanged.
///
/// # Example
/// ```ignore
/// let command = CommandSpec::new(); // Assume CommandSpec is defined elsewhere
/// let mut contract = ProviderContract::new(); // Assume ProviderContract is defined elsewhere
/// build_return_contract(&command, &mut contract);
/// ```
///
/// # Notes
/// - This function assumes that the `CommandSpec` and `ProviderContract` types, along with
///   the `convert_schema_to_return_contract` function, are defined and accessible within the same context.
/// - The function does not explicitly handle errors or edge cases beyond checking for an existing HTTP configuration
///   and the presence of fields in the derived return contract.
///
/// # Related
/// - [`convert_schema_to_return_contract`]: Processes schema to derive a return contract structure.
fn build_return_contract(command: &CommandSpec, contract: &mut ProviderContract) {
    let Some(http) = command.http() else {
        return;
    };

    let schema = http.output_schema.as_ref();
    let return_contract = convert_schema_to_return_contract(schema);
    if !return_contract.fields.is_empty() {
        contract.returns = return_contract;
    }
}
/// Converts an optional `SchemaProperty` into a `ProviderReturnContract`.
///
/// This function processes the provided schema, extracting relevant data
/// to construct a `ProviderReturnContract`. The schema is expected to
/// describe the structure of an object or an array. If there is no schema
/// provided, the function returns a default `ProviderReturnContract`.
///
/// # Parameters
/// - `schema`: An optional reference to a `SchemaProperty` that
///   contains the schema to be converted.
///
/// # Returns
/// - A `ProviderReturnContract` representing the converted schema.
///
/// # Behavior
/// - If the `schema` is an object (`type` is "object"):
///   - Iterates through the schema's properties, sorts them by key,
///     and adds each property as a `ProviderFieldContract` to the
///     `fields` of the `ProviderReturnContract`.
/// - If the `schema` is an array (`type` is "array"):
///   - Processes the `items` schema recursively, converting it
///     to a `ProviderReturnContract`. If the nested contract
///     contains fields, it overrides the current contract.
/// - If no schema type is specified or recognized, an empty
///   default `ProviderReturnContract` is returned.
///
/// # Example
/// ```rust,ignore
/// let schema = Some(&SchemaProperty {
///     r#type: "object".to_string(),
///     properties: Some(vec![
///         ("field1".to_string(), Box::new(SchemaProperty {
///             r#type: "string".to_string(),
///             tags: vec!["tag1".to_string()],
///             ..Default::default()
///         })),
///         ("field2".to_string(), Box::new(SchemaProperty {
///             r#type: "integer".to_string(),
///             ..Default::default()
///         })),
///     ].into_iter().collect()),
///     ..Default::default()
/// });
///
/// let return_contract = convert_schema_to_return_contract(schema);
/// assert_eq!(return_contract.fields.len(), 2);
/// assert_eq!(return_contract.fields[0].name, "field1");
/// assert_eq!(return_contract.fields[0].r#type, Some("string".to_string()));
/// assert_eq!(return_contract.fields[1].name, "field2");
/// assert_eq!(return_contract.fields[1].r#type, Some("integer".to_string()));
/// ```
///
/// # Notes
/// - Preserves the order of fields by sorting them alphabetically based on
///   their names within an object schema.
/// - Requires the resulting `ProviderFieldContract` names and types
///   to match the schema field definitions.
fn convert_schema_to_return_contract(schema: Option<&SchemaProperty>) -> ProviderReturnContract {
    let mut return_contract = ProviderReturnContract::default();
    let Some(root) = schema else {
        return return_contract;
    };

    match root.r#type.as_str() {
        "object" => {
            if let Some(properties) = &root.properties {
                let mut entries: Vec<_> = properties.iter().map(|(name, property)| (name, property.as_ref())).collect();
                entries.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));
                for (name, property) in entries {
                    return_contract.fields.push(ProviderFieldContract {
                        name: name.clone(),
                        r#type: Some(property.r#type.clone()),
                        tags: property.tags.clone(),
                    });
                }
            }
        }
        "array" => {
            if let Some(item_schema) = root.items.as_deref() {
                let nested = convert_schema_to_return_contract(Some(item_schema));
                if !nested.fields.is_empty() {
                    return_contract = nested;
                }
            }
        }
        _ => {}
    }

    return_contract
}
/// Extracts and returns a list of placeholders from a given URL path.
///
/// This function identifies placeholders in the path string, which are enclosed
/// in curly braces `{}`. It strips leading and trailing slashes, splits the path into segments,
/// identifies segments that are placeholders, and returns a vector of the placeholders' names.
///
/// # Arguments
///
/// * `path` - A string slice that represents the URL path to extract placeholders from.
///
/// # Returns
///
/// A `Vec<String>` containing all the placeholder names found in the URL path. Each placeholder
/// will be stripped of its enclosing curly braces and trimmed of any extra whitespace. If no
/// placeholders are found, an empty vector is returned.
///
/// # Examples
///
/// ```ignore
/// let path = "/users/{user_id}/posts/{post_id}";
/// let placeholders = extract_path_placeholders(path);
/// assert_eq!(placeholders, vec!["user_id", "post_id"]);
///
/// let path_without_placeholders = "/users/posts";
/// let placeholders = extract_path_placeholders(path_without_placeholders);
/// assert!(placeholders.is_empty());
///
/// let path_with_empty_placeholder = "/users/{ }/posts";
/// let placeholders = extract_path_placeholders(path_with_empty_placeholder);
/// assert!(placeholders.is_empty());
/// ```
///
/// # Notes
///
/// * Placeholders are valid only if they are enclosed in `{}` and contain at least
///   one non-whitespace character.
/// * Path segments not enclosed in `{}` are ignored.
///
/// # Edge Cases
///
/// * A segment like `{}` or `{ }` will not be considered a valid placeholder.
/// * Leading and trailing slashes in the path are ignored during processing.
///
/// # Panics
///
/// This function does not panic under normal circumstances.
fn extract_path_placeholders(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter_map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                let placeholder = segment.trim_start_matches('{').trim_end_matches('}').trim();
                if placeholder.is_empty() {
                    None
                } else {
                    Some(placeholder.to_string())
                }
            } else {
                None
            }
        })
        .collect()
}

/// Determines if the given file path has a YAML file extension.
///
/// # Arguments
///
/// * `path` - A reference to a `Path` object representing the file path to check.
///
/// # Returns
///
/// * `true` if the file path has an extension of "yaml" or "yml".
/// * `false` if the file path does not have an extension or if the extension is not "yaml" or "yml".
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
///
/// let path = Path::new("config.yaml");
/// assert_eq!(is_yaml(path), true);
///
/// let path = Path::new("config.json");
/// assert_eq!(is_yaml(path), false);
///
/// let path = Path::new("no_extension");
/// assert_eq!(is_yaml(path), false);
/// ```ignore
fn is_yaml(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(ext, "yaml" | "yml"),
        None => false,
    }
}
/// Determines if a given string appears to represent an OpenAPI or Swagger JSON document.
///
/// This function performs lightweight detection by checking if the input string contains
/// either the `"openapi"` or `"swagger"` keywords. It does not fully parse the JSON structure
/// and is intended as a quick heuristic for identifying potential OpenAPI/Swagger content.
///
/// # Arguments
///
/// * `s` - A string slice that potentially contains an OpenAPI or Swagger JSON document.
///
/// # Returns
///
/// * `true` if the string contains the keywords `"openapi"` or `"swagger"`, indicating that
///   it likely represents an OpenAPI or Swagger document.
/// * `false` otherwise.
///
/// # Examples
///
/// ```text
/// let valid_openapi = r#"{"openapi": "3.0.0"}"#;
/// let valid_swagger = r#"{"swagger": "2.0"}"#;
/// let invalid_json = r#"{"title": "Not OpenAPI"}"#;
///
/// assert!(looks_like_openapi_json(valid_openapi));
/// assert!(looks_like_openapi_json(valid_swagger));
/// assert!(!looks_like_openapi_json(invalid_json));
/// ```text
fn looks_like_openapi_json(s: &str) -> bool {
    // Lightweight detection to avoid parsing twice
    s.contains("\"openapi\"") || s.contains("\"swagger\"")
}
/// Transforms a given JSON document into a hyper-schema-like structure with links if it is identified as an OpenAPI document.
///
/// # Description
/// This function checks if the given JSON document corresponds to an OpenAPI specification.
/// If the document contains an "openapi" field, it is assumed to be an OpenAPI v3 document.
/// The function will then transform the OpenAPI document into a hyper-schema-like format using the `transform_openapi_to_links` function.
/// If the "openapi" field is not present, the function assumes the input is already in a hyper-schema-like format and returns it unchanged.
///
/// # Parameters
/// - `doc`: A reference to a `serde_json::Value` that represents the input JSON document.
///
/// # Returns
/// - On success, returns a `Result<serde_json::Value>` containing the transformed document if it is OpenAPI,
///   or the original document if it is not OpenAPI.
/// - On failure, returns an error from the underlying transformation function (`transform_openapi_to_links`).
///
/// # Notes
/// - Only OpenAPI v3 is supported currently. OpenAPI v2 support can be added in the future.
/// - In the absence of the "openapi" field, no transformation is performed.
///
/// # Errors
/// This function may return an error if the `transform_openapi_to_links` function fails during the transformation process.
///
/// # Examples
/// ```ignore
/// use serde_json::json;
///
/// let openapi_doc = json!({
///     "openapi": "3.0.0",
///     "info": { "title": "Sample API", "version": "1.0.0" },
///     "paths": {}
/// });
///
/// let result = transform_to_links_if_openapi(&openapi_doc);
///
/// assert!(result.is_ok());
/// ```text
///
/// ```ignore
/// use serde_json::json;
///
/// let non_openapi_doc = json!({
///     "title": "Example Schema",
///     "type": "object"
/// });
///
/// let result = transform_to_links_if_openapi(&non_openapi_doc);
///
/// assert!(result.is_ok());
/// assert_eq!(result.unwrap(), non_openapi_doc);
/// ```text
fn transform_to_links_if_openapi(doc: &serde_json::Value) -> Result<serde_json::Value> {
    // For now we support OpenAPI v3 only; v2 can be added later
    if doc.get("openapi").is_some() {
        transform_openapi_to_links(doc)
    } else {
        // Not OpenAPI; assume it's already hyper-schema-like
        Ok(doc.clone())
    }
}
/// Loads workflow definitions from the specified directory.
///
/// # Arguments
///
/// * `workflow_root` - An optional reference to a `Path` that specifies the root directory
///   containing workflow definition files. It can be `None`, in which case an empty list
///   of workflows is returned.
///
/// # Returns
///
/// A `Result` containing a vector of `WorkflowDefinition` objects if the operation succeeds.
/// If the directory does not exist or is empty, an empty vector is returned. If any error
/// occurs (e.g., reading files, parsing YAML/JSON), the function returns an error with an
/// appropriate context.
///
/// # Workflow File Expectations
///
/// * The directory is expected to contain workflow definition files in either YAML or JSON format.
/// * The function determines the format of the file based on its extension and parses
///   it accordingly.
/// * Files must be valid YAML or JSON and conform to the structure of `WorkflowDefinition`.
///
/// # Errors
///
/// This function returns an error if:
/// * A file in the directory cannot be read.
/// * A file fails to parse as either YAML or JSON.
/// * Any other I/O-related issue occurs while processing the directory or its files.
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
///
/// let workflow_root = Some(Path::new("./workflows"));
/// match load_workflows(workflow_root) {
///     Ok(workflows) => {
///         for workflow in workflows {
///             println!("Loaded workflow: {:?}", workflow);
///         }
///     }
///     Err(e) => eprintln!("Failed to load workflows: {}", e),
/// }
/// ```ignore
///
/// # Implementation Details
///
/// 1. If `workflow_root` is `None` or the directory does not exist, the function returns an empty `Vec<WorkflowDefinition>`.
/// 2. Workflow files in the root directory are collected using `collect_workflow_files`.
/// 3. The files are sorted to ensure deterministic order.
/// 4. Each file's content is read, and its format is inferred based on the file extension:
///    - `.yaml` or `.yml` files are parsed as YAML.
///    - Other files (e.g., `.json`) are parsed as JSON.
/// 5. The parsed workflows are sorted by the `workflow` field before being returned.
///
/// # Dependencies
///
/// This function relies on:
/// * File I/O operations from `std::fs`.
/// * Error handling using the `anyhow` crate for context and error propagation.
/// * YAML and JSON parsing using the `serde_yaml` and `serde_json` crates, respectively.
///
/// # Notes
///
/// The function assumes that `collect_workflow_files` is a helper function that collects
/// all relevant workflow files in the directory and appends their paths to the provided
/// `files` vector.
///
fn load_workflows(workflow_root: Option<&Path>) -> Result<Vec<WorkflowDefinition>> {
    let Some(root) = workflow_root else {
        return Ok(Vec::new());
    };
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_workflow_files(root, &mut files)?;

    files.sort();

    let mut workflows = Vec::with_capacity(files.len());
    for path in files {
        let content = fs::read_to_string(&path).with_context(|| format!("read workflow {}", path.display()))?;
        let workflow: WorkflowDefinition = if is_yaml(&path) {
            serde_yaml::from_str(&content).with_context(|| format!("parse workflow yaml {}", path.display()))?
        } else {
            serde_json::from_str(&content).with_context(|| format!("parse workflow json {}", path.display()))?
        };
        workflows.push(workflow);
    }

    workflows.sort_by(|a, b| a.workflow.cmp(&b.workflow));
    Ok(workflows)
}
/// Recursively collects workflow files from a given directory and its subdirectories.
///
/// This function traverses the directory tree starting at the specified root path,
/// looking for files that satisfy the `should_ingest_workflow` condition. All such
/// files are added to the provided vector, `files`. Errors encountered during
/// directory traversal or file inspection are returned as error results with
/// context information.
///
/// # Arguments
///
/// * `root` - A reference to the root path where the directory traversal will begin.
/// * `files` - A mutable reference to a vector that will be populated with the paths
///   of workflow files that satisfy the condition defined in `should_ingest_workflow`.
///
/// # Returns
///
/// A `Result` indicating success or failure:
/// * On success, returns `Ok(())`.
/// * On failure, returns a `Result::Err` with detailed context on the failure.
///
/// # Errors
///
/// This function may return errors in the following cases:
/// * The root directory cannot be read due to insufficient permissions or other file
///   system issues.
/// * Encountering issues while reading or inspecting files in the directory, such as
///   a failed call to retrieve metadata or traverse subdirectories.
///
/// # Example
///
/// ```rust,ignore
/// use std::path::Path;
/// use std::vec::Vec;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let root = Path::new("/path/to/workflows");
///     let mut workflow_files = Vec::new();
///
///     collect_workflow_files(&root, &mut workflow_files)?;
///
///     for file in workflow_files {
///         println!("Workflow file found: {}", file.display());
///     }
///
///     Ok(())
/// }
/// ```rust,ignore
///
/// # Note
///
/// The function assumes the existence of the helper function `should_ingest_workflow`,
/// which determines whether a file should be included in the workflow files collection.
fn collect_workflow_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read workflow dir {}", root.display()))? {
        let entry = entry.with_context(|| format!("walk {}", root.display()))?;
        let path = entry.path();
        if entry.file_type().with_context(|| format!("inspect {}", path.display()))?.is_dir() {
            collect_workflow_files(&path, files)?;
        } else if should_ingest_workflow(&path) {
            files.push(path);
        }
    }
    Ok(())
}
/// Determines whether a file at the given path should be ingested as a workflow.
///
/// # Arguments
/// * `path` - A reference to a [`Path`] representing the file's path.
///
/// # Returns
/// * `true` if the file extension matches "yaml", "yml", or "json".
/// * `false` if the file has no extension or the extension does not match the accepted types.
///
/// # Examples
/// ```rust,ignore
/// use std::path::Path;
///
/// let workflow_path = Path::new("workflow.yaml");
/// assert!(should_ingest_workflow(&workflow_path));
///
/// let invalid_path = Path::new("document.txt");
/// assert!(!should_ingest_workflow(&invalid_path));
///
/// let no_extension_path = Path::new("README");
/// assert!(!should_ingest_workflow(&no_extension_path));
/// ```
fn should_ingest_workflow(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(ext, "yaml" | "yml" | "json"),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn should_ingest_workflow_recognizes_supported_extensions() {
        assert!(should_ingest_workflow(Path::new("a.yaml")));
        assert!(should_ingest_workflow(Path::new("a.yml")));
        assert!(should_ingest_workflow(Path::new("a.json")));
        assert!(!should_ingest_workflow(Path::new("a.txt")));
        assert!(!should_ingest_workflow(Path::new("README")));
    }

    #[test]
    fn is_yaml_detects_yaml_and_yml() {
        assert!(is_yaml(Path::new("workflow.yaml")));
        assert!(is_yaml(Path::new("workflow.yml")));
        assert!(!is_yaml(Path::new("workflow.json")));
        assert!(!is_yaml(Path::new("workflow")));
    }

    #[test]
    fn extract_path_placeholders_happy_and_edge_cases() {
        let path = "/apps/{app_id}/addons/{addon_id}";
        let placeholders = extract_path_placeholders(path);
        assert_eq!(placeholders, vec!["app_id", "addon_id"]);

        let path_no_ph = "/apps/list";
        assert!(extract_path_placeholders(path_no_ph).is_empty());

        let path_with_empty = "/apps/{ }/addons/{}";
        assert!(extract_path_placeholders(path_with_empty).is_empty());
    }

    #[test]
    fn looks_like_openapi_json_detects_keywords() {
        assert!(looks_like_openapi_json("{\"openapi\": \"3.0.0\"}"));
        assert!(looks_like_openapi_json("{\"swagger\": \"2.0\"}"));
        assert!(!looks_like_openapi_json("{\"title\": \"nope\"}"));
    }

    fn make_temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        dir.push(format!("io_rs_tests_{}", nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_workflows_reads_yaml_and_json_recursively_and_sorts() -> Result<()> {
        let root = make_temp_dir();

        // Create nested directories
        let nested = root.join("nested");
        fs::create_dir_all(&nested)?;

        // YAML workflow
        let yaml_path = root.join("a_workflow.yaml");
        let mut yaml_file = fs::File::create(&yaml_path)?;
        writeln!(yaml_file, "workflow: app_with_db\ntitle: App with DB\n")?;

        // JSON workflow
        let json_path = nested.join("b_workflow.json");
        let mut json_file = fs::File::create(&json_path)?;
        write!(
            json_file,
            "{}",
            serde_json::json!({
                "workflow": "backup_db",
                "steps": []
            })
        )?;

        let mut workflows = load_workflows(Some(&root))?;
        // Expect two workflows, sorted by `workflow` field
        assert_eq!(workflows.len(), 2);
        workflows.sort_by(|a, b| a.workflow.cmp(&b.workflow));
        assert_eq!(workflows[0].workflow, "app_with_db");
        assert_eq!(workflows[1].workflow, "backup_db");
        Ok(())
    }

    #[test]
    fn write_manifest_json_writes_file_with_workflows_only() -> Result<()> {
        // Set up temp output and workflows; skip schema generation by providing an empty inputs vec
        let dir = make_temp_dir();
        let out_path = dir.join("manifest.json");

        // Create a simple workflow dir
        let wf_dir = dir.join("workflows");
        fs::create_dir_all(&wf_dir)?;
        fs::write(wf_dir.join("x.yaml"), "workflow: test\ndescription: test workflow\n")?;

        write_manifest_json(vec![], Some(wf_dir.clone()), out_path.clone())?;

        // Verify the file exists and is non-empty
        let bytes = fs::read(&out_path)?;
        assert!(!bytes.is_empty());

        // Verify JSON structure has a workflows array
        let v: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert!(v.get("workflows").and_then(|w| w.as_array()).is_some());
        Ok(())
    }
}
