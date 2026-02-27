use crate::openapi::{collect_base_urls_from_document, derive_commands_from_openapi, derive_vendor_from_document};
use anyhow::{Context, Result};
use heck::ToSnakeCase;
use indexmap::{IndexMap, map::Entry as IndexMapEntry};
use postcard::to_stdvec;
use std::{fs, path::PathBuf};

use oatty_types::{
    CommandSpec,
    command::SchemaProperty,
    manifest::{RegistryCatalog, RegistryManifest},
    provider::{ProviderArgumentContract, ProviderContract, ProviderFieldContract, ProviderReturnContract},
};

/// Input descriptor for a registry generation pass.
///
/// Each entry identifies an OpenAPI document to parse into `CommandSpec` entries.
#[derive(Debug, Clone)]
pub struct ManifestInput {
    /// Path to the OpenAPI document.
    pub file_path: Option<PathBuf>,
    /// Local content of the OpenAPI document.
    pub local: Option<String>,
    /// Prefix override for commands. If None, it defaults to the vendor name.
    pub prefix_override: Option<String>,
}

impl ManifestInput {
    pub fn new(file_path: Option<PathBuf>, local: Option<String>, prefix_override: Option<String>) -> Self {
        Self {
            file_path,
            local,
            prefix_override,
        }
    }

    /// Reads the input descriptor by consuming self
    /// and returning the contents as a String.
    pub fn take_contents(mut self) -> Result<String> {
        if let Some(file_path) = &self.file_path {
            let contents = fs::read_to_string(file_path).with_context(|| format!("read {}", file_path.display()))?;
            return Ok(contents);
        }
        if let Some(local) = self.local.take() {
            return Ok(local);
        }
        Err(anyhow::anyhow!("No file path or local content provided"))
    }
}

/// Generates a registry catalog from a manifest input.
///
/// This function reads OpenAPI documents from the input paths, generates
/// commands, and constructs a `RegistryCatalog` with the generated commands
/// and provider contracts.
pub fn generate_catalog(mut input: ManifestInput) -> Result<RegistryCatalog> {
    let prefix_override = input.prefix_override.take();
    let document = parse_openapi_document(input)?;
    let (name, commands) = create_commands_from_document(&document, prefix_override)?;
    let provider_contracts = build_provider_contracts(&commands);
    let base_urls = collect_base_urls_from_document(&document);
    let manifest = RegistryManifest {
        commands,
        provider_contracts,
        vendor: name,
    };
    let description = document
        .pointer("/info/description")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let title = document
        .pointer("/info/title")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    Ok(RegistryCatalog {
        title,
        description,
        vendor: Some(manifest.vendor.clone()),
        base_urls,
        base_url_index: 0,
        manifest: Some(manifest),
        ..Default::default()
    })
}

/// Generates a registry manifest from a list of input descriptors.
///
/// This function reads OpenAPI documents from the input paths, generates
/// commands, and constructs a `RegistryManifest` with the generated commands
/// and provider contracts.
///
/// # Arguments
///
/// * `inputs` - OpenAPI document paths to read.
///
/// # Errors
///
/// Returns an error if file reading, command generation, or provider contract
/// construction fails.
pub fn generate_manifest(mut input: ManifestInput) -> Result<RegistryManifest> {
    let prefix_override = input.prefix_override.take();
    let document = parse_openapi_document(input)?;
    let (vendor, commands) = create_commands_from_document(&document, prefix_override)?;
    let provider_contracts = build_provider_contracts(&commands);
    Ok(RegistryManifest {
        commands,
        provider_contracts,
        vendor,
    })
}

/// Builds provider contracts for an already-materialized command list.
///
/// This helper is used by catalog mutation paths that replace command
/// specifications without re-generating the full catalog from OpenAPI.
pub fn build_provider_contracts_for_commands(commands: &[CommandSpec]) -> IndexMap<String, ProviderContract> {
    build_provider_contracts(commands)
}

/// Writes the command manifest to a file.
///
/// This function reads OpenAPI documents from the input paths, generates
/// commands, encodes them using bincode, and writes the output to the
/// specified path.
///
/// # Arguments
///
/// * `inputs` - OpenAPI document paths to read.
/// * `output` - Path to write the bincode-encoded manifest.
///
/// # Errors
///
/// Returns an error if file reading, directory creation, command generation,
/// encoding, or writing fails.
pub fn write_manifest(input: ManifestInput, output: PathBuf) -> Result<()> {
    let manifest = generate_manifest(input)?;
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }

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
/// * `inputs` - OpenAPI document paths to read.
/// * `output` - Path to write the JSON manifest.
///
/// # Errors
///
/// Returns an error if file reading, directory creation, command generation,
/// encoding, or writing fails.
pub fn write_manifest_json(input: ManifestInput, output: PathBuf) -> Result<()> {
    let manifest = generate_manifest(input)?;
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(&manifest).context("serialize manifest to json")?;
    fs::write(&output, &json)?;
    println!("wrote {} bytes (json) to {}", json.len(), output.display());
    Ok(())
}

/// Generates a list of `CommandSpec` from a vector of `ManifestInput`.
///
/// This function processes each OpenAPI input and derives commands from its
/// paths and operations. Inputs may be YAML or JSON.
///
/// # Arguments
///
/// * `inputs` - A vector of `ManifestInput` instances, each containing the OpenAPI file path.
///
/// # Returns
///
/// Returns `Ok(Vec<CommandSpec>)` if the inputs are successfully processed and the commands are derived.
/// Otherwise, it returns an `Err` with a description of the encountered error.
///
/// # Processing Steps:
///
/// 1. For each input, determine if the file is YAML or JSON using `is_yaml`.
/// 2. Parse the document into a JSON value.
/// 3. Derive commands directly from the OpenAPI document.
/// 4. All generated commands across inputs are collected into a vector and sorted by group and name.
///
/// # Errors
///
/// This function returns an error if any of the following operations fail:
/// - File reading (`fs::read_to_string`).
/// - Parsing YAML or JSON content.
/// - Generating commands from the OpenAPI document.
///
/// # Example
///
/// ```rust,ignore
/// let inputs = vec![
///     ManifestInput {
///         input: PathBuf::from("path/to/schema.yaml"),
///     },
///     ManifestInput {
///         input: PathBuf::from("path/to/schema.json"),
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
/// * `derive_commands_from_openapi`: Derives command specifications from the OpenAPI document.
///
/// # Sorting
///
/// All generated commands are sorted by their group and name before returning.
fn create_commands_from_document(document: &serde_json::Value, prefix_override: Option<String>) -> Result<(String, Vec<CommandSpec>)> {
    let vendor = prefix_override.unwrap_or_else(|| derive_vendor_from_document(document));
    let mut commands = derive_commands_from_openapi(document, &vendor)?;
    commands.sort_by(|a, b| a.group.cmp(&b.group).then(a.name.cmp(&b.name)));
    Ok((vendor, commands))
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
///     CommandSpec { group: "group1", name: "command1", catalog_identifier: None },
///     CommandSpec { group: "group2", name: "command2", catalog_identifier: None }
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
///   expectations derived from the argument name (for example, `resource` → accepts
///   `resource_id`, `resource_name`, `resource_slug`).
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
    fallback_argument_accepts(&normalized)
}

fn fallback_argument_accepts(argument_key: &str) -> (Vec<String>, Option<String>) {
    let normalized = argument_key.trim();
    if normalized.is_empty() {
        return (Vec::new(), None);
    }
    if normalized == "id" {
        return (vec!["id".to_string()], Some("id".to_string()));
    }

    let suffixes = ["_id", "_name", "_slug"];
    for suffix in suffixes {
        if let Some(stripped) = normalized.strip_suffix(suffix)
            && !stripped.is_empty()
        {
            let tag = format!("{}{}", stripped, suffix);
            return (vec![tag.clone()], Some(tag));
        }
    }

    let expanded = suffixes
        .iter()
        .map(|suffix| format!("{}{}", normalized, suffix))
        .collect::<Vec<_>>();
    let preferred = expanded.first().cloned();
    (expanded, preferred)
}

fn normalize_argument_key(argument_name: &str) -> String {
    let without_indices = argument_name.replace("[]", "");
    let segment = without_indices
        .rsplit_once('.')
        .map(|(_, trailing)| trailing)
        .unwrap_or(&without_indices);
    let sanitized = segment.replace('-', "_");
    sanitized.to_snake_case().trim_matches('_').to_string()
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
    let Some(root) = schema else {
        return ProviderReturnContract::default();
    };

    let mut fields = Vec::new();
    collect_return_fields(root, "", &mut fields);
    fields.sort_by(|left, right| left.name.cmp(&right.name));

    ProviderReturnContract { fields }
}

fn collect_return_fields(schema: &SchemaProperty, prefix: &str, output: &mut Vec<ProviderFieldContract>) {
    match schema.r#type.as_str() {
        "object" => {
            let Some(properties) = &schema.properties else {
                return;
            };
            let mut entries = properties.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (property_name, property_schema) in entries {
                let path = if prefix.is_empty() {
                    property_name.clone()
                } else {
                    format!("{prefix}.{property_name}")
                };
                collect_return_fields(property_schema.as_ref(), &path, output);
            }
        }
        "array" => {
            let Some(item_schema) = schema.items.as_deref() else {
                return;
            };
            collect_return_fields(item_schema, prefix, output);
        }
        _ => {
            if prefix.is_empty() {
                return;
            }
            output.push(ProviderFieldContract {
                name: prefix.to_string(),
                r#type: Some(schema.r#type.clone()),
                tags: schema.tags.clone(),
            });
        }
    }
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

/// Parses the OpenAPI document from the given path.
///
/// # Arguments
///
/// * `path` - Path to the OpenAPI document.
///
/// # Returns
///
/// The parsed OpenAPI document as a JSON value.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the contents are invalid.
fn parse_openapi_document(input: ManifestInput) -> Result<serde_json::Value> {
    let text = input.take_contents()?;
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
        serde_json::to_value(yaml).context("could not convert yaml to json")
    } else {
        serde_json::from_str(&text).context("Unable to parse json. Invalid document format")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
    fn normalize_argument_key_handles_camel_case_and_segments() {
        assert_eq!(normalize_argument_key("resourceId"), "resource_id");
        assert_eq!(normalize_argument_key("userName"), "user_name");
        assert_eq!(normalize_argument_key("userID"), "user_id");
        assert_eq!(normalize_argument_key("metadata.ownerId"), "owner_id");
        assert_eq!(normalize_argument_key("items[].resourceId"), "resource_id");
        assert_eq!(normalize_argument_key("resource-id"), "resource_id");
    }

    #[test]
    fn argument_accepts_respects_camel_case_suffixes() {
        let (accepts, prefer) = argument_accepts_and_preference("resourceId");
        assert_eq!(accepts, vec!["resource_id"]);
        assert_eq!(prefer.as_deref(), Some("resource_id"));
    }

    #[test]
    fn argument_accepts_expands_base_names() {
        let (accepts, prefer) = argument_accepts_and_preference("resource");
        assert_eq!(accepts, vec!["resource_id", "resource_name", "resource_slug"]);
        assert_eq!(prefer.as_deref(), Some("resource_id"));
    }

    fn schema(ty: &str) -> SchemaProperty {
        SchemaProperty {
            r#type: ty.to_string(),
            description: String::new(),
            properties: None,
            required: Vec::new(),
            items: None,
            enum_values: Vec::new(),
            format: None,
            tags: Vec::new(),
        }
    }

    fn object_schema(properties: Vec<(&str, SchemaProperty)>) -> SchemaProperty {
        let mut map = HashMap::new();
        for (name, property) in properties {
            map.insert(name.to_string(), Box::new(property));
        }
        let mut root = schema("object");
        root.properties = Some(map);
        root
    }

    #[test]
    fn convert_schema_to_return_contract_flattens_nested_scalar_paths() {
        let schema = object_schema(vec![
            ("cursor", schema("string")),
            (
                "item",
                object_schema(vec![
                    ("id", schema("string")),
                    ("name", schema("string")),
                    ("owner", object_schema(vec![("id", schema("string")), ("slug", schema("string"))])),
                ]),
            ),
        ]);

        let contract = convert_schema_to_return_contract(Some(&schema));
        let field_names = contract.fields.into_iter().map(|field| field.name).collect::<Vec<_>>();
        assert_eq!(
            field_names,
            vec!["cursor", "item.id", "item.name", "item.owner.id", "item.owner.slug"]
        );
    }
}
