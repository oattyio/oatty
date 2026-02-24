//! Workflow MCP resources for specs, schema, and manifest/catalog snapshots.

use crate::server::workflow::errors::{internal_error, invalid_params_error, not_found_error};
use crate::server::workflow::services::storage::list_manifest_records;
use oatty_registry::CommandRegistry;
use rmcp::model::{
    AnnotateAble, ListResourceTemplatesResult, ListResourcesResult, RawResource, RawResourceTemplate, ReadResourceResult, ResourceContents,
};
use serde_json::Value;
use std::sync::{Arc, Mutex};

const WORKFLOW_SPEC_URI: &str = "oatty://workflow/spec";
const WORKFLOW_TUI_SPEC_URI: &str = "oatty://workflow/tui-spec";
const WORKFLOW_SCHEMA_URI: &str = "oatty://workflow/schema";
const WORKFLOW_MANIFESTS_URI: &str = "oatty://workflow/manifests";
const WORKFLOW_PROVIDER_CATALOG_URI: &str = "oatty://workflow/provider-catalog";
const WORKFLOW_COMMAND_CATALOG_URI: &str = "oatty://workflow/command-catalog";
const WORKFLOW_MANIFEST_URI_PREFIX: &str = "oatty://workflow/manifest/";
const EMBEDDED_WORKFLOW_SPEC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs/WORKFLOWS.md"));
const EMBEDDED_WORKFLOW_TUI_SPEC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs/WORKFLOW_TUI.md"));
const EMBEDDED_WORKFLOW_SCHEMA: &str = include_str!(concat!(env!("OUT_DIR"), "/workflow_definition.schema.json"));

/// Build the server resource list for workflow authoring and execution context.
pub fn list_resources() -> ListResourcesResult {
    let resources = vec![
        resource(
            WORKFLOW_SPEC_URI,
            "workflow.spec",
            Some("Workflow specification"),
            Some("specs/WORKFLOWS.md reference"),
        ),
        resource(
            WORKFLOW_TUI_SPEC_URI,
            "workflow.tui_spec",
            Some("Workflow TUI specification"),
            Some("specs/WORKFLOW_TUI.md reference"),
        ),
        resource(
            WORKFLOW_SCHEMA_URI,
            "workflow.schema",
            Some("Workflow JSON schema"),
            Some("Schema for workflow manifests"),
        ),
        resource(
            WORKFLOW_MANIFESTS_URI,
            "workflow.manifests",
            Some("Workflow manifest inventory"),
            Some("Filesystem-backed workflow manifest metadata"),
        ),
        resource(
            WORKFLOW_PROVIDER_CATALOG_URI,
            "workflow.provider_catalog",
            Some("Workflow provider catalog"),
            Some("Provider contracts and bindings available to workflows"),
        ),
        resource(
            WORKFLOW_COMMAND_CATALOG_URI,
            "workflow.command_catalog",
            Some("Workflow command catalog"),
            Some("Commands that workflow steps can reference"),
        ),
    ];

    ListResourcesResult::with_all_items(resources)
}

/// Build resource templates for parameterized resource reads.
pub fn list_resource_templates() -> ListResourceTemplatesResult {
    let templates = vec![
        RawResourceTemplate {
            uri_template: "oatty://workflow/manifest/{workflow_id}".to_string(),
            name: "workflow.manifest".to_string(),
            title: Some("Workflow manifest by id".to_string()),
            description: Some("Read a single workflow manifest from runtime storage.".to_string()),
            mime_type: Some("application/json".to_string()),
            icons: None,
        }
        .no_annotation(),
    ];
    ListResourceTemplatesResult::with_all_items(templates)
}

/// Read a workflow resource URI and return text content.
pub fn read_resource(uri: &str, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<ReadResourceResult, rmcp::model::ErrorData> {
    match uri {
        WORKFLOW_SPEC_URI => Ok(text_resource(uri, "text/markdown", EMBEDDED_WORKFLOW_SPEC.to_string())),
        WORKFLOW_TUI_SPEC_URI => Ok(text_resource(uri, "text/markdown", EMBEDDED_WORKFLOW_TUI_SPEC.to_string())),
        WORKFLOW_SCHEMA_URI => Ok(text_resource(uri, "application/json", EMBEDDED_WORKFLOW_SCHEMA.to_string())),
        WORKFLOW_MANIFESTS_URI => {
            let manifests = list_manifest_records().map_err(|error| {
                internal_error(
                    "WORKFLOW_MANIFEST_RESOURCE_FAILED",
                    error.to_string(),
                    serde_json::json!({ "uri": uri }),
                    "Verify runtime workflow directory accessibility and retry.",
                )
            })?;
            let payload = manifests
                .into_iter()
                .map(|record| {
                    serde_json::json!({
                        "workflow_id": record.definition.workflow,
                        "title": record.definition.title,
                        "description": record.definition.description,
                        "path": record.path.to_string_lossy().to_string(),
                        "format": record.format.as_str(),
                        "version": record.version,
                    })
                })
                .collect::<Vec<Value>>();
            Ok(text_resource(
                uri,
                "application/json",
                serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".to_string()),
            ))
        }
        WORKFLOW_PROVIDER_CATALOG_URI => {
            let payload = provider_catalog_resource(command_registry)?;
            Ok(text_resource(
                uri,
                "application/json",
                serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
            ))
        }
        WORKFLOW_COMMAND_CATALOG_URI => {
            let payload = command_catalog_resource(command_registry)?;
            Ok(text_resource(
                uri,
                "application/json",
                serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".to_string()),
            ))
        }
        _ if uri.starts_with(WORKFLOW_MANIFEST_URI_PREFIX) => {
            let workflow_identifier = uri.trim_start_matches(WORKFLOW_MANIFEST_URI_PREFIX);
            if workflow_identifier.trim().is_empty() {
                return Err(invalid_params_error(
                    "WORKFLOW_RESOURCE_IDENTIFIER_MISSING",
                    "workflow manifest URI is missing workflow identifier",
                    serde_json::json!({ "uri": uri }),
                    "Use oatty://workflow/manifest/{workflow_id}.",
                ));
            }
            let maybe_record = crate::server::workflow::services::storage::find_manifest_record(workflow_identifier).map_err(|error| {
                internal_error(
                    "WORKFLOW_RESOURCE_READ_FAILED",
                    error.to_string(),
                    serde_json::json!({ "uri": uri, "workflow_id": workflow_identifier }),
                    "Inspect runtime workflow directory and retry.",
                )
            })?;
            let Some(record) = maybe_record else {
                return Err(not_found_error(
                    "WORKFLOW_NOT_FOUND",
                    format!("workflow '{}' was not found", workflow_identifier),
                    serde_json::json!({ "uri": uri, "workflow_id": workflow_identifier }),
                    "Use workflow_list to inspect available workflow identifiers.",
                ));
            };

            let payload = serde_json::json!({
                "workflow_id": record.definition.workflow,
                "path": record.path.to_string_lossy().to_string(),
                "format": record.format.as_str(),
                "content": record.content,
                "parsed": record.definition,
                "version": record.version,
            });
            Ok(text_resource(
                uri,
                "application/json",
                serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
            ))
        }
        _ => Err(not_found_error(
            "WORKFLOW_RESOURCE_NOT_FOUND",
            format!("resource '{}' was not found", uri),
            serde_json::json!({ "uri": uri }),
            "Call resources/list to inspect supported resource URIs.",
        )),
    }
}

fn resource(uri: &str, name: &str, title: Option<&str>, description: Option<&str>) -> rmcp::model::Resource {
    RawResource {
        uri: uri.to_string(),
        name: name.to_string(),
        title: title.map(ToString::to_string),
        description: description.map(ToString::to_string),
        mime_type: Some("application/json".to_string()),
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation()
}

fn text_resource(uri: &str, mime_type: &str, text: String) -> ReadResourceResult {
    ReadResourceResult {
        contents: vec![ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some(mime_type.to_string()),
            text,
            meta: None,
        }],
    }
}

fn command_catalog_resource(command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, rmcp::model::ErrorData> {
    let registry = command_registry.lock().map_err(|error| {
        internal_error(
            "WORKFLOW_COMMAND_CATALOG_LOCK_FAILED",
            format!("registry lock failed: {error}"),
            serde_json::json!({}),
            "Retry resource read.",
        )
    })?;

    let payload = registry
        .commands
        .iter()
        .map(|command| {
            serde_json::json!({
                "canonical_id": command.canonical_id(),
                "summary": command.summary,
                "execution_type": execution_type(command),
                "http_method": command.http().map(|http| http.method.clone()),
                "positional_args": command.positional_args.iter().map(|arg| arg.name.clone()).collect::<Vec<String>>(),
                "flags": command.flags.iter().map(|flag| flag.name.clone()).collect::<Vec<String>>(),
            })
        })
        .collect::<Vec<Value>>();

    Ok(Value::Array(payload))
}

fn provider_catalog_resource(command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, rmcp::model::ErrorData> {
    let registry = command_registry.lock().map_err(|error| {
        internal_error(
            "WORKFLOW_PROVIDER_CATALOG_LOCK_FAILED",
            format!("registry lock failed: {error}"),
            serde_json::json!({}),
            "Retry resource read.",
        )
    })?;

    let providers = registry
        .commands
        .iter()
        .map(|command| {
            let positional = command
                .positional_args
                .iter()
                .filter_map(|arg| {
                    arg.provider
                        .as_ref()
                        .map(|provider| serde_json::json!({"name": arg.name, "provider": provider}))
                })
                .collect::<Vec<Value>>();
            let flags = command
                .flags
                .iter()
                .filter_map(|flag| {
                    flag.provider
                        .as_ref()
                        .map(|provider| serde_json::json!({"name": flag.name, "provider": provider}))
                })
                .collect::<Vec<Value>>();

            serde_json::json!({
                "canonical_id": command.canonical_id(),
                "positional_providers": positional,
                "flag_providers": flags,
                "contract": registry.provider_contracts.get(&command.canonical_id()),
            })
        })
        .collect::<Vec<Value>>();

    Ok(serde_json::json!({ "providers": providers }))
}

fn execution_type(command_spec: &oatty_types::CommandSpec) -> &'static str {
    if command_spec.http().is_some() {
        return "http";
    }
    if command_spec.mcp().is_some() {
        return "mcp";
    }
    "unknown"
}
