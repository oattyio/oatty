//! Shared configuration mutation types and pure patch helpers for MCP server
//! management.
//!
//! This module defines the request contract, outcomes, errors, and deterministic
//! merge rules used by both TUI save flows and the `mcp_server_configure` tool.
//! It intentionally contains no runtime or I/O side effects; orchestration that
//! acquires the operation guard, persists the file, and reloads the engine lives
//! in the sibling [`super::apply_config_mutation`] module.

use crate::config::{McpConfig, McpServer, validate_config};
use indexmap::IndexMap;
use serde_json::Value;
use thiserror::Error;

/// A field that can be explicitly cleared (set to absent/empty) during a patch.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClearField {
    /// Clear the stdio command.
    Command,
    /// Clear the argument list.
    Args,
    /// Clear the working directory.
    Cwd,
    /// Clear the HTTP base URL.
    BaseUrl,
    /// Clear all environment variables (sets an empty collection).
    Env,
    /// Clear all HTTP headers (sets an empty collection).
    Headers,
    /// Clear the tag list.
    Tags,
    /// Clear the entire auth object.
    Auth,
    /// Clear the auth username.
    AuthUsername,
    /// Clear the auth password.
    AuthPassword,
    /// Clear the auth token.
    AuthToken,
    /// Clear the auth header name.
    AuthHeaderName,
    /// Clear the auth interactive flag.
    AuthInteractive,
}

/// Partial scalar/map configuration for a single server.
///
/// Omitted fields preserve persisted values. An explicit `Some(value)` replaces
/// the persisted value; `None` leaves the field unchanged.
#[derive(Debug, Clone, Default)]
pub struct McpServerConfigurationPatch {
    /// Replacement stdio command.
    pub command: Option<String>,
    /// Replacement argument list.
    pub args: Option<Vec<String>>,
    /// Environment variables to upsert.
    pub env: Option<IndexMap<String, String>>,
    /// Replacement working directory.
    pub cwd: Option<String>,
    /// Replacement HTTP base URL.
    pub base_url: Option<String>,
    /// HTTP headers to upsert.
    pub headers: Option<IndexMap<String, String>>,
    /// Authorization fields to upsert.
    pub auth: Option<McpAuthPatch>,
    /// Replacement disabled flag.
    pub disabled: Option<bool>,
    /// Replacement tag list.
    pub tags: Option<Vec<String>>,
}

/// Partial authorization configuration for patching.
///
/// Each field is independently optional. An explicit `Some(value)` replaces the
/// persisted value; `None` preserves it.
#[derive(Debug, Clone, Default)]
pub struct McpAuthPatch {
    /// Authorization scheme (e.g., "basic").
    pub scheme: Option<String>,
    /// Username.
    pub username: Option<String>,
    /// Password.
    pub password: Option<String>,
    /// Token.
    pub token: Option<String>,
    /// Custom header name.
    pub header_name: Option<String>,
    /// Allow interactive prompting on failure.
    pub interactive: Option<bool>,
}

/// Complete patch for a single server including upsert, clear, and remove
/// operations.
#[derive(Debug, Clone, Default)]
pub struct McpServerPatch {
    /// Scalar/map upserts.
    pub configuration: McpServerConfigurationPatch,
    /// Fields to clear.
    pub clear_fields: Vec<ClearField>,
    /// Environment variable keys to remove.
    pub remove_env: Vec<String>,
    /// HTTP header names to remove.
    pub remove_headers: Vec<String>,
}

/// Whether a configure operation adds a new server or edits an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigureMode {
    /// Add a new server; the name must not already exist.
    Add,
    /// Edit an existing server; the name must already exist.
    Edit,
}

/// A mutation targeting the MCP configuration file.
#[derive(Debug, Clone)]
pub enum McpConfigMutation {
    /// Configure a named server using the patch contract.
    ConfigureServer {
        /// Configured server name.
        name: String,
        /// Patch operations to apply.
        patch: McpServerPatch,
        /// Add or edit mode.
        mode: ConfigureMode,
    },
    /// Replace a server by name (used by the TUI replace/rename path).
    ReplaceServer {
        /// Previous server name.
        old_name: String,
        /// New server name.
        new_name: String,
        /// Complete replacement server configuration.
        server: McpServer,
    },
    /// Remove a server by name (used by the TUI delete path).
    RemoveServer(String),
    /// Set the local HTTP server autostart boolean (TUI-only).
    SetHttpAutostart {
        /// Whether the local HTTP server should auto-start.
        auto_start: bool,
    },
}

/// Outcome of a configuration mutation, including persistence and runtime
/// stages.
#[derive(Debug, Clone)]
pub struct McpConfigMutationOutcome {
    /// Affected server name.
    pub server_name: String,
    /// Operation kind, e.g., "configure", "replace", "remove", "set_autostart".
    pub operation: String,
    /// Names of fields that changed.
    pub changed_fields: Vec<String>,
    /// Whether the mutation was successfully persisted.
    pub persisted: bool,
    /// Whether the mutation was applied at runtime.
    pub runtime_applied: bool,
    /// Whether a connection was attempted.
    pub connection_attempted: bool,
    /// Current lifecycle status.
    pub status: ServerStatus,
    /// Safe error projection when applicable.
    pub error: Option<SafeError>,
}

/// Safe error that never contains raw credentials or debug dumps.
#[derive(Debug, Clone)]
pub struct SafeError {
    /// Error category.
    pub error_code: ErrorCategory,
    /// Stage at which the error occurred.
    pub stage: ErrorStage,
    /// Actionable guidance.
    pub next_step: String,
}

/// Category of error for structured reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// File read failure.
    ReadFailed,
    /// JSON parse failure.
    ParseFailed,
    /// Conflicting patch operations.
    PatchConflict,
    /// Candidate validation failure.
    ValidationFailed,
    /// Secret/tokenization failure.
    TokenizationFailed,
    /// File save failure.
    SaveFailed,
    /// Runtime reload failure.
    RuntimeLoadFailed,
    /// Runtime apply failure.
    RuntimeApplyFailed,
    /// Connection failure.
    ConnectionFailed,
}

/// Stage at which an error occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorStage {
    /// Reading the persisted file.
    Read,
    /// Parsing the persisted file.
    Parse,
    /// Applying the patch.
    Patch,
    /// Saving the persisted file.
    Save,
    /// Loading the runtime candidate.
    RuntimeLoad,
    /// Applying the runtime candidate.
    RuntimeApply,
    /// Connecting the target server.
    Connect,
}

/// Current lifecycle status of a server.
///
/// Alias for [`oatty_types::plugin::PluginStatus`] — the widened variants
/// (`Warning`) are collapsed only at display time by the adapter layer.
pub type ServerStatus = oatty_types::plugin::PluginStatus;

/// Errors specific to configuration mutations.
#[derive(Debug, Error)]
pub enum McpConfigMutationError {
    /// Failed to read the configuration file.
    #[error("Failed to read configuration file: {0}")]
    ReadFailed(std::io::Error),

    /// Configuration file contains invalid JSON.
    #[error("Configuration file contains invalid JSON: {0}")]
    ParseFailed(#[from] serde_json::Error),

    /// Root document is not an object with an object `mcpServers` key.
    #[error("Root document is not an object with 'mcpServers' key")]
    InvalidRootShape,

    /// Patch operations conflict with one another.
    #[error("Patch conflict detected: {0:?}")]
    PatchConflict(Vec<String>),

    /// Candidate configuration is invalid after applying the patch.
    #[error("Invalid candidate after patch: {0}")]
    ValidationFailed(String),

    /// Failed to tokenize secrets before saving.
    #[error("Failed to tokenize configuration: {0}")]
    TokenizationFailed(String),

    /// Failed to save the configuration file.
    #[error("Failed to save configuration file: {0}")]
    SaveFailed(std::io::Error),

    /// Failed to reload configuration after saving.
    #[error("Failed to reload configuration after save: {0}")]
    RuntimeLoadFailed(String),

    /// Failed to apply configuration at runtime.
    #[error("Failed to apply configuration at runtime: {0}")]
    RuntimeApplyFailed(String),
}

/// Policy controlling how the shared mutation service applies changes at
/// runtime.
#[derive(Debug, Clone)]
pub enum ReloadPolicy {
    /// Restart all servers that were running before the mutation.
    RestorePreviouslyRunning,

    /// Connect or disconnect the specified server according to `connect`.
    ConfigureTarget {
        /// Target server name.
        name: String,
        /// Whether to connect the target server.
        connect: bool,
    },
}

/// Internal mirror of [`McpConfigMutation::ConfigureServer`] fields so the
/// patch helper can accept a typed reference without re-exposing the public
/// enum variant shape.
#[derive(Debug, Clone)]
pub(crate) struct ConfigureServerMutation {
    pub(crate) name: String,
    pub(crate) patch: McpServerPatch,
    pub(crate) mode: ConfigureMode,
}

/// Returns the schema path used to report a [`ClearField`] in conflict
/// messages.
fn clear_field_path(field: &ClearField) -> &'static str {
    match field {
        ClearField::Command => "command",
        ClearField::Args => "args",
        ClearField::Cwd => "cwd",
        ClearField::BaseUrl => "baseUrl",
        ClearField::Env => "env",
        ClearField::Headers => "headers",
        ClearField::Tags => "tags",
        ClearField::Auth => "auth",
        ClearField::AuthUsername => "auth.username",
        ClearField::AuthPassword => "auth.password",
        ClearField::AuthToken => "auth.token",
        ClearField::AuthHeaderName => "auth.headerName",
        ClearField::AuthInteractive => "auth.interactive",
    }
}

/// Validates that clear, remove, and upsert operations are not in conflict.
///
/// Returns a list of conflicting field paths. An empty list means the patch is
/// safe to apply (pending typed deserialization of the complete candidate).
pub fn validate_patch_operations(patch: &McpServerPatch) -> Vec<String> {
    let mut conflicts = Vec::new();
    let configuration = &patch.configuration;

    let cleared: std::collections::HashSet<&'static str> = patch.clear_fields.iter().map(clear_field_path).collect();

    // Clear + supply the same field, including empty supplied arrays/maps.
    if cleared.contains("command") && configuration.command.is_some() {
        conflicts.push("command".to_string());
    }
    if cleared.contains("args") && configuration.args.is_some() {
        conflicts.push("args".to_string());
    }
    if cleared.contains("cwd") && configuration.cwd.is_some() {
        conflicts.push("cwd".to_string());
    }
    if cleared.contains("baseUrl") && configuration.base_url.is_some() {
        conflicts.push("baseUrl".to_string());
    }
    if cleared.contains("tags") && configuration.tags.is_some() {
        conflicts.push("tags".to_string());
    }
    if cleared.contains("env") && configuration.env.is_some() {
        conflicts.push("env".to_string());
    }
    if cleared.contains("headers") && configuration.headers.is_some() {
        conflicts.push("headers".to_string());
    }
    if cleared.contains("auth") && configuration.auth.is_some() {
        conflicts.push("auth".to_string());
    }
    if cleared.contains("auth.username") && configuration.auth.as_ref().and_then(|a| a.username.as_ref()).is_some() {
        conflicts.push("auth.username".to_string());
    }
    if cleared.contains("auth.password") && configuration.auth.as_ref().and_then(|a| a.password.as_ref()).is_some() {
        conflicts.push("auth.password".to_string());
    }
    if cleared.contains("auth.token") && configuration.auth.as_ref().and_then(|a| a.token.as_ref()).is_some() {
        conflicts.push("auth.token".to_string());
    }
    if cleared.contains("auth.headerName") && configuration.auth.as_ref().and_then(|a| a.header_name.as_ref()).is_some() {
        conflicts.push("auth.headerName".to_string());
    }
    if cleared.contains("auth.interactive") && configuration.auth.as_ref().and_then(|a| a.interactive.as_ref()).is_some() {
        conflicts.push("auth.interactive".to_string());
    }

    // Clear parent + clear child is redundant and rejected.
    if cleared.contains("auth") {
        for child in [
            "auth.username",
            "auth.password",
            "auth.token",
            "auth.headerName",
            "auth.interactive",
        ] {
            if cleared.contains(child) {
                conflicts.push(format!("auth + {child}"));
            }
        }
    }

    // Clear collection + remove an entry from that collection.
    if cleared.contains("env") {
        for key in &patch.remove_env {
            conflicts.push(format!("env + remove_env.{key}"));
        }
    }
    if cleared.contains("headers") {
        for key in &patch.remove_headers {
            conflicts.push(format!("headers + remove_headers.{key}"));
        }
    }

    // Upsert + remove the same entry.
    if let Some(env) = &configuration.env {
        for key in &patch.remove_env {
            if env.contains_key(key) {
                conflicts.push(format!("configuration.env.{key} + remove_env.{key}"));
            }
        }
    }
    if let Some(headers) = &configuration.headers {
        let upsert_normalized: std::collections::HashSet<String> = headers.keys().map(|k| k.to_ascii_lowercase()).collect();
        for key in &patch.remove_headers {
            if upsert_normalized.contains(&key.to_ascii_lowercase()) {
                conflicts.push(format!("configuration.headers.{key} + remove_headers.{key}"));
            }
        }
    }

    // Duplicate env removals.
    let mut seen_env_removals = std::collections::HashSet::new();
    for key in &patch.remove_env {
        if !seen_env_removals.insert(key) {
            conflicts.push(format!("duplicate remove_env.{key}"));
        }
    }

    // Duplicate normalized header removals.
    let mut seen_removals = std::collections::HashSet::new();
    for key in &patch.remove_headers {
        if !seen_removals.insert(key.to_ascii_lowercase()) {
            conflicts.push(format!("duplicate remove_headers.{key}"));
        }
    }

    // Duplicate normalized header upserts.
    if let Some(headers) = &configuration.headers {
        let mut seen_upserts = std::collections::HashSet::new();
        for key in headers.keys() {
            if !seen_upserts.insert(key.to_ascii_lowercase()) {
                conflicts.push(format!("duplicate header upsert: {key}"));
            }
        }
    }

    conflicts
}

/// Applies a configure patch to the selected raw server entry in `document`.
///
/// The operation is applied to a clone of the selected entry; only a validated
/// candidate is written back to `document`. On error the caller's document is
/// left unchanged. The function validates patch operations, enforces the
/// add/edit mode, applies operations in the deterministic order required by the
/// contract, runs the configure-tool stricter transport check on the selected
/// entry, and clears its diagnostic `err` field.
pub(crate) fn apply_server_patch(document: &mut Value, mutation: &ConfigureServerMutation) -> Result<(), McpConfigMutationError> {
    let conflicts = validate_patch_operations(&mutation.patch);
    if !conflicts.is_empty() {
        return Err(McpConfigMutationError::PatchConflict(conflicts));
    }

    let root = document.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;
    let mcp_servers = root
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .ok_or(McpConfigMutationError::InvalidRootShape)?;

    // Enforce add/edit mode before any mutation.
    let exists = mcp_servers.contains_key(&mutation.name);
    match mutation.mode {
        ConfigureMode::Add if exists => {
            return Err(McpConfigMutationError::ValidationFailed(format!(
                "server '{}' already exists; use edit mode",
                mutation.name
            )));
        }
        ConfigureMode::Edit if !exists => {
            return Err(McpConfigMutationError::ValidationFailed(format!(
                "server '{}' does not exist; use add mode",
                mutation.name
            )));
        }
        _ => {}
    }

    // Work on a clone so the original document is untouched until validation
    // succeeds.
    let mut server_value = mcp_servers
        .get(&mutation.name)
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let server = server_value.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;

    // 1. Clear fields.
    for field in &mutation.patch.clear_fields {
        match field {
            ClearField::Command => {
                server.remove("command");
            }
            ClearField::Args => {
                server.remove("args");
            }
            ClearField::Cwd => {
                server.remove("cwd");
            }
            ClearField::BaseUrl => {
                server.remove("baseUrl");
            }
            ClearField::Env => {
                server.insert("env".to_string(), Value::Object(serde_json::Map::new()));
            }
            ClearField::Headers => {
                server.insert("headers".to_string(), Value::Object(serde_json::Map::new()));
            }
            ClearField::Tags => {
                server.remove("tags");
            }
            ClearField::Auth => {
                server.remove("auth");
            }
            ClearField::AuthUsername => remove_auth_child(server, "username"),
            ClearField::AuthPassword => remove_auth_child(server, "password"),
            ClearField::AuthToken => remove_auth_child(server, "token"),
            ClearField::AuthHeaderName => remove_auth_child(server, "headerName"),
            ClearField::AuthInteractive => remove_auth_child(server, "interactive"),
        }
    }

    // 2. Remove map entries from env/headers collections.
    remove_env_entries(server, &mutation.patch.remove_env);
    remove_header_entries(server, &mutation.patch.remove_headers);

    // 3. Apply scalar/array replacements and map/auth upserts.
    let configuration = &mutation.patch.configuration;
    if let Some(command) = &configuration.command {
        server.insert("command".to_string(), Value::String(command.clone()));
    }
    if let Some(args) = &configuration.args {
        server.insert("args".to_string(), Value::Array(args.iter().cloned().map(Value::String).collect()));
    }
    if let Some(cwd) = &configuration.cwd {
        server.insert("cwd".to_string(), Value::String(cwd.clone()));
    }
    if let Some(base_url) = &configuration.base_url {
        server.insert("baseUrl".to_string(), Value::String(base_url.clone()));
    }
    if let Some(tags) = &configuration.tags {
        server.insert("tags".to_string(), Value::Array(tags.iter().cloned().map(Value::String).collect()));
    }
    if let Some(env) = &configuration.env {
        merge_env_collection(server, env)?;
    }
    if let Some(headers) = &configuration.headers {
        merge_header_collection(server, headers)?;
    }
    if let Some(auth) = &configuration.auth {
        merge_auth(server, auth);
    }

    // 4. Clear the selected server's diagnostic err.
    server.remove("err");

    // New servers may not have a persisted disabled value; default it to false
    // so the typed candidate deserializes without requiring the caller to supply
    // an otherwise-optional flag.
    server.entry("disabled".to_string()).or_insert_with(|| Value::Bool(false));

    // 5. Validate the selected entry under the stricter configure-tool transport
    //    rules: exactly one of nonempty command or valid http/https baseUrl, with
    //    no incompatible leftovers from the other transport.
    let patched_server: McpServer = serde_json::from_value(server_value.clone())
        .map_err(|_| McpConfigMutationError::ValidationFailed(format!("server '{}' contains invalid field values", mutation.name)))?;
    validate_selected_server_transport(&patched_server, &mutation.name)?;

    // 6. Commit the validated clone back to the document.
    mcp_servers.insert(mutation.name.clone(), server_value);
    Ok(())
}

/// Removes a child field from the server's `auth` object if present.
fn remove_auth_child(server: &mut serde_json::Map<String, Value>, child: &str) {
    if let Some(Value::Object(auth)) = server.get_mut("auth") {
        auth.remove(child);
    }
}

/// Removes entries from the server's `env` collection, respecting both map and
/// array shapes.
fn remove_env_entries(server: &mut serde_json::Map<String, Value>, keys: &[String]) {
    if keys.is_empty() {
        return;
    }
    match server.get_mut("env") {
        Some(Value::Object(map)) => {
            for key in keys {
                map.remove(key);
            }
        }
        Some(Value::Array(array)) => {
            array.retain(|item| {
                item.get("key")
                    .and_then(Value::as_str)
                    .map(|item_key| !keys.contains(&item_key.to_string()))
                    .unwrap_or(true)
            });
        }
        _ => {}
    }
}

/// Removes entries from the server's `headers` collection, respecting both map
/// and array shapes and using ASCII case-insensitive key matching.
fn remove_header_entries(server: &mut serde_json::Map<String, Value>, names: &[String]) {
    if names.is_empty() {
        return;
    }
    let normalized_names: std::collections::HashSet<String> = names.iter().map(|n| n.to_ascii_lowercase()).collect();
    match server.get_mut("headers") {
        Some(Value::Object(map)) => {
            map.retain(|key, _| !normalized_names.contains(&key.to_ascii_lowercase()));
        }
        Some(Value::Array(array)) => {
            array.retain(|item| {
                item.get("key")
                    .and_then(Value::as_str)
                    .map(|item_key| !normalized_names.contains(&item_key.to_ascii_lowercase()))
                    .unwrap_or(true)
            });
        }
        _ => {}
    }
}

/// Merges supplied environment variables into the existing `env` collection,
/// preserving its persisted shape.
fn merge_env_collection(
    server: &mut serde_json::Map<String, Value>,
    upsert: &IndexMap<String, String>,
) -> Result<(), McpConfigMutationError> {
    if upsert.is_empty() {
        return Ok(());
    }

    let is_array = matches!(server.get("env"), Some(Value::Array(_)));
    if is_array {
        let array = server
            .get_mut("env")
            .and_then(Value::as_array_mut)
            .ok_or(McpConfigMutationError::InvalidRootShape)?;
        for (key, value) in upsert {
            if let Some(existing) = array.iter_mut().find(|item| {
                item.get("key")
                    .and_then(Value::as_str)
                    .map(|item_key| item_key == key)
                    .unwrap_or(false)
            }) {
                if let Some(existing_value) = existing.get_mut("value") {
                    *existing_value = Value::String(value.clone());
                }
            } else {
                let mut entry = serde_json::Map::new();
                entry.insert("key".to_string(), Value::String(key.clone()));
                entry.insert("value".to_string(), Value::String(value.clone()));
                array.push(Value::Object(entry));
            }
        }
        return Ok(());
    }

    let map = match server.get_mut("env") {
        Some(Value::Object(map)) => map,
        None => {
            server.insert("env".to_string(), Value::Object(serde_json::Map::new()));
            server.get_mut("env").unwrap().as_object_mut().unwrap()
        }
        _ => {
            return Err(McpConfigMutationError::ValidationFailed(
                "server env field is neither a map nor an array".to_string(),
            ));
        }
    };
    for (key, value) in upsert {
        map.insert(key.clone(), Value::String(value.clone()));
    }

    Ok(())
}

/// Merges supplied headers into the existing `headers` collection, preserving
/// its persisted shape and rejecting duplicate normalized header names.
fn merge_header_collection(
    server: &mut serde_json::Map<String, Value>,
    upsert: &IndexMap<String, String>,
) -> Result<(), McpConfigMutationError> {
    if upsert.is_empty() {
        return Ok(());
    }

    // Reject duplicate normalized header names within the upsert itself.
    let mut seen = std::collections::HashSet::new();
    for key in upsert.keys() {
        if !seen.insert(key.to_ascii_lowercase()) {
            return Err(McpConfigMutationError::PatchConflict(vec![format!(
                "duplicate header upsert: {key}"
            )]));
        }
    }

    let is_array = matches!(server.get("headers"), Some(Value::Array(_)));
    if is_array {
        let array = server
            .get_mut("headers")
            .and_then(Value::as_array_mut)
            .ok_or(McpConfigMutationError::InvalidRootShape)?;
        for (key, value) in upsert {
            if let Some(existing) = array.iter_mut().find(|item| {
                item.get("key")
                    .and_then(Value::as_str)
                    .map(|item_key| item_key.eq_ignore_ascii_case(key))
                    .unwrap_or(false)
            }) {
                if let Some(existing_value) = existing.get_mut("value") {
                    *existing_value = Value::String(value.clone());
                }
            } else {
                let mut entry = serde_json::Map::new();
                entry.insert("key".to_string(), Value::String(key.clone()));
                entry.insert("value".to_string(), Value::String(value.clone()));
                array.push(Value::Object(entry));
            }
        }
        return Ok(());
    }

    let map = match server.get_mut("headers") {
        Some(Value::Object(map)) => map,
        None => {
            server.insert("headers".to_string(), Value::Object(serde_json::Map::new()));
            server.get_mut("headers").unwrap().as_object_mut().unwrap()
        }
        _ => {
            return Err(McpConfigMutationError::ValidationFailed(
                "server headers field is neither a map nor an array".to_string(),
            ));
        }
    };
    for (key, value) in upsert {
        let target_key = map
            .keys()
            .find(|existing| existing.eq_ignore_ascii_case(key))
            .cloned()
            .unwrap_or_else(|| key.clone());
        map.insert(target_key, Value::String(value.clone()));
    }

    Ok(())
}

/// Merges supplied auth fields into the server's `auth` object.
fn merge_auth(server: &mut serde_json::Map<String, Value>, patch: &McpAuthPatch) {
    // An empty auth patch is a no-op; do not create an auth object with no
    // required scheme field.
    if patch.scheme.is_none()
        && patch.username.is_none()
        && patch.password.is_none()
        && patch.token.is_none()
        && patch.header_name.is_none()
        && patch.interactive.is_none()
    {
        return;
    }

    let auth = server
        .entry("auth".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    let auth_obj = match auth {
        Value::Object(map) => map,
        _ => {
            let new_auth = serde_json::Map::new();
            *auth = Value::Object(new_auth);
            auth.as_object_mut().unwrap()
        }
    };

    if let Some(scheme) = &patch.scheme {
        auth_obj.insert("scheme".to_string(), Value::String(scheme.clone()));
    }
    if let Some(username) = &patch.username {
        auth_obj.insert("username".to_string(), Value::String(username.clone()));
    }
    if let Some(password) = &patch.password {
        auth_obj.insert("password".to_string(), Value::String(password.clone()));
    }
    if let Some(token) = &patch.token {
        auth_obj.insert("token".to_string(), Value::String(token.clone()));
    }
    if let Some(header_name) = &patch.header_name {
        auth_obj.insert("headerName".to_string(), Value::String(header_name.clone()));
    }
    if let Some(interactive) = patch.interactive {
        auth_obj.insert("interactive".to_string(), Value::Bool(interactive));
    }
}

/// Validates that the selected server has exactly one transport and no
/// incompatible leftovers from the other transport.
///
/// This stricter configure-tool rule is applied only to the entry being patched
/// so that pre-existing mixed configuration files continue to load through the
/// runtime loader.
fn validate_selected_server_transport(server: &McpServer, name: &str) -> Result<(), McpConfigMutationError> {
    let mut normalized_headers = std::collections::HashSet::with_capacity(server.headers.len());
    for header in &server.headers {
        if !normalized_headers.insert(header.key.to_ascii_lowercase()) {
            return Err(McpConfigMutationError::ValidationFailed(format!(
                "server '{name}' has duplicate header names ignoring ASCII case"
            )));
        }
    }

    let has_command = server.command.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    let has_base_url = server.base_url.is_some();

    match (has_command, has_base_url) {
        (true, true) => Err(McpConfigMutationError::ValidationFailed(format!(
            "server '{name}' has both command and baseUrl; exactly one transport is allowed"
        ))),
        (false, false) => Err(McpConfigMutationError::ValidationFailed(format!(
            "server '{name}' has no transport; provide either command or baseUrl"
        ))),
        (true, false) => {
            if !server.headers.is_empty() {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses stdio transport; headers must be cleared"
                )));
            }
            if server.auth.is_some() {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses stdio transport; auth must be cleared"
                )));
            }
            Ok(())
        }
        (false, true) => {
            if server.command.is_some() {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses http transport; command must be cleared"
                )));
            }
            if server
                .base_url
                .as_ref()
                .is_some_and(|url| url.scheme() != "http" && url.scheme() != "https")
            {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses an unsupported URL scheme; only http and https are allowed"
                )));
            }
            if server.args.is_some() {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses http transport; args must be cleared"
                )));
            }
            if server.cwd.is_some() {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses http transport; cwd must be cleared"
                )));
            }
            if !server.env.is_empty() {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{name}' uses http transport; env must be cleared"
                )));
            }
            Ok(())
        }
    }
}

/// Deserializes and validates a complete configuration candidate.
///
/// The raw `document` is first converted to [`McpConfig`] (which enforces
/// `deny_unknown_fields` on servers and auth), then run through the existing
/// whole-configuration validator. Validation failures are reported as safe
/// messages without echoing raw values.
pub(crate) fn deserialize_validated_candidate(document: Value) -> Result<McpConfig, McpConfigMutationError> {
    let config: McpConfig = serde_json::from_value(document).map_err(|_| {
        McpConfigMutationError::ValidationFailed("candidate configuration contains unsupported fields or invalid types".to_string())
    })?;
    validate_config(&config).map_err(|e| McpConfigMutationError::ValidationFailed(e.to_string()))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configure_mutation(name: &str, patch: McpServerPatch, mode: ConfigureMode) -> ConfigureServerMutation {
        ConfigureServerMutation {
            name: name.to_string(),
            patch,
            mode,
        }
    }

    fn server_value(command: &str) -> Value {
        serde_json::json!({
            "command": command,
            "args": ["a", "b"],
            "cwd": "/tmp",
            "env": {"EXISTING": "old"},
            "disabled": false,
            "tags": ["tag1"]
        })
    }

    #[test]
    fn clear_field_paths_match_contract() {
        assert_eq!(clear_field_path(&ClearField::Command), "command");
        assert_eq!(clear_field_path(&ClearField::Auth), "auth");
        assert_eq!(clear_field_path(&ClearField::AuthHeaderName), "auth.headerName");
    }

    #[test]
    fn conflict_table_env_upsert_and_remove() {
        let mut env = IndexMap::new();
        env.insert("KEY".to_string(), "value".to_string());
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                env: Some(env),
                ..Default::default()
            },
            remove_env: vec!["KEY".to_string()],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(
            conflicts
                .iter()
                .any(|c| c.contains("configuration.env.KEY") && c.contains("remove_env.KEY"))
        );
    }

    #[test]
    fn conflict_table_header_upsert_and_remove_case_insensitive() {
        let mut headers = IndexMap::new();
        headers.insert("Authorization".to_string(), "token".to_string());
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                headers: Some(headers),
                ..Default::default()
            },
            remove_headers: vec!["authorization".to_string()],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(
            conflicts
                .iter()
                .any(|c| c.contains("configuration.headers.authorization") && c.contains("remove_headers.authorization")),
            "got {conflicts:?}"
        );
    }

    #[test]
    fn conflict_table_clear_and_supply_same_field() {
        let mut env = IndexMap::new();
        env.insert("KEY".to_string(), "value".to_string());
        let mut headers = IndexMap::new();
        headers.insert("X-Header".to_string(), "value".to_string());
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                command: Some("node".to_string()),
                tags: Some(vec![]),
                env: Some(env),
                headers: Some(headers),
                ..Default::default()
            },
            clear_fields: vec![ClearField::Command, ClearField::Tags, ClearField::Env, ClearField::Headers],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.contains(&"command".to_string()));
        assert!(conflicts.contains(&"tags".to_string()));
        assert!(conflicts.contains(&"env".to_string()));
        assert!(conflicts.contains(&"headers".to_string()));
    }

    #[test]
    fn conflict_table_clear_and_supply_empty_map_is_conflict() {
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                env: Some(IndexMap::new()),
                headers: Some(IndexMap::new()),
                ..Default::default()
            },
            clear_fields: vec![ClearField::Env, ClearField::Headers],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.contains(&"env".to_string()));
        assert!(conflicts.contains(&"headers".to_string()));
    }

    #[test]
    fn conflict_table_clear_auth_and_supply_auth() {
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                auth: Some(McpAuthPatch {
                    scheme: Some("basic".to_string()),
                    username: Some("user".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            clear_fields: vec![ClearField::Auth],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.contains(&"auth".to_string()));
    }

    #[test]
    fn allowed_clear_auth_password_and_supply_username() {
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                auth: Some(McpAuthPatch {
                    username: Some("user".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            clear_fields: vec![ClearField::AuthPassword],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.is_empty(), "got {conflicts:?}");
    }

    #[test]
    fn conflict_table_clear_auth_password_and_supply_password() {
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                auth: Some(McpAuthPatch {
                    password: Some("secret".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            clear_fields: vec![ClearField::AuthPassword],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.contains(&"auth.password".to_string()));
    }

    #[test]
    fn conflict_table_clear_parent_and_child() {
        let patch = McpServerPatch {
            clear_fields: vec![ClearField::Auth, ClearField::AuthPassword],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.iter().any(|c| c.contains("auth + auth.password")));
    }

    #[test]
    fn conflict_table_clear_env_and_remove_entry() {
        let patch = McpServerPatch {
            clear_fields: vec![ClearField::Env],
            remove_env: vec!["KEY".to_string()],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.iter().any(|c| c.contains("env + remove_env.KEY")));
    }

    #[test]
    fn conflict_table_clear_headers_and_remove_entry() {
        let patch = McpServerPatch {
            clear_fields: vec![ClearField::Headers],
            remove_headers: vec!["X-Header".to_string()],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.iter().any(|c| c.contains("headers + remove_headers.X-Header")));
    }

    #[test]
    fn clear_absent_field_is_no_op() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "node",
                    "disabled": false
                }
            }
        });
        let patch = McpServerPatch {
            clear_fields: vec![ClearField::Auth, ClearField::Tags],
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();
        let server = document["mcpServers"]["server"].as_object().unwrap();
        assert_eq!(server["command"], "node");
        assert!(!server.contains_key("auth"));
        assert!(!server.contains_key("tags"));
    }

    #[test]
    fn raw_entry_repair_before_typed_validation() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "broken": {
                    "baseUrl": "not-a-url",
                    "disabled": false
                }
            }
        });

        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                base_url: Some("https://tools.example.com/mcp".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("broken", patch, ConfigureMode::Edit)).unwrap();

        let config = deserialize_validated_candidate(document).unwrap();
        let server = config.mcp_servers.get("broken").unwrap();
        assert_eq!(server.base_url.as_ref().unwrap().as_str(), "https://tools.example.com/mcp");
    }

    #[test]
    fn err_cleared_on_patched_entry() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "node",
                    "err": "stale failure",
                    "disabled": false
                }
            }
        });

        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                args: Some(vec!["--version".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();
        assert!(!document["mcpServers"]["server"].as_object().unwrap().contains_key("err"));
    }

    #[test]
    fn env_merge_preserves_unrelated_entries_and_array_metadata() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "node",
                    "env": [
                        {"key": "KEEP", "value": "old", "source": "Env", "effective": false},
                        {"key": "CHANGE", "value": "old", "source": "Secret"}
                    ]
                }
            }
        });

        let mut env = IndexMap::new();
        env.insert("CHANGE".to_string(), "new".to_string());
        env.insert("NEW".to_string(), "value".to_string());
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                env: Some(env),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();

        let env_array = document["mcpServers"]["server"]["env"].as_array().unwrap();
        assert_eq!(env_array.len(), 3);

        let keep = env_array.iter().find(|e| e["key"] == "KEEP").unwrap();
        assert_eq!(keep["value"], "old");
        assert_eq!(keep["source"], "Env");
        assert_eq!(keep["effective"], false);

        let changed = env_array.iter().find(|e| e["key"] == "CHANGE").unwrap();
        assert_eq!(changed["value"], "new");
        assert_eq!(changed["source"], "Secret");

        let new = env_array.iter().find(|e| e["key"] == "NEW").unwrap();
        assert_eq!(new["value"], "value");
        assert!(!new.as_object().unwrap().contains_key("source"));
    }

    #[test]
    fn headers_merge_preserves_unrelated_entries_and_case_insensitive_identity() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "baseUrl": "https://example.com",
                    "headers": {
                        "Authorization": "old",
                        "X-Keep": "keep"
                    }
                }
            }
        });

        let mut headers = IndexMap::new();
        headers.insert("authorization".to_string(), "new".to_string());
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                headers: Some(headers),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();

        let header_map = document["mcpServers"]["server"]["headers"].as_object().unwrap();
        assert_eq!(header_map.len(), 2);
        assert_eq!(header_map["Authorization"], "new");
        assert_eq!(header_map["X-Keep"], "keep");
    }

    #[test]
    fn duplicate_persisted_headers_are_rejected_without_mutation() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "baseUrl": "https://example.com",
                    "headers": {
                        "Authorization": "old-secret",
                        "authorization": "stale-secret"
                    },
                    "disabled": false
                }
            }
        });
        let original = document.clone();
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                tags: Some(vec!["updated".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        };

        let error = apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap_err();
        assert!(matches!(error, McpConfigMutationError::ValidationFailed(_)));
        assert_eq!(document, original);
    }

    #[test]
    fn empty_command_must_be_explicitly_cleared_for_http_switch() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "",
                    "baseUrl": "https://example.com",
                    "disabled": false
                }
            }
        });
        let original = document.clone();
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                tags: Some(vec!["updated".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        };

        let error = apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap_err();
        assert!(matches!(error, McpConfigMutationError::ValidationFailed(_)));
        assert_eq!(document, original);
    }

    #[test]
    fn empty_args_field_must_be_cleared_for_http_transport() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "baseUrl": "https://example.com",
                    "args": [],
                    "disabled": false
                }
            }
        });
        let original = document.clone();
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                tags: Some(vec!["updated".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        };

        let error = apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap_err();
        assert!(matches!(error, McpConfigMutationError::ValidationFailed(_)));
        assert_eq!(document, original);
    }

    #[test]
    fn transport_switch_requires_clearing_old_fields() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": server_value("node")
            }
        });

        // Switch to HTTP without clearing stdio fields -> ambiguous transport.
        let bad_patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                base_url: Some("https://example.com".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = apply_server_patch(&mut document, &configure_mutation("server", bad_patch, ConfigureMode::Edit)).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ValidationFailed(_)));

        // Switch to HTTP with explicit clears -> valid.
        let good_patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                base_url: Some("https://example.com".to_string()),
                ..Default::default()
            },
            clear_fields: vec![ClearField::Command, ClearField::Args, ClearField::Cwd, ClearField::Env],
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", good_patch, ConfigureMode::Edit)).unwrap();
        let server = document["mcpServers"]["server"].as_object().unwrap();
        assert_eq!(server["baseUrl"], "https://example.com");
        assert!(!server.contains_key("command"));
    }

    #[test]
    fn validation_fails_on_ambiguous_final_transport() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "node",
                    "baseUrl": "https://example.com",
                    "disabled": false
                }
            }
        });

        // Supply both transports to produce an ambiguous final candidate.
        let ambiguous_patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                command: Some("node".to_string()),
                base_url: Some("https://example.com".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = apply_server_patch(&mut document, &configure_mutation("server", ambiguous_patch, ConfigureMode::Edit)).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ValidationFailed(_)), "got {err:?}");
    }

    #[test]
    fn http_to_stdio_switch_requires_clearing_http_fields() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "baseUrl": "https://example.com",
                    "headers": {"Authorization": "token"},
                    "auth": {"scheme": "basic", "username": "u"},
                    "disabled": false
                }
            }
        });

        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                command: Some("node".to_string()),
                ..Default::default()
            },
            clear_fields: vec![ClearField::BaseUrl, ClearField::Headers, ClearField::Auth],
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();

        let server = document["mcpServers"]["server"].as_object().unwrap();
        assert_eq!(server["command"], "node");
        assert!(!server.contains_key("baseUrl"));
        assert!(server["headers"].as_object().unwrap().is_empty());
        assert!(!server.contains_key("auth"));
    }

    #[test]
    fn deserialize_validated_candidate_rejects_invalid_root() {
        let document = serde_json::json!({"mcpServers": {"bad": {"baseUrl": "not-a-url"}}});
        let err = deserialize_validated_candidate(document).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ValidationFailed(_)));
    }

    #[test]
    fn empty_auth_patch_is_no_op() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "baseUrl": "https://example.com",
                    "auth": {"scheme": "basic", "username": "u", "password": "p"},
                    "disabled": false
                }
            }
        });

        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                auth: Some(McpAuthPatch::default()),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();

        let server = document["mcpServers"]["server"].as_object().unwrap();
        assert!(server.contains_key("auth"));
        assert_eq!(server["auth"]["scheme"], "basic");
    }

    #[test]
    fn add_mode_rejects_existing_server() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {"command": "node", "disabled": false}
            }
        });
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                command: Some("other".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Add)).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ValidationFailed(_)));
        // Document must remain unchanged on failure.
        assert_eq!(document["mcpServers"]["server"]["command"], "node");
    }

    #[test]
    fn edit_mode_rejects_missing_server() {
        let mut document = serde_json::json!({"mcpServers": {}});
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                command: Some("node".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = apply_server_patch(&mut document, &configure_mutation("missing", patch, ConfigureMode::Edit)).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ValidationFailed(_)));
        assert!(!document["mcpServers"].as_object().unwrap().contains_key("missing"));
    }

    #[test]
    fn add_mode_creates_new_server() {
        let mut document = serde_json::json!({"mcpServers": {}});
        let patch = McpServerPatch {
            configuration: McpServerConfigurationPatch {
                command: Some("node".to_string()),
                args: Some(vec!["--version".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("new", patch, ConfigureMode::Add)).unwrap();

        let server = document["mcpServers"]["new"].as_object().unwrap();
        assert_eq!(server["command"], "node");
        assert_eq!(server["args"], serde_json::json!(["--version"]));
        assert_eq!(server["disabled"], false);
    }

    #[test]
    fn removing_missing_env_or_header_is_no_op() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "node",
                    "env": {"KEEP": "v"},
                    "disabled": false
                }
            }
        });
        let patch = McpServerPatch {
            remove_env: vec!["MISSING".to_string()],
            remove_headers: vec!["X-Missing".to_string()],
            ..Default::default()
        };
        apply_server_patch(&mut document, &configure_mutation("server", patch, ConfigureMode::Edit)).unwrap();

        let server = document["mcpServers"]["server"].as_object().unwrap();
        assert_eq!(server["env"]["KEEP"], "v");
    }

    #[test]
    fn duplicate_remove_env_is_conflict() {
        let patch = McpServerPatch {
            remove_env: vec!["KEY".to_string(), "KEY".to_string()],
            ..Default::default()
        };
        let conflicts = validate_patch_operations(&patch);
        assert!(conflicts.iter().any(|c| c.contains("duplicate remove_env.KEY")));
    }
}
