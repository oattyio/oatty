use crate::PluginEngine;
use crate::server::catalog::{
    import_openapi_catalog, preview_openapi_import, remove_catalog_runtime, set_catalog_enabled_state, validate_openapi_source,
};
use crate::server::http::McpHttpLogEntry;
use crate::server::log_payload::{build_log_payload, build_parsed_response_payload};
use crate::server::schemas::{
    CatalogImportOpenApiRequest, CatalogPreviewImportRequest, CatalogRemoveRequest, CatalogSetEnabledRequest,
    CatalogValidateOpenApiRequest, CommandDetailRequest, CommandSummariesRequest, OutputSchemaDetail, ProviderMetadataDetail,
    RunCommandRequestParam, SearchInputsDetail, SearchRequestParam,
};
use crate::server::workflow::{
    errors::{conflict_error, not_found_error},
    prompts::{get_prompt as get_workflow_prompt, list_prompts as list_workflow_prompts},
    resources::{
        list_resource_templates as list_workflow_resource_templates, list_resources as list_workflow_resources,
        read_resource as read_workflow_resource,
    },
    tools::{
        author_and_run, delete_workflow, export_workflow, get_workflow, import_workflow, list_workflows, preview_inputs, preview_rendered,
        purge_workflow_history, rename_workflow, repair_and_rerun, resolve_inputs, run_with_task_capability_guard, save_workflow,
        step_plan,
        types::{
            WorkflowAuthorAndRunRequest, WorkflowCancelRequest, WorkflowDeleteRequest, WorkflowExportRequest, WorkflowGetRequest,
            WorkflowImportRequest, WorkflowPreviewInputsRequest, WorkflowPreviewRenderedRequest, WorkflowPurgeHistoryRequest,
            WorkflowRenameRequest, WorkflowRepairAndRerunRequest, WorkflowResolveInputsRequest, WorkflowRunRequest, WorkflowSaveRequest,
            WorkflowStepPlanRequest, WorkflowValidateRequest,
        },
        validate_workflow,
    },
};
use anyhow::Result;
use oatty_registry::{CommandRegistry, SearchHandle};
use oatty_types::{CommandSpec, ExecOutcome, SearchResult};
use oatty_util::http::exec_remote_for_provider;
use reqwest::Method;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ErrorData, ErrorData as McpError, GetPromptRequestParams, GetPromptResult, Implementation, ListPromptsResult,
    ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
    ReadResourceResult, ServerCapabilities, ServerInfo,
};
use rmcp::task_handler;
use rmcp::task_manager::OperationProcessor;
use rmcp::{ServerHandler, service::RequestContext, tool, tool_handler, tool_router};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

/// Shared services for MCP tool handlers.
#[derive(Debug)]
pub struct McpToolServices {
    command_registry: Arc<Mutex<CommandRegistry>>,
    plugin_engine: Arc<PluginEngine>,
    search_handle: SearchHandle,
}

impl McpToolServices {
    /// Create services backed by the provided command registry and search engine.
    pub(crate) fn new(
        command_registry: Arc<Mutex<CommandRegistry>>,
        plugin_engine: Arc<PluginEngine>,
        search_handle: SearchHandle,
    ) -> Self {
        Self {
            command_registry,
            plugin_engine,
            search_handle,
        }
    }

    async fn search_commands(&self, query: String, vendor: Option<&str>) -> Result<Vec<SearchResult>> {
        let mut results = self.search_handle.search(&query).await?;
        let registry = self
            .command_registry
            .lock()
            .map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;

        if let Some(vendor_name) = vendor {
            results.retain(|result| vendor_matches(&registry, result, vendor_name));
        }
        Ok(results)
    }
}

#[derive(Clone)]
pub struct OattyMcpCore {
    tool_router: ToolRouter<Self>,
    log_sender: Option<UnboundedSender<McpHttpLogEntry>>,
    services: Arc<McpToolServices>,
    task_processor: Arc<tokio::sync::Mutex<OperationProcessor>>,
}

#[tool_router]
impl OattyMcpCore {
    /// Create a new MCP core handler with shared service dependencies.
    pub fn new(log_sender: Option<UnboundedSender<McpHttpLogEntry>>, services: Arc<McpToolServices>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            log_sender,
            services,
            task_processor: Arc::new(tokio::sync::Mutex::new(OperationProcessor::new())),
        }
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "Find executable commands by intent. Use first before any run_* call. Use during workflow authoring to discover valid step `run` values (canonical command IDs in `<group> <command>` format, for example `apps apps:list`). Input: query, optional vendor, optional limit, optional include_inputs(none|required_only|full). Canonical direct-hit queries return exactly one result when found. include_inputs=none returns minimal discovery metadata (canonical_id, execution_type, http_method). include_inputs=required_only adds required input fields, compact provider_inputs hints, and compact output_fields. include_inputs=full adds complete positional_args, flags, provider_inputs, and output_fields. For full nested output schema, call get_command with `output_schema_detail=full`. For exact single-command inspection after discovery, use get_command with canonical_id. Authoring decision rule: if required commands are still not discoverable after two focused searches, switch to catalog.validate_openapi -> catalog.preview_import -> catalog.import_openapi before drafting workflow steps. Efficiency rule: after candidate canonical IDs are found, stop fuzzy search and switch to get_command; use at most one include_inputs=full search per vendor/intent. Routing: GET -> run_safe_command, POST/PUT/PATCH -> run_command, DELETE -> run_destructive_command, MCP -> run_safe_command or run_command."
    )]
    async fn search_commands(&self, param: Parameters<SearchRequestParam>) -> Result<CallToolResult, ErrorData> {
        let direct_hit = {
            let registry_guard = self.services.command_registry.lock().map_err(|error| {
                internal_error_with_next_step(
                    error.to_string(),
                    serde_json::json!({
                        "query": param.0.query,
                        "vendor": param.0.vendor
                    }),
                    "Retry search_commands. If this persists, verify registry availability and MCP server health.",
                )
            })?;
            find_direct_search_hit(&registry_guard, param.0.query.as_str(), param.0.vendor.as_deref())
        };

        let mut results = if let Some(hit) = direct_hit {
            vec![hit]
        } else {
            self.services
                .search_commands(param.0.query.clone(), param.0.vendor.as_deref())
                .await
                .map_err(|error| {
                    internal_error_with_next_step(
                        error.to_string(),
                        serde_json::json!({
                            "query": param.0.query,
                            "vendor": param.0.vendor
                        }),
                        "Retry search_commands. If this persists, verify registry availability and MCP server health.",
                    )
                })?
        };
        if let Some(vendor_name) = param.0.vendor.as_deref()
            && results.is_empty()
            && !vendor_has_enabled_command_catalog(&self.services.command_registry, vendor_name).map_err(|error| {
                internal_error_with_next_step(
                    error.to_string(),
                    serde_json::json!({
                        "query": param.0.query,
                        "vendor": vendor_name
                    }),
                    "Retry search_commands. If this persists, verify registry availability.",
                )
            })?
        {
            return Err(invalid_params_with_next_step(
                format!("no enabled command catalog found for vendor '{vendor_name}'"),
                serde_json::json!({
                    "query": param.0.query,
                    "vendor": vendor_name,
                    "results": 0
                }),
                "Check list_command_topics for a disabled/alternate catalog. If present, enable it with catalog.set_enabled; otherwise import the vendor OpenAPI catalog via catalog.validate_openapi -> catalog.preview_import -> catalog.import_openapi, then retry search_commands.",
            ));
        }
        if let Some(limit) = param.0.limit {
            results.truncate(limit);
        }
        let inputs_detail = param.0.include_inputs.unwrap_or_default();
        let structured = if matches!(inputs_detail, SearchInputsDetail::None) {
            minimal_search_results(&results)
        } else {
            search_results_with_inputs(&self.services.command_registry, &results, inputs_detail).map_err(|error| {
                internal_error_with_next_step(
                    error.to_string(),
                    serde_json::json!({
                        "query": param.0.query,
                        "include_inputs": format!("{inputs_detail:?}").to_lowercase()
                    }),
                    "Retry search_commands with a smaller limit or include_inputs=none to isolate the issue.",
                )
            })?
        };
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "search_commands",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "Get command details by canonical_id. Defaults to compact output fields (`output_fields`) and no provider metadata. Set `output_schema_detail=full` only when full nested output schema is required. Set `include_providers=required_only|full` to include provider-backed input hints."
    )]
    async fn get_command(&self, param: Parameters<CommandDetailRequest>) -> Result<CallToolResult, ErrorData> {
        let command_spec = resolve_command_spec(&self.services.command_registry, &param.0.canonical_id)?;
        let output_schema_detail = param.0.output_schema_detail.unwrap_or_default();
        let provider_metadata_detail = param.0.include_providers.unwrap_or_default();
        let structured = build_command_summary(&command_spec, output_schema_detail, provider_metadata_detail);
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "get_command",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "List available command catalogs/topics by vendor. Use when you need a catalog title for get_command_summaries_by_catalog. Only entries with type='command' support catalog summary lookups."
    )]
    async fn list_command_topics(&self) -> Result<CallToolResult, ErrorData> {
        let catalogs = list_registry_catalogs(&self.services.command_registry, &self.services.plugin_engine)
            .await
            .map_err(|error| {
                internal_error_with_next_step(
                    error.to_string(),
                    serde_json::json!({}),
                    "Retry list_command_topics. If this persists, check registry/plugin connectivity.",
                )
            })?;
        let response = CallToolResult::structured(serde_json::json!(catalogs));
        self.emit_log(
            "list_command_catalogs",
            None,
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true),
        description = "Get command argument schemas for a full command catalog (type='command'). High-token response intended for batch inspection only. Prefer search_commands -> get_command for normal workflow authoring and single-command lookup."
    )]
    async fn get_command_summaries_by_catalog(&self, param: Parameters<CommandSummariesRequest>) -> Result<CallToolResult, ErrorData> {
        let summaries =
            list_command_summaries_by_catalog(&self.services.command_registry, param.0.catalog_title.as_str()).map_err(|error| {
                invalid_params_with_next_step(
                    error.to_string(),
                    serde_json::json!({ "catalog_title": param.0.catalog_title }),
                    "Use list_command_topics and choose an entry with type='command'; for plugin entries use search_commands with vendor instead.",
                )
            })?;
        let response = CallToolResult::structured(serde_json::json!(summaries));
        self.emit_log(
            "get_command_summaries_by_catalog",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(read_only_hint = true, open_world_hint = true),
        description = "Execute read-only commands. Use for HTTP GET or read-only MCP commands. Input: canonical_id, positional_args[], named_flags[[name,value]]. named_flags values may be JSON scalars, arrays, or objects. Rejects write/destructive HTTP methods."
    )]
    async fn run_safe_command(&self, param: Parameters<RunCommandRequestParam>) -> Result<CallToolResult, ErrorData> {
        let response = self.execute_command_with_guard(&param.0, HttpMethodGuard::SafeGet).await?;
        self.emit_log(
            "run_safe_command",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(open_world_hint = true),
        description = "Execute non-destructive write commands. Use for HTTP POST/PUT/PATCH or non-destructive MCP commands. Input: canonical_id, positional_args[], named_flags[[name,value]]. named_flags values may be JSON scalars, arrays, or objects. Rejects HTTP GET and DELETE."
    )]
    async fn run_command(&self, param: Parameters<RunCommandRequestParam>) -> Result<CallToolResult, ErrorData> {
        let response = self.execute_command_with_guard(&param.0, HttpMethodGuard::Write).await?;
        self.emit_log(
            "run_command",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        annotations(open_world_hint = true),
        description = "Execute HTTP DELETE commands only. MCP commands are not allowed. Input: canonical_id, positional_args[], named_flags[[name,value]]. named_flags values may be JSON scalars, arrays, or objects."
    )]
    async fn run_destructive_command(&self, param: Parameters<RunCommandRequestParam>) -> Result<CallToolResult, ErrorData> {
        let response = self
            .execute_command_with_guard(&param.0, HttpMethodGuard::DestructiveDelete)
            .await?;
        self.emit_log(
            "run_destructive_command",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "catalog.validate_openapi",
        annotations(read_only_hint = true),
        description = "Validate an OpenAPI source before import. Input: source, source_type?. Returns valid, document_kind, operation_count, warnings, violations."
    )]
    async fn catalog_validate_openapi(&self, param: Parameters<CatalogValidateOpenApiRequest>) -> Result<CallToolResult, ErrorData> {
        let preview = validate_openapi_source(&param.0).await?;
        let response = CallToolResult::structured(preview);
        self.emit_log(
            "catalog.validate_openapi",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "catalog.preview_import",
        annotations(read_only_hint = true),
        description = "Preview OpenAPI catalog import without writing files. Input: source, source_type?, catalog_title, vendor?, base_url?, include_command_preview?."
    )]
    async fn catalog_preview_import(&self, param: Parameters<CatalogPreviewImportRequest>) -> Result<CallToolResult, ErrorData> {
        let preview = preview_openapi_import(&param.0).await?;
        let response = CallToolResult::structured(preview);
        self.emit_log(
            "catalog.preview_import",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "catalog.import_openapi",
        annotations(open_world_hint = true),
        description = "Import OpenAPI schema into runtime catalog configuration. Input: source, source_type?, catalog_title, vendor?, base_url?, overwrite?, enabled?. This mutates local config and should be treated as a user-approved action. If APIs require auth, ask user to configure catalog headers (for example Authorization) before execution."
    )]
    async fn catalog_import_openapi(&self, param: Parameters<CatalogImportOpenApiRequest>) -> Result<CallToolResult, ErrorData> {
        let imported = import_openapi_catalog(&self.services.command_registry, &param.0).await?;
        let response = CallToolResult::structured(imported);
        self.emit_log(
            "catalog.import_openapi",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "catalog.set_enabled",
        annotations(open_world_hint = true),
        description = "Enable or disable an existing runtime catalog. Input: catalog_id, enabled."
    )]
    async fn catalog_set_enabled(&self, param: Parameters<CatalogSetEnabledRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = set_catalog_enabled_state(&self.services.command_registry, &param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "catalog.set_enabled",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "catalog.remove",
        annotations(open_world_hint = true),
        description = "Remove an existing runtime catalog entry. Input: catalog_id, remove_manifest?."
    )]
    async fn catalog_remove(&self, param: Parameters<CatalogRemoveRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = remove_catalog_runtime(&self.services.command_registry, &param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "catalog.remove",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(description = "List available workflows")]
    async fn get_workflows(&self) -> Result<CallToolResult, ErrorData> {
        let structured = list_workflows()?;
        let response = CallToolResult::structured(structured);
        self.emit_log("get_workflows", None, Some(serde_json::to_value(&response).unwrap_or(Value::Null)));
        Ok(response)
    }

    #[tool(
        name = "workflow.list",
        annotations(read_only_hint = true),
        description = "List filesystem-backed workflow manifests with path, format, and version metadata."
    )]
    async fn workflow_list(&self) -> Result<CallToolResult, ErrorData> {
        let structured = list_workflows()?;
        let response = CallToolResult::structured(structured);
        self.emit_log("workflow.list", None, Some(serde_json::to_value(&response).unwrap_or(Value::Null)));
        Ok(response)
    }

    #[tool(
        name = "workflow.get",
        annotations(read_only_hint = true),
        description = "Retrieve workflow manifest content for editing, including content version metadata. Optional flags: include_content (default true), include_parsed (default false)."
    )]
    async fn workflow_get(&self, param: Parameters<WorkflowGetRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = get_workflow(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.get",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.validate",
        annotations(read_only_hint = true),
        description = "Validate workflow YAML/JSON without saving. Input: manifest_content (inline) OR input_path (absolute file path), optional format. Includes schema checks and command/catalog preflight checks, and returns structured validation errors with violations[]. Condition syntax guidance: use `==`/`!=` (not `===`/`!==`), and use roots `inputs.*`, `steps.*`, or `env.*` (`output.*` is unsupported in conditions). Authoring guidance: for manual/free-text inputs, include `placeholder`, `hint`, and `example` metadata to improve collector UX; provider-backed inputs must use explicit scalar `select.value_field` paths."
    )]
    async fn workflow_validate(&self, param: Parameters<WorkflowValidateRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = validate_workflow(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.validate",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.save",
        annotations(open_world_hint = true),
        description = "Validate and persist workflow manifest to runtime filesystem storage. Input: workflow_id?, manifest_content, format?, overwrite?, expected_version?. Authoring sequence: search_commands -> workflow.validate -> workflow.save -> workflow.resolve_inputs -> workflow.run. Condition syntax guidance: use `==`/`!=` (not `===`/`!==`), and use roots `inputs.*`, `steps.*`, or `env.*` (`output.*` is unsupported in conditions). For manual/free-text inputs, include `placeholder`, `hint`, and `example` metadata. Provider-backed inputs must use explicit scalar `select.value_field` paths."
    )]
    async fn workflow_save(&self, param: Parameters<WorkflowSaveRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = save_workflow(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.save",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.export",
        annotations(open_world_hint = true),
        description = "Export a runtime workflow manifest into a project-relative file for source control. Input: workflow_id, output_path, format?, overwrite?, create_directories?."
    )]
    async fn workflow_export(&self, param: Parameters<WorkflowExportRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = export_workflow(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.export",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.import",
        annotations(open_world_hint = true),
        description = "Import a workflow manifest file from an absolute filesystem path into runtime storage. Input: input_path (absolute path), workflow_id?, format?, overwrite?, expected_version?."
    )]
    async fn workflow_import(&self, param: Parameters<WorkflowImportRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = import_workflow(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.import",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.rename",
        annotations(open_world_hint = true),
        description = "Rename a workflow identifier and persist it in runtime storage."
    )]
    async fn workflow_rename(&self, param: Parameters<WorkflowRenameRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = rename_workflow(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.rename",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.delete",
        annotations(open_world_hint = true),
        description = "Delete a workflow manifest from runtime storage and synchronize in-memory registry state."
    )]
    async fn workflow_delete(&self, param: Parameters<WorkflowDeleteRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = delete_workflow(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.delete",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.preview_inputs",
        annotations(read_only_hint = true),
        description = "Preview workflow input requirements and readiness. Returns workflow_id + required_missing by default; set include_inputs=true for full per-input detail rows."
    )]
    async fn workflow_preview_inputs(&self, param: Parameters<WorkflowPreviewInputsRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = preview_inputs(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.preview_inputs",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.resolve_inputs",
        annotations(open_world_hint = true),
        description = "Resolve defaults and provider bindings, then validate input values. Input: workflow_id or manifest_content, format?, partial_inputs?, include_resolved_inputs?(default false), include_provider_resolutions?(default false). Always returns ready + required_missing; include detailed payloads only when needed."
    )]
    async fn workflow_resolve_inputs(&self, param: Parameters<WorkflowResolveInputsRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = resolve_inputs(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.resolve_inputs",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.run",
        annotations(open_world_hint = true),
        description = "Execute workflow by identifier or inline manifest. Input: workflow_id|manifest_content, format?, inputs?, execution_mode(sync|auto|task), include_results?(default true), include_outputs?(default false). Mode guidance: task for long/uncertain runs or when progress/cancel is needed; sync for short immediate runs; auto when unsure."
    )]
    async fn workflow_run(&self, param: Parameters<WorkflowRunRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = run_with_task_capability_guard(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.run",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.step_plan",
        annotations(read_only_hint = true),
        description = "Return ordered workflow steps with dependency and condition evaluation metadata."
    )]
    async fn workflow_step_plan(&self, param: Parameters<WorkflowStepPlanRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = step_plan(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.step_plan",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.preview_rendered",
        annotations(read_only_hint = true),
        description = "Preview rendered workflow step payloads after template interpolation with candidate inputs."
    )]
    async fn workflow_preview_rendered(&self, param: Parameters<WorkflowPreviewRenderedRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = preview_rendered(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.preview_rendered",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.cancel",
        annotations(open_world_hint = true),
        description = "Cancel a task-backed workflow execution by task operation identifier."
    )]
    async fn workflow_cancel(&self, param: Parameters<WorkflowCancelRequest>) -> Result<CallToolResult, ErrorData> {
        let mut processor = self.task_processor.lock().await;
        if processor
            .peek_completed()
            .iter()
            .any(|result| result.descriptor.operation_id == param.0.operation_id)
        {
            return Err(conflict_error(
                "WORKFLOW_CANCEL_CONFLICT",
                format!("operation '{}' is already completed and cannot be cancelled", param.0.operation_id),
                serde_json::json!({ "operation_id": param.0.operation_id }),
                "Inspect task result and start a new run if needed.",
            ));
        }

        let cancelled = processor.cancel_task(&param.0.operation_id);
        let structured = if cancelled {
            serde_json::json!({
                "cancelled": true,
                "operation_id": param.0.operation_id,
            })
        } else if processor
            .peek_completed()
            .iter()
            .any(|result| result.descriptor.operation_id == param.0.operation_id)
        {
            return Err(conflict_error(
                "WORKFLOW_CANCEL_CONFLICT",
                format!("operation '{}' is already completed and cannot be cancelled", param.0.operation_id),
                serde_json::json!({ "operation_id": param.0.operation_id }),
                "Inspect task result and start a new run if needed.",
            ));
        } else {
            return Err(not_found_error(
                "WORKFLOW_OPERATION_NOT_FOUND",
                format!("operation '{}' was not found", param.0.operation_id),
                serde_json::json!({ "operation_id": param.0.operation_id }),
                "Use tasks/list to inspect active task-backed workflow runs.",
            ));
        };
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.cancel",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.purge_history",
        annotations(open_world_hint = true),
        description = "Purge persisted workflow run history entries by workflow identifier and/or input keys."
    )]
    async fn workflow_purge_history(&self, param: Parameters<WorkflowPurgeHistoryRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = purge_workflow_history(&param.0)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.purge_history",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.author_and_run",
        annotations(open_world_hint = true),
        description = "Orchestrate validate -> save -> resolve_inputs -> run for a draft workflow manifest. For manual/free-text inputs, include `placeholder`, `hint`, and `example` metadata."
    )]
    async fn workflow_author_and_run(&self, param: Parameters<WorkflowAuthorAndRunRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = author_and_run(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.author_and_run",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    #[tool(
        name = "workflow.repair_and_rerun",
        annotations(open_world_hint = true),
        description = "Orchestrate repair/save/rerun using manifest_content and optional repaired_manifest_content."
    )]
    async fn workflow_repair_and_rerun(&self, param: Parameters<WorkflowRepairAndRerunRequest>) -> Result<CallToolResult, ErrorData> {
        let structured = repair_and_rerun(&param.0, &self.services.command_registry)?;
        let response = CallToolResult::structured(structured);
        self.emit_log(
            "workflow.repair_and_rerun",
            Some(serde_json::to_value(&param.0).unwrap_or(Value::Null)),
            Some(serde_json::to_value(&response).unwrap_or(Value::Null)),
        );
        Ok(response)
    }

    async fn execute_command_with_guard(
        &self,
        param: &RunCommandRequestParam,
        method_guard: HttpMethodGuard,
    ) -> Result<CallToolResult, ErrorData> {
        let command_spec = resolve_command_spec(&self.services.command_registry, &param.canonical_id)?;
        if let Some(http_spec) = command_spec.http() {
            let method = Method::from_str(&http_spec.method).map_err(|error| {
                invalid_params_with_next_step(
                    format!("invalid HTTP method: {error}"),
                    serde_json::json!({
                        "canonical_id": command_spec.canonical_id(),
                        "http_method": http_spec.method
                    }),
                    "Regenerate or correct the catalog command spec so HTTP method is valid.",
                )
            })?;
            method_guard.ensure_allowed(&method)?;

            let exec_outcome = execute_http_command(&self.services.command_registry, &command_spec, param).await?;
            let structured = exec_outcome_to_value(exec_outcome)?;
            return Ok(CallToolResult::structured(structured));
        }

        if command_spec.mcp().is_some() {
            method_guard.ensure_mcp_allowed()?;
            let arguments = build_mcp_arguments(&command_spec, param)?;
            let exec_outcome = self
                .services
                .plugin_engine
                .execute_tool(&command_spec, &arguments, 0)
                .await
                .map_err(|error| {
                    internal_error_with_next_step(
                        error.to_string(),
                        serde_json::json!({ "canonical_id": command_spec.canonical_id() }),
                        "Inspect plugin health and required arguments, then retry the MCP command.",
                    )
                })?;
            let structured = exec_outcome_to_value(exec_outcome)?;
            return Ok(CallToolResult::structured(structured));
        }

        Err(ErrorData::invalid_params(
            "command execution type is unsupported by the MCP server",
            Some(serde_json::json!({
                "canonical_id": param.canonical_id,
                "next_step": "Use search_commands and execute only commands with execution_type=http or execution_type=mcp."
            })),
        ))
    }

    fn emit_log(&self, tool_name: &str, request: Option<Value>, response: Option<Value>) {
        let Some(sender) = self.log_sender.as_ref() else {
            return;
        };
        let parsed_payload = build_parsed_response_payload(request.as_ref(), response.as_ref());
        let payload = build_log_payload(request, response);
        let message = format!("MCP HTTP: {tool_name}");
        let _ = sender.send(McpHttpLogEntry::new(message, payload));

        if let Some(parsed_payload) = parsed_payload {
            let parsed_message = format!("MCP HTTP: {tool_name} (parsed response.text)");
            let _ = sender.send(McpHttpLogEntry::new(parsed_message, Some(parsed_payload)));
        }
    }
}

#[tool_handler]
#[allow(deprecated)]
#[task_handler(processor = self.task_processor)]
impl ServerHandler for OattyMcpCore {
    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(list_workflow_resources()))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(list_workflow_resource_templates()))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        std::future::ready(read_workflow_resource(&request.uri, &self.services.command_registry))
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(list_workflow_prompts()))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
        std::future::ready(get_workflow_prompt(&request.name, request.arguments.as_ref()))
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .enable_resources()
                .enable_prompts()
                .build(),
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "Oatty".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Oatty MCP".to_string()),
                ..Default::default()
            },
            instructions: Some(
                "LLM-ONLY SERVER INSTRUCTIONS.\nGENERAL FLOW:\n1) Call search_commands.\nWORKFLOW INTENT MODE:\n- If the user asks to create/author/generate a workflow, you MUST use Oatty workflow tools (search_commands -> get_command -> workflow.validate -> workflow.save or workflow.author_and_run).\n- Before implementation, verify command discoverability for EVERY required provider/platform in the request.\n- If any required provider is missing after two focused searches, STOP and run catalog.validate_openapi -> catalog.preview_import -> catalog.import_openapi.\n- Do NOT create repository docs, blueprints, scripts, or CI files unless explicitly requested by the user.\n- File-only fallback is allowed only after reporting which provider cannot be imported and receiving explicit user approval.\n\n2) Select canonical_id from results.\n3) Call get_command for exact single-command schema.\n4) Route by execution_type/http_method.\nEXAMPLES:\n- User asks: 'list vercel projects' -> search_commands(query='list vercel projects', vendor='vercel', include_inputs='none') -> get_command(canonical_id) -> run_safe_command.\n- User asks: 'create render web service' -> search_commands(query='create render web service', vendor='render', include_inputs='required_only') -> get_command(canonical_id) -> run_command.\nCATALOG ONBOARDING:\n- If search_commands returns no relevant commands or required vendors are missing, validate/import catalogs first.\n- Use catalog.validate_openapi -> catalog.preview_import before catalog.import_openapi.\n- catalog.import_openapi mutates user configuration: request user confirmation before calling it.\n- If target APIs require auth, instruct user to configure catalog headers (for example Authorization) before running HTTP commands.\nROUTING:\n- http + GET => run_safe_command\n- http + POST|PUT|PATCH => run_command\n- http + DELETE => run_destructive_command\n- mcp + read-only => run_safe_command\n- mcp + non-destructive => run_command\n- mcp + destructive => unsupported\nSEARCH OPTIMIZATION:\n- Use search_commands limit (for example 5-10) to reduce token usage.\n- Use include_inputs=none for initial discovery.\n- Use include_inputs=required_only for low-token execution planning with compact provider_inputs hints.\n- Use include_inputs=full only when complete flags/args and output_schema detail is required.\n- Canonical query form `<group> <command>` returns a single direct-hit result when found.\n- Do not call get_command_summaries_by_catalog for normal authoring; use it only for deliberate large batch inspection.\n- Use at most one include_inputs=full search per vendor/intent; then switch to get_command.\nDECISION RULES:\n- If required commands are not found after two focused searches, stop searching and switch to catalog.validate_openapi -> catalog.preview_import -> catalog.import_openapi.\n- Treat \"only unrelated catalogs found\" as a hard stop for direct workflow authoring until missing catalogs are imported.\n- After selecting candidate canonical_ids, stop fuzzy searching and switch to get_command for deterministic inspection.\n- Prefer the intended authoring order over hand-written fallback guessing.\nAUTHORING PRECHECK:\n- Confirm required command catalogs exist and are enabled.\n- Confirm required HTTP commands are discoverable.\n- Confirm provider-backed inputs are used for enumerable identifiers/list selections when provider contracts exist.\n- If any precheck fails, import missing catalogs or correct input/provider wiring before drafting more steps.\nARGUMENTS:\n- Prefer get_command for exact single-command args/flags.\n- For provider-backed workflow inputs, call get_command with include_providers=required_only (or full when binds are needed).\n- Build positional_args in declared order.\n- Build named_flags as [name,value]; values may be JSON scalars, arrays, or objects; boolean flags use presence semantics.\nWORKFLOW AUTHORING FLOW:\n- Workflow steps execute HTTP-backed commands only.\n- Do not use MCP/plugin commands as workflow steps.\n- Use search_commands with vendor filters and prefer execution_type=http when building workflows.\n- Use search_commands to discover valid step `run` command IDs (`<group> <command>`, for example `apps apps:list`).\n- If search_commands returns `provider_inputs`, prefer provider-backed workflow inputs unless the field is transformation-heavy.\n- Step arguments belong under `with` using real command parameter names.\n- Use `if`/`when` for conditions (not `condition`).\n- Input defaults must be structured objects (`default: { from: literal|env|history|workflow_output, value: ... }`).\n- Provider-first rule: use providers for enumerable identifiers/list selections (for example owner_id, project_id, service_id, domain).\n- Hybrid rule: keep manual inputs for transformation-heavy fields requiring human mapping.\n- For manual/free-text inputs, include `placeholder`, `hint`, and `example` metadata to guide users in the collector modal.\n- For provider-backed inputs, set `select.value_field` to an explicit scalar path from provider item output (for example `owner.id`).\n- Use output_fields/output_schema to map step outputs into downstream provider_args and step inputs.\n- Use summary-first payloads by default; request detailed fields only when needed.\n- Follow this sequence: search_commands -> get_command -> workflow.validate (minimal) -> expand manifest -> workflow.validate -> workflow.save -> workflow.resolve_inputs -> workflow.run.".to_string()
            ),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HttpMethodGuard {
    SafeGet,
    Write,
    DestructiveDelete,
}

impl HttpMethodGuard {
    fn ensure_allowed(&self, method: &Method) -> Result<(), ErrorData> {
        let allowed = match self {
            HttpMethodGuard::SafeGet => method == Method::GET,
            HttpMethodGuard::Write => matches!(*method, Method::POST | Method::PUT | Method::PATCH),
            HttpMethodGuard::DestructiveDelete => method == Method::DELETE,
        };
        if allowed {
            Ok(())
        } else {
            Err(ErrorData::invalid_params(
                format!("HTTP method '{method}' is not allowed for this tool"),
                Some(serde_json::json!({
                    "expected_tool": self.tool_name(),
                    "allowed_http_methods": self.allowed_http_methods(),
                    "next_step": "Use search_commands to inspect execution_type/http_method before selecting a runner tool."
                })),
            ))
        }
    }

    fn ensure_mcp_allowed(&self) -> Result<(), ErrorData> {
        match self {
            HttpMethodGuard::DestructiveDelete => Err(ErrorData::invalid_params(
                "MCP-backed commands cannot be run with the destructive command tool",
                Some(serde_json::json!({
                    "expected_tool": "run_command",
                    "allowed_execution_type": "mcp",
                    "next_step": "Use run_safe_command for read-only MCP tools or run_command for non-destructive MCP tools."
                })),
            )),
            HttpMethodGuard::SafeGet | HttpMethodGuard::Write => Ok(()),
        }
    }

    fn tool_name(&self) -> &'static str {
        match self {
            HttpMethodGuard::SafeGet => "run_safe_command",
            HttpMethodGuard::Write => "run_command",
            HttpMethodGuard::DestructiveDelete => "run_destructive_command",
        }
    }

    fn allowed_http_methods(&self) -> &'static [&'static str] {
        match self {
            HttpMethodGuard::SafeGet => &["GET"],
            HttpMethodGuard::Write => &["POST", "PUT", "PATCH"],
            HttpMethodGuard::DestructiveDelete => &["DELETE"],
        }
    }
}

fn resolve_command_spec(registry: &Arc<Mutex<CommandRegistry>>, canonical_id: &str) -> Result<CommandSpec, ErrorData> {
    let (group, name) = split_canonical_id(canonical_id)?;
    let registry_guard = registry.lock().map_err(|error| {
        internal_error_with_next_step(
            format!("registry lock failed: {error}"),
            serde_json::json!({ "canonical_id": canonical_id }),
            "Retry the command. If this persists, restart the MCP server session.",
        )
    })?;
    registry_guard.find_by_group_and_cmd_cloned(&group, &name).map_err(|error| {
        let suggested_search_query = derive_search_query_from_canonical_id(canonical_id);
        let next_step = format!(
            "Use search_commands(query='{}', include_inputs='none') to discover valid canonical IDs, then call get_command and retry.",
            suggested_search_query
        );
        invalid_params_with_next_step(
            error.to_string(),
            serde_json::json!({
                "canonical_id": canonical_id,
                "group": group,
                "command": name,
                "suggested_search_query": suggested_search_query
            }),
            next_step.as_str(),
        )
    })
}

fn split_canonical_id(canonical_id: &str) -> Result<(String, String), ErrorData> {
    parse_canonical_search_query(canonical_id).ok_or_else(|| {
        let suggested_search_query = derive_search_query_from_canonical_id(canonical_id);
        ErrorData::invalid_params(
            "canonical_id must be in 'group command' format",
            Some(serde_json::json!({
                "expected_format": "<group> <command>",
                "example": "apps apps:list",
                "suggested_search_query": suggested_search_query,
                "next_step": format!(
                    "Do not use vendor CLI syntax directly. Call search_commands(query='{}', include_inputs='none') to discover a canonical_id first.",
                    suggested_search_query
                )
            })),
        )
    })
}

fn derive_search_query_from_canonical_id(canonical_id: &str) -> String {
    let normalized = canonical_id.trim();
    if normalized.is_empty() {
        return "describe desired action in natural language".to_string();
    }
    normalized
        .chars()
        .map(|character| {
            if matches!(character, ':' | '_' | '-' | '/') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
}

async fn list_registry_catalogs(registry: &Arc<Mutex<CommandRegistry>>, plugin_engine: &Arc<PluginEngine>) -> Result<Vec<Value>> {
    let mut response: Vec<Value> = {
        let registry_guard = registry.lock().map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;
        let empty = Vec::new();
        let catalogs = registry_guard.config.catalogs.as_ref().unwrap_or(&empty);

        catalogs
            .iter()
            .filter(|catalog| catalog.is_enabled)
            .map(|catalog| {
                let vendor = catalog
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.vendor.clone())
                    .unwrap_or_default();
                serde_json::json!({
                    "title": catalog.title,
                    "vendor": vendor,
                    "description": catalog.description,
                    "type": "command",
                    "supports_command_summaries": true,
                    "workflow_step_compatible": true
                })
            })
            .collect()
    };

    if let Some(mut infos) = plugin_engine.get_active_client_infos().await {
        infos.drain(..).for_each(|info| {
            let instructions = info.instructions.unwrap_or_default();
            let Implementation { title, name, .. } = info.server_info;
            response.push(serde_json::json!({
                "title": title.unwrap_or_default(),
                "vendor": name,
                "description": instructions,
                "type": "plugin",
                "supports_command_summaries": false,
                "workflow_step_compatible": false,
                "next_step_if_workflow_goal": "Import an HTTP OpenAPI command catalog for this vendor before authoring workflow steps."
            }))
        })
    }

    Ok(response)
}

fn list_command_summaries_by_catalog(registry: &Arc<Mutex<CommandRegistry>>, catalog_title: &str) -> Result<Vec<Value>> {
    let registry_guard = registry.lock().map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;

    let catalogs = registry_guard
        .config
        .catalogs
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no catalogs configured"))?;
    let catalog_index = catalogs
        .iter()
        .position(|catalog| catalog.title == catalog_title)
        .ok_or_else(|| anyhow::anyhow!("catalog '{}' not found", catalog_title))?;
    let summaries = registry_guard
        .commands
        .iter()
        .filter(|command| command.catalog_identifier == catalog_index)
        .map(|command| build_command_summary(command, OutputSchemaDetail::Paths, ProviderMetadataDetail::None))
        .collect();
    Ok(summaries)
}

fn minimal_search_results(results: &[SearchResult]) -> Value {
    let payload = results
        .iter()
        .map(|result| Value::Object(SearchResultProjection::from_search_result(result, false).into_value_map()))
        .collect::<Vec<Value>>();
    Value::Array(payload)
}

fn search_results_with_inputs(
    registry: &Arc<Mutex<CommandRegistry>>,
    results: &[SearchResult],
    inputs_detail: SearchInputsDetail,
) -> Result<Value> {
    let registry_guard = registry.lock().map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;
    let enriched = results
        .iter()
        .map(|result| {
            let mut entry_object = SearchResultProjection::from_search_result(result, true).into_value_map();

            if let Some(command) = registry_guard
                .commands
                .iter()
                .find(|command| command.canonical_id() == result.canonical_id)
            {
                append_command_inputs_metadata(
                    &mut entry_object,
                    command,
                    inputs_detail,
                    OutputSchemaDetail::Paths,
                    ProviderMetadataDetail::None,
                );
            }
            Value::Object(entry_object)
        })
        .collect::<Vec<Value>>();

    Ok(Value::Array(enriched))
}

fn build_command_summary(
    command: &CommandSpec,
    output_schema_detail: OutputSchemaDetail,
    provider_metadata_detail: ProviderMetadataDetail,
) -> Value {
    let mut summary = SearchResultProjection::from_command_spec(command, true).into_value_map();
    append_command_inputs_metadata(
        &mut summary,
        command,
        SearchInputsDetail::Full,
        output_schema_detail,
        provider_metadata_detail,
    );
    Value::Object(summary)
}

/// Shared projection for search-oriented command summaries.
///
/// This keeps response-envelope shaping consistent between lightweight search
/// results and full command summaries.
#[derive(Debug, Clone)]
struct SearchResultProjection {
    canonical_id: String,
    execution_type: String,
    summary: Option<String>,
    http_method: Option<String>,
}

impl SearchResultProjection {
    fn from_search_result(search_result: &SearchResult, include_summary: bool) -> Self {
        Self {
            canonical_id: search_result.canonical_id.clone(),
            execution_type: search_result.execution_type.clone(),
            summary: include_summary.then(|| search_result.summary.clone()),
            http_method: search_result.http_method.clone(),
        }
    }

    fn from_command_spec(command_spec: &CommandSpec, include_summary: bool) -> Self {
        Self {
            canonical_id: command_spec.canonical_id(),
            execution_type: command_execution_type(command_spec).to_string(),
            summary: include_summary.then(|| command_spec.summary.clone()),
            http_method: command_spec.http().map(|http_spec| http_spec.method.clone()),
        }
    }

    fn into_value_map(self) -> Map<String, Value> {
        let mut entry_object = Map::new();
        entry_object.insert("canonical_id".to_string(), Value::String(self.canonical_id));
        entry_object.insert("execution_type".to_string(), Value::String(self.execution_type));
        if let Some(summary) = self.summary.as_deref() {
            insert_non_empty_string(&mut entry_object, "summary", summary);
        }
        if let Some(http_method) = self.http_method.as_deref() {
            insert_non_empty_string(&mut entry_object, "http_method", http_method);
        }
        entry_object
    }
}

fn insert_non_empty_string(entry_object: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.trim().is_empty() {
        entry_object.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn append_command_inputs_metadata(
    summary: &mut Map<String, Value>,
    command: &CommandSpec,
    inputs_detail: SearchInputsDetail,
    output_schema_detail: OutputSchemaDetail,
    provider_metadata_detail: ProviderMetadataDetail,
) {
    match inputs_detail {
        SearchInputsDetail::None => {}
        SearchInputsDetail::RequiredOnly => {
            let required_positional_args = command
                .positional_args
                .iter()
                .map(|positional_arg| Value::String(positional_arg.name.clone()))
                .collect::<Vec<Value>>();
            if !required_positional_args.is_empty() {
                summary.insert("required_positional_args".to_string(), Value::Array(required_positional_args));
            }

            let required_flags = command
                .flags
                .iter()
                .filter(|flag| flag.required)
                .map(|flag| serde_json::json!({ "name": flag.name, "type": flag.r#type }))
                .collect::<Vec<Value>>();
            if !required_flags.is_empty() {
                summary.insert("required_flags".to_string(), Value::Array(required_flags));
            }

            let provider_inputs = compact_provider_input_hints(command);
            if !provider_inputs.is_empty() {
                summary.insert("provider_inputs".to_string(), Value::Array(provider_inputs));
            }

            let output_fields = command_output_field_names(command);
            if !output_fields.is_empty() {
                summary.insert(
                    "output_fields".to_string(),
                    Value::Array(output_fields.into_iter().map(Value::String).collect::<Vec<Value>>()),
                );
            }
        }
        SearchInputsDetail::Full => {
            let (positional_args, flags) = command_input_metadata(command, provider_metadata_detail);
            if !positional_args.is_empty() {
                summary.insert("positional_args".to_string(), Value::Array(positional_args));
            }
            if !flags.is_empty() {
                summary.insert("flags".to_string(), Value::Array(flags));
            }
            let provider_inputs = compact_provider_input_hints(command);
            if !provider_inputs.is_empty() {
                summary.insert("provider_inputs".to_string(), Value::Array(provider_inputs));
            }
            append_output_schema_metadata(summary, command, output_schema_detail);
            let output_fields = command_output_field_names(command);
            if !output_fields.is_empty() {
                summary.insert(
                    "output_fields".to_string(),
                    Value::Array(output_fields.into_iter().map(Value::String).collect::<Vec<Value>>()),
                );
            }
        }
    }
}

fn append_output_schema_metadata(summary: &mut Map<String, Value>, command: &CommandSpec, output_schema_detail: OutputSchemaDetail) {
    if output_schema_detail != OutputSchemaDetail::Full {
        return;
    }

    let Some(output_schema) = command_output_schema(command) else {
        return;
    };
    let Ok(serialized_output_schema) = serde_json::to_value(output_schema) else {
        return;
    };
    let Some(pruned_output_schema) = prune_sparse_json(serialized_output_schema) else {
        return;
    };
    summary.insert("output_schema".to_string(), pruned_output_schema);
}

fn compact_provider_input_hints(command: &CommandSpec) -> Vec<Value> {
    let mut hints = Vec::new();

    hints.extend(command.positional_args.iter().filter_map(|argument| {
        provider_source(argument.provider.as_ref()).map(|source| {
            serde_json::json!({
                "name": argument.name,
                "source": source,
                "kind": "positional",
                "required": true,
            })
        })
    }));

    hints.extend(command.flags.iter().filter_map(|flag| {
        provider_source(flag.provider.as_ref()).map(|source| {
            serde_json::json!({
                "name": flag.name,
                "source": source,
                "kind": "flag",
                "required": flag.required,
            })
        })
    }));

    hints
}

fn provider_source(provider: Option<&oatty_types::ValueProvider>) -> Option<String> {
    provider.map(|oatty_types::ValueProvider::Command { command_id, .. }| command_id.clone())
}

fn prune_sparse_json(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(Value::String(text))
            }
        }
        Value::Array(items) => {
            let pruned_items = items.into_iter().filter_map(prune_sparse_json).collect::<Vec<Value>>();
            if pruned_items.is_empty() {
                None
            } else {
                Some(Value::Array(pruned_items))
            }
        }
        Value::Object(entries) => {
            let mut pruned_entries = Map::new();
            for (key, entry_value) in entries {
                if let Some(pruned_value) = prune_sparse_json(entry_value) {
                    pruned_entries.insert(key, pruned_value);
                }
            }
            if pruned_entries.is_empty() {
                None
            } else {
                Some(Value::Object(pruned_entries))
            }
        }
        scalar => Some(scalar),
    }
}

fn parse_canonical_search_query(query: &str) -> Option<(String, String)> {
    let trimmed = query.trim();
    let (group, command_name) = trimmed.split_once(' ')?;
    let normalized_group = group.trim();
    let normalized_command_name = command_name.trim();
    if normalized_group.is_empty() || normalized_command_name.is_empty() || normalized_command_name.contains(' ') {
        return None;
    }
    Some((normalized_group.to_string(), normalized_command_name.to_string()))
}

fn find_direct_search_hit(registry: &CommandRegistry, query: &str, vendor: Option<&str>) -> Option<SearchResult> {
    let (group, command_name) = parse_canonical_search_query(query)?;
    let command = registry.find_by_group_and_cmd_ref(&group, &command_name).ok()?;
    let index = registry
        .commands
        .iter()
        .position(|candidate| candidate.group == group && candidate.name == command_name)?;
    let hit = SearchResult {
        index,
        canonical_id: command.canonical_id(),
        summary: command.summary.clone(),
        execution_type: command_execution_type(command).to_string(),
        http_method: command.http().map(|http_spec| http_spec.method.clone()),
    };
    if let Some(vendor_name) = vendor
        && !vendor_matches(registry, &hit, vendor_name)
    {
        return None;
    }
    Some(hit)
}

fn command_output_schema(command: &CommandSpec) -> Option<&oatty_types::SchemaProperty> {
    command
        .http()
        .and_then(|http| http.output_schema.as_ref())
        .or_else(|| command.mcp().and_then(|mcp| mcp.output_schema.as_ref()))
}

fn command_output_field_names(command: &CommandSpec) -> Vec<String> {
    let Some(schema) = command_output_schema(command) else {
        return Vec::new();
    };
    collect_compact_output_fields(schema)
}

fn collect_compact_output_fields(schema: &oatty_types::SchemaProperty) -> Vec<String> {
    if schema.r#type == "object" {
        let Some(properties) = schema.properties.as_ref() else {
            return Vec::new();
        };
        let mut keys: Vec<String> = properties.keys().cloned().collect();
        keys.sort();
        return keys;
    }

    if schema.r#type == "array"
        && let Some(items) = schema.items.as_ref()
        && items.r#type == "object"
        && let Some(properties) = items.properties.as_ref()
    {
        let mut keys: Vec<String> = properties.keys().map(|key| format!("[].{key}")).collect();
        keys.sort();
        return keys;
    }

    Vec::new()
}

fn command_input_metadata(command: &CommandSpec, provider_metadata_detail: ProviderMetadataDetail) -> (Vec<Value>, Vec<Value>) {
    let positional_args = command
        .positional_args
        .iter()
        .map(|positional_argument| compose_positional_argument_metadata(positional_argument, provider_metadata_detail))
        .collect::<Vec<Value>>();

    let flags = command
        .flags
        .iter()
        .map(|flag| compose_flag_metadata(flag, provider_metadata_detail))
        .collect::<Vec<Value>>();

    (positional_args, flags)
}

fn compose_positional_argument_metadata(
    positional_argument: &oatty_types::PositionalArgument,
    provider_metadata_detail: ProviderMetadataDetail,
) -> Value {
    let mut value = Map::new();
    value.insert("name".to_string(), Value::String(positional_argument.name.clone()));
    value.insert("required".to_string(), Value::Bool(true));
    if let Some(help) = positional_argument.help.as_ref()
        && !help.is_empty()
    {
        value.insert("help".to_string(), Value::String(help.clone()));
    }
    append_provider_metadata(&mut value, positional_argument.provider.as_ref(), true, provider_metadata_detail);
    Value::Object(value)
}

fn compose_flag_metadata(flag: &oatty_types::CommandFlag, provider_metadata_detail: ProviderMetadataDetail) -> Value {
    let mut value = Map::new();
    value.insert("name".to_string(), Value::String(flag.name.clone()));
    value.insert("required".to_string(), Value::Bool(flag.required));
    value.insert("type".to_string(), Value::String(flag.r#type.clone()));

    if let Some(short_name) = flag.short_name.as_ref()
        && !short_name.is_empty()
    {
        value.insert("short_name".to_string(), Value::String(short_name.clone()));
    }
    if !flag.enum_values.is_empty() {
        value.insert(
            "enum_values".to_string(),
            Value::Array(flag.enum_values.iter().cloned().map(Value::String).collect::<Vec<Value>>()),
        );
    }
    if let Some(default_value) = flag.default_value.as_ref()
        && !default_value.is_empty()
    {
        value.insert("default_value".to_string(), Value::String(default_value.clone()));
    }
    if let Some(description) = flag.description.as_ref()
        && !description.is_empty()
    {
        value.insert("description".to_string(), Value::String(description.clone()));
    }
    append_provider_metadata(&mut value, flag.provider.as_ref(), flag.required, provider_metadata_detail);

    Value::Object(value)
}

fn append_provider_metadata(
    metadata: &mut Map<String, Value>,
    provider: Option<&oatty_types::ValueProvider>,
    required: bool,
    provider_metadata_detail: ProviderMetadataDetail,
) {
    match provider_metadata_detail {
        ProviderMetadataDetail::None => {}
        ProviderMetadataDetail::RequiredOnly => {
            if !required {
                return;
            }
            if let Some(provider_value) = provider_compact_value(provider, false) {
                metadata.insert("provider".to_string(), provider_value);
            }
        }
        ProviderMetadataDetail::Full => {
            if let Some(provider_value) = provider_compact_value(provider, true) {
                metadata.insert("provider".to_string(), provider_value);
            }
        }
    }
}

fn provider_compact_value(provider: Option<&oatty_types::ValueProvider>, include_binds: bool) -> Option<Value> {
    match provider {
        Some(oatty_types::ValueProvider::Command { command_id, binds }) => {
            let mut provider_value = Map::new();
            provider_value.insert("source".to_string(), Value::String(command_id.clone()));

            if include_binds && !binds.is_empty() {
                let bind_values = binds
                    .iter()
                    .map(|bind| {
                        serde_json::json!({
                            "provider_key": bind.provider_key,
                            "from": bind.from,
                        })
                    })
                    .collect::<Vec<Value>>();
                provider_value.insert("binds".to_string(), Value::Array(bind_values));
            }

            Some(Value::Object(provider_value))
        }
        None => None,
    }
}

fn command_execution_type(command_spec: &CommandSpec) -> &'static str {
    if command_spec.http().is_some() {
        return "http";
    }
    if command_spec.mcp().is_some() {
        return "mcp";
    }
    "unknown"
}

fn build_flag_map(command_spec: &CommandSpec, named_flags: &[(String, Value)]) -> Result<HashMap<String, Option<String>>, ErrorData> {
    let mut map = HashMap::new();
    for (name, value) in named_flags {
        let flag_spec = command_spec.flags.iter().find(|flag| flag.name == *name).ok_or_else(|| {
            ErrorData::invalid_params(
                format!("unknown flag '--{}'", name),
                Some(serde_json::json!({
                    "unknown_flag": name,
                    "known_flags": command_spec.flags.iter().map(|flag| flag.name.clone()).collect::<Vec<String>>(),
                    "next_step": "Call get_command_summaries_by_catalog for valid flag names."
                })),
            )
        })?;
        if flag_spec.r#type == "boolean" {
            map.insert(name.clone(), None);
        } else {
            map.insert(name.clone(), Some(value_to_validation_string(value)));
        }
    }
    Ok(map)
}

fn build_mcp_arguments(command_spec: &CommandSpec, param: &RunCommandRequestParam) -> Result<Map<String, Value>, ErrorData> {
    let positional_args = param.positional_args.clone().unwrap_or_default();
    let named_flags = param.named_flags.clone().unwrap_or_default();

    let flag_map = build_flag_map(command_spec, &named_flags)?;
    let positional_strings = positional_args.clone();
    command_spec.validate_arguments(&flag_map, &positional_strings).map_err(|error| {
        invalid_params_with_next_step(
            error.to_string(),
            serde_json::json!({
                "canonical_id": command_spec.canonical_id(),
                "provided_positional_args": positional_args,
                "provided_named_flags": named_flags
            }),
            "Call get_command_summaries_by_catalog to verify required args/flags, then retry.",
        )
    })?;

    let mut arguments = Map::new();

    for (spec, value) in command_spec.positional_args.iter().zip(positional_args.iter()) {
        arguments.insert(spec.name.clone(), Value::String(value.clone()));
    }

    for (name, value) in named_flags {
        if is_boolean_flag(command_spec, &name) {
            arguments.insert(name, Value::Bool(true));
        } else {
            arguments.insert(name, value);
        }
    }

    Ok(arguments)
}

fn is_boolean_flag(command_spec: &CommandSpec, name: &str) -> bool {
    command_spec
        .flags
        .iter()
        .find(|flag| flag.name == name)
        .is_some_and(|flag| flag.r#type == "boolean")
}

async fn execute_http_command(
    registry: &Arc<Mutex<CommandRegistry>>,
    command_spec: &CommandSpec,
    param: &RunCommandRequestParam,
) -> Result<ExecOutcome, ErrorData> {
    let (base_url, headers) = {
        let registry_guard = registry.lock().map_err(|error| {
            internal_error_with_next_step(
                format!("registry lock failed: {error}"),
                serde_json::json!({ "canonical_id": command_spec.canonical_id() }),
                "Retry command execution. If this persists, restart the MCP server session.",
            )
        })?;
        let base_url = registry_guard.resolve_base_url_for_command(command_spec).ok_or_else(|| {
            invalid_params_with_next_step(
                "base url not configured",
                serde_json::json!({ "canonical_id": command_spec.canonical_id() }),
                "Set a base URL for the catalog in Library, then retry the command.",
            )
        })?;
        let headers = registry_guard
            .resolve_headers_for_command(command_spec)
            .ok_or_else(|| {
                invalid_params_with_next_step(
                    "headers not configured",
                    serde_json::json!({ "canonical_id": command_spec.canonical_id() }),
                    "Configure required headers for the catalog in Library, then retry the command.",
                )
            })?
            .clone();
        (base_url, headers)
    };

    let input_map = build_http_input_map(command_spec, param)?;
    exec_remote_for_provider(command_spec, base_url.as_str(), &headers, input_map, 0)
        .await
        .map_err(|error| {
            internal_error_with_next_step(
                error.to_string(),
                serde_json::json!({ "canonical_id": command_spec.canonical_id() }),
                "Inspect the failing HTTP call details and retry with corrected inputs or configuration.",
            )
        })
}

fn build_http_input_map(command_spec: &CommandSpec, param: &RunCommandRequestParam) -> Result<Map<String, Value>, ErrorData> {
    let positional_args = param.positional_args.clone().unwrap_or_default();
    let named_flags = param.named_flags.clone().unwrap_or_default();

    let flag_map = build_flag_map(command_spec, &named_flags)?;
    let positional_strings = positional_args.clone();
    command_spec.validate_arguments(&flag_map, &positional_strings).map_err(|error| {
        invalid_params_with_next_step(
            error.to_string(),
            serde_json::json!({
                "canonical_id": command_spec.canonical_id(),
                "provided_positional_args": positional_args,
                "provided_named_flags": named_flags
            }),
            "Call get_command or get_command_summaries_by_catalog to verify required args/flags, then retry.",
        )
    })?;

    let mut input_map = Map::new();
    for (spec, value) in command_spec.positional_args.iter().zip(positional_args.iter()) {
        input_map.insert(spec.name.clone(), Value::String(value.clone()));
    }

    for (name, value) in named_flags {
        let flag_spec = command_spec.flags.iter().find(|flag| flag.name == name).ok_or_else(|| {
            ErrorData::invalid_params(
                format!("unknown flag '--{}'", name),
                Some(serde_json::json!({
                    "unknown_flag": name,
                    "known_flags": command_spec.flags.iter().map(|flag| flag.name.clone()).collect::<Vec<String>>(),
                    "next_step": "Call get_command for valid flag names and types."
                })),
            )
        })?;
        let normalized_value = normalize_http_flag_value(flag_spec.r#type.as_str(), value, &name)?;
        input_map.insert(name, normalized_value);
    }

    Ok(input_map)
}

fn normalize_http_flag_value(expected_type: &str, value: Value, flag_name: &str) -> Result<Value, ErrorData> {
    if expected_type == "boolean" {
        return Ok(Value::Bool(true));
    }

    match expected_type {
        "number" | "integer" => match value {
            Value::Number(_) => Ok(value),
            Value::String(text) => serde_json::from_str::<Value>(&text).ok().filter(Value::is_number).ok_or_else(|| {
                invalid_params_with_next_step(
                    format!("flag '{}' expects a numeric value", flag_name),
                    serde_json::json!({ "flag": flag_name, "provided_value": text }),
                    "Provide a JSON number (or numeric string) for this flag.",
                )
            }),
            other => Err(invalid_params_with_next_step(
                format!("flag '{}' expects a numeric value", flag_name),
                serde_json::json!({ "flag": flag_name, "provided_value": other }),
                "Provide a JSON number (or numeric string) for this flag.",
            )),
        },
        "array" => normalize_structured_value(value, flag_name, Value::is_array, "JSON array"),
        "object" => normalize_structured_value(value, flag_name, Value::is_object, "JSON object"),
        _ => Ok(value),
    }
}

fn normalize_structured_value(value: Value, flag_name: &str, matcher: fn(&Value) -> bool, expected: &str) -> Result<Value, ErrorData> {
    let next_step = format!("Provide {} value directly or as valid JSON text.", expected);

    if matcher(&value) {
        return Ok(value);
    }

    if let Value::String(text) = value {
        let parsed = serde_json::from_str::<Value>(&text).map_err(|_| {
            invalid_params_with_next_step(
                format!("flag '{}' expects {}", flag_name, expected),
                serde_json::json!({ "flag": flag_name, "provided_value": text }),
                next_step.as_str(),
            )
        })?;
        if matcher(&parsed) {
            return Ok(parsed);
        }
        return Err(invalid_params_with_next_step(
            format!("flag '{}' expects {}", flag_name, expected),
            serde_json::json!({ "flag": flag_name, "provided_value": text }),
            next_step.as_str(),
        ));
    }

    Err(invalid_params_with_next_step(
        format!("flag '{}' expects {}", flag_name, expected),
        serde_json::json!({ "flag": flag_name, "provided_value": value }),
        next_step.as_str(),
    ))
}

fn value_to_validation_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn invalid_params_with_next_step(message: impl Into<String>, context: Value, next_step: &str) -> ErrorData {
    let message = message.into();
    ErrorData::invalid_params(
        message,
        Some(serde_json::json!({
            "context": context,
            "next_step": next_step
        })),
    )
}

fn internal_error_with_next_step(message: impl Into<String>, context: Value, next_step: &str) -> ErrorData {
    let message = message.into();
    ErrorData::internal_error(
        message,
        Some(serde_json::json!({
            "context": context,
            "next_step": next_step
        })),
    )
}

fn exec_outcome_to_value(outcome: ExecOutcome) -> Result<Value, ErrorData> {
    match outcome {
        ExecOutcome::Http {
            status_code,
            log_entry,
            payload,
            request_id,
        } => Ok(serde_json::json!({
            "status_code": status_code,
            "log_entry": log_entry,
            "payload": payload,
            "request_id": request_id,
        })),
        ExecOutcome::Mcp {
            log_entry,
            payload,
            request_id,
        } => Ok(serde_json::json!({
            "status_code": 200,
            "log_entry": log_entry,
            "payload": payload,
            "request_id": request_id,
        })),
        other => Ok(serde_json::json!({
            "log_entry": format!("Unexpected execution outcome: {other:?}"),
            "status_code": 520,
            "payload": Value::Null,
            "request_id": 0,
        })),
    }
}

fn vendor_matches(registry: &CommandRegistry, result: &SearchResult, vendor_name: &str) -> bool {
    let canonical_id = result.canonical_id.as_str();
    let Some(catalogs) = registry.config.catalogs.as_ref() else {
        return false;
    };

    catalogs.iter().any(|catalog| {
        let Some(manifest) = catalog.manifest.as_ref() else {
            return false;
        };
        manifest.vendor.eq_ignore_ascii_case(vendor_name) && manifest.commands.iter().any(|command| command.canonical_id() == canonical_id)
    })
}

fn vendor_has_enabled_command_catalog(registry: &Arc<Mutex<CommandRegistry>>, vendor_name: &str) -> Result<bool> {
    let registry_guard = registry.lock().map_err(|error| anyhow::anyhow!("registry lock failed: {error}"))?;
    let Some(catalogs) = registry_guard.config.catalogs.as_ref() else {
        return Ok(false);
    };

    Ok(catalogs
        .iter()
        .filter(|catalog| catalog.is_enabled)
        .filter_map(|catalog| catalog.manifest.as_ref())
        .any(|manifest| manifest.vendor.eq_ignore_ascii_case(vendor_name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oatty_registry::RegistryConfig;
    use oatty_types::{CommandFlag, HttpCommandSpec, SchemaProperty};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn build_http_spec_for_flag_tests(flags: Vec<CommandFlag>) -> CommandSpec {
        CommandSpec::new_http(
            "vendor".to_string(),
            "resource:create".to_string(),
            "Create a resource".to_string(),
            Vec::new(),
            flags,
            HttpCommandSpec::new("POST", "/v1/resources", None, None),
            1,
        )
    }

    fn build_flag(name: &str, flag_type: &str) -> CommandFlag {
        CommandFlag {
            name: name.to_string(),
            short_name: None,
            required: false,
            r#type: flag_type.to_string(),
            enum_values: Vec::new(),
            default_value: None,
            description: None,
            provider: None,
        }
    }

    #[test]
    fn build_http_input_map_preserves_object_and_array_flag_values() {
        let command_spec = build_http_spec_for_flag_tests(vec![build_flag("project", "object"), build_flag("target", "array")]);
        let param = RunCommandRequestParam {
            canonical_id: command_spec.canonical_id(),
            positional_args: None,
            named_flags: Some(vec![
                ("project".to_string(), json!({ "name": "demo" })),
                ("target".to_string(), json!(["production", "preview"])),
            ]),
        };

        let input_map = build_http_input_map(&command_spec, &param).expect("typed flags should build an HTTP input map");

        assert_eq!(input_map.get("project"), Some(&json!({ "name": "demo" })));
        assert_eq!(input_map.get("target"), Some(&json!(["production", "preview"])));
    }

    #[test]
    fn build_http_input_map_accepts_json_string_for_structured_flags() {
        let command_spec = build_http_spec_for_flag_tests(vec![build_flag("project", "object"), build_flag("target", "array")]);
        let param = RunCommandRequestParam {
            canonical_id: command_spec.canonical_id(),
            positional_args: None,
            named_flags: Some(vec![
                ("project".to_string(), Value::String("{\"name\":\"demo\"}".to_string())),
                ("target".to_string(), Value::String("[\"production\",\"preview\"]".to_string())),
            ]),
        };

        let input_map = build_http_input_map(&command_spec, &param).expect("JSON text should parse for structured flags");

        assert_eq!(input_map.get("project"), Some(&json!({ "name": "demo" })));
        assert_eq!(input_map.get("target"), Some(&json!(["production", "preview"])));
    }

    #[test]
    fn run_command_payload_deserialization_preserves_typed_named_flags() {
        let command_spec = build_http_spec_for_flag_tests(vec![
            build_flag("project", "object"),
            build_flag("target", "array"),
            build_flag("upsert", "string"),
        ]);
        let raw_payload = json!({
            "canonical_id": command_spec.canonical_id(),
            "named_flags": [
                ["project", { "name": "starter-node" }],
                ["target", ["production", "preview", "development"]],
                ["upsert", "true"]
            ]
        });
        let param: RunCommandRequestParam = serde_json::from_value(raw_payload).expect("MCP run_command payload should deserialize");

        let input_map = build_http_input_map(&command_spec, &param).expect("deserialized flags should normalize");

        assert_eq!(input_map.get("project"), Some(&json!({ "name": "starter-node" })));
        assert_eq!(input_map.get("target"), Some(&json!(["production", "preview", "development"])));
        assert_eq!(input_map.get("upsert"), Some(&json!("true")));
    }

    #[test]
    fn parse_canonical_search_query_requires_group_and_command() {
        assert_eq!(
            parse_canonical_search_query("apps apps:list"),
            Some(("apps".to_string(), "apps:list".to_string()))
        );
        assert_eq!(
            parse_canonical_search_query("  apps   apps:list  "),
            Some(("apps".to_string(), "apps:list".to_string()))
        );
        assert_eq!(parse_canonical_search_query("apps"), None);
        assert_eq!(parse_canonical_search_query("apps apps:list extra"), None);
    }

    #[test]
    fn derive_search_query_from_canonical_id_normalizes_vendor_cli_like_tokens() {
        assert_eq!(
            derive_search_query_from_canonical_id("vercel projects:env:list"),
            "vercel projects env list".to_string()
        );
        assert_eq!(
            derive_search_query_from_canonical_id("render/services:create"),
            "render services create".to_string()
        );
    }

    #[test]
    fn find_direct_search_hit_returns_single_exact_match() {
        let command = CommandSpec::new_http(
            "apps".to_string(),
            "apps:list".to_string(),
            "List apps".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/apps", None, None),
            0,
        );
        let mut registry = CommandRegistry::default().with_commands(vec![command]);
        registry.config = RegistryConfig::default();

        let hit = find_direct_search_hit(&registry, "apps apps:list", None).expect("direct hit should resolve");
        assert_eq!(hit.canonical_id, "apps apps:list");
        assert_eq!(hit.execution_type, "http");
        assert_eq!(hit.http_method.as_deref(), Some("GET"));
    }

    #[test]
    fn search_results_with_inputs_omits_index_field() {
        let command = CommandSpec::new_http(
            "apps".to_string(),
            "apps:list".to_string(),
            "List apps".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/apps", None, None),
            0,
        );
        let mut command_registry = CommandRegistry::default().with_commands(vec![command]);
        command_registry.config = RegistryConfig::default();
        let registry = Arc::new(Mutex::new(command_registry));
        let results = vec![SearchResult {
            index: 0,
            canonical_id: "apps apps:list".to_string(),
            summary: "List apps".to_string(),
            execution_type: "http".to_string(),
            http_method: Some("GET".to_string()),
        }];

        let payload = search_results_with_inputs(&registry, &results, SearchInputsDetail::RequiredOnly).expect("payload should build");
        let first = payload
            .as_array()
            .and_then(|array| array.first())
            .and_then(Value::as_object)
            .expect("first object payload");

        assert!(!first.contains_key("index"));
        assert_eq!(first.get("canonical_id"), Some(&Value::String("apps apps:list".to_string())));
    }

    #[test]
    fn search_results_required_only_include_compact_provider_hints() {
        let command = CommandSpec::new_http(
            "apps".to_string(),
            "apps:create".to_string(),
            "Create app".to_string(),
            vec![oatty_types::PositionalArgument {
                name: "owner_id".to_string(),
                help: None,
                provider: Some(oatty_types::ValueProvider::Command {
                    command_id: "teams:list".to_string(),
                    binds: Vec::new(),
                }),
            }],
            vec![oatty_types::CommandFlag {
                name: "project_id".to_string(),
                short_name: Some("p".to_string()),
                required: true,
                r#type: "string".to_string(),
                enum_values: Vec::new(),
                default_value: None,
                description: None,
                provider: Some(oatty_types::ValueProvider::Command {
                    command_id: "projects:list".to_string(),
                    binds: Vec::new(),
                }),
            }],
            HttpCommandSpec::new("POST", "/apps", None, None),
            0,
        );
        let mut command_registry = CommandRegistry::default().with_commands(vec![command]);
        command_registry.config = RegistryConfig::default();
        let registry = Arc::new(Mutex::new(command_registry));
        let results = vec![SearchResult {
            index: 0,
            canonical_id: "apps apps:create".to_string(),
            summary: "Create app".to_string(),
            execution_type: "http".to_string(),
            http_method: Some("POST".to_string()),
        }];

        let payload = search_results_with_inputs(&registry, &results, SearchInputsDetail::RequiredOnly).expect("payload should build");
        let first = payload
            .as_array()
            .and_then(|array| array.first())
            .and_then(Value::as_object)
            .expect("first object payload");
        let provider_inputs = first
            .get("provider_inputs")
            .and_then(Value::as_array)
            .expect("provider_inputs should be included");

        assert!(provider_inputs.iter().any(|entry| {
            entry.get("name") == Some(&Value::String("owner_id".to_string()))
                && entry.get("source") == Some(&Value::String("teams:list".to_string()))
                && entry.get("kind") == Some(&Value::String("positional".to_string()))
        }));
        assert!(provider_inputs.iter().any(|entry| {
            entry.get("name") == Some(&Value::String("project_id".to_string()))
                && entry.get("source") == Some(&Value::String("projects:list".to_string()))
                && entry.get("kind") == Some(&Value::String("flag".to_string()))
        }));
    }

    #[test]
    fn command_summary_paths_mode_omits_output_schema() {
        let output_schema = SchemaProperty {
            r#type: "object".to_string(),
            description: String::new(),
            properties: None,
            required: Vec::new(),
            items: None,
            enum_values: Vec::new(),
            format: None,
            tags: Vec::new(),
        };
        let command_spec = CommandSpec::new_http(
            "apps".to_string(),
            "apps:list".to_string(),
            "List apps".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/apps", Some(output_schema), None),
            0,
        );

        let summary = build_command_summary(&command_spec, OutputSchemaDetail::Paths, ProviderMetadataDetail::None);
        let object = summary.as_object().expect("command summary should be object");
        assert!(!object.contains_key("output_schema"));
    }

    #[test]
    fn command_summary_full_mode_prunes_sparse_schema_fields() {
        let output_schema = SchemaProperty {
            r#type: "object".to_string(),
            description: String::new(),
            properties: None,
            required: Vec::new(),
            items: None,
            enum_values: Vec::new(),
            format: None,
            tags: Vec::new(),
        };
        let command_spec = CommandSpec::new_http(
            "apps".to_string(),
            "apps:list".to_string(),
            "List apps".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/apps", Some(output_schema), None),
            0,
        );

        let summary = build_command_summary(&command_spec, OutputSchemaDetail::Full, ProviderMetadataDetail::None);
        let object = summary.as_object().expect("command summary should be object");
        let schema = object
            .get("output_schema")
            .and_then(Value::as_object)
            .expect("full mode should include output_schema");
        assert_eq!(schema.get("type"), Some(&Value::String("object".to_string())));
        assert!(!schema.contains_key("description"));
        assert!(!schema.contains_key("required"));
    }

    #[test]
    fn command_summary_default_omits_provider_metadata() {
        let command_spec = CommandSpec::new_http(
            "apps".to_string(),
            "apps:list".to_string(),
            "List apps".to_string(),
            vec![oatty_types::PositionalArgument {
                name: "owner_id".to_string(),
                help: None,
                provider: Some(oatty_types::ValueProvider::Command {
                    command_id: "teams:list".to_string(),
                    binds: Vec::new(),
                }),
            }],
            vec![oatty_types::CommandFlag {
                name: "project_id".to_string(),
                short_name: Some("p".to_string()),
                required: true,
                r#type: "string".to_string(),
                enum_values: Vec::new(),
                default_value: None,
                description: None,
                provider: Some(oatty_types::ValueProvider::Command {
                    command_id: "projects:list".to_string(),
                    binds: Vec::new(),
                }),
            }],
            HttpCommandSpec::new("GET", "/apps", None, None),
            0,
        );

        let summary = build_command_summary(&command_spec, OutputSchemaDetail::Paths, ProviderMetadataDetail::None);
        let object = summary.as_object().expect("command summary should be object");
        let positional = object
            .get("positional_args")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .expect("positional arg metadata");
        assert!(!positional.contains_key("provider"));

        let flag = object
            .get("flags")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .expect("flag metadata");
        assert!(!flag.contains_key("provider"));
    }

    #[test]
    fn command_summary_required_provider_mode_only_includes_required_provider_hints() {
        let command_spec = CommandSpec::new_http(
            "apps".to_string(),
            "apps:create".to_string(),
            "Create app".to_string(),
            Vec::new(),
            vec![
                CommandFlag {
                    name: "team_id".to_string(),
                    short_name: Some("t".to_string()),
                    required: true,
                    r#type: "string".to_string(),
                    enum_values: Vec::new(),
                    default_value: None,
                    description: None,
                    provider: Some(oatty_types::ValueProvider::Command {
                        command_id: "teams:list".to_string(),
                        binds: Vec::new(),
                    }),
                },
                CommandFlag {
                    name: "region".to_string(),
                    short_name: Some("r".to_string()),
                    required: false,
                    r#type: "string".to_string(),
                    enum_values: Vec::new(),
                    default_value: None,
                    description: None,
                    provider: Some(oatty_types::ValueProvider::Command {
                        command_id: "regions:list".to_string(),
                        binds: Vec::new(),
                    }),
                },
            ],
            HttpCommandSpec::new("POST", "/apps", None, None),
            0,
        );

        let summary = build_command_summary(&command_spec, OutputSchemaDetail::Paths, ProviderMetadataDetail::RequiredOnly);
        let flags = summary
            .as_object()
            .and_then(|object| object.get("flags"))
            .and_then(Value::as_array)
            .expect("flag metadata");

        let required_flag = flags
            .iter()
            .find(|item| item.get("name") == Some(&Value::String("team_id".to_string())))
            .and_then(Value::as_object)
            .expect("required flag metadata");
        assert_eq!(required_flag.get("provider"), Some(&json!({ "source": "teams:list" })));

        let optional_flag = flags
            .iter()
            .find(|item| item.get("name") == Some(&Value::String("region".to_string())))
            .and_then(Value::as_object)
            .expect("optional flag metadata");
        assert!(!optional_flag.contains_key("provider"));
    }

    #[test]
    fn command_summary_full_provider_mode_includes_binds() {
        let command_spec = CommandSpec::new_http(
            "apps".to_string(),
            "apps:create".to_string(),
            "Create app".to_string(),
            vec![oatty_types::PositionalArgument {
                name: "project_id".to_string(),
                help: None,
                provider: Some(oatty_types::ValueProvider::Command {
                    command_id: "projects:list".to_string(),
                    binds: vec![oatty_types::Bind {
                        provider_key: "teamId".to_string(),
                        from: "team_id".to_string(),
                    }],
                }),
            }],
            Vec::new(),
            HttpCommandSpec::new("POST", "/apps", None, None),
            0,
        );

        let summary = build_command_summary(&command_spec, OutputSchemaDetail::Paths, ProviderMetadataDetail::Full);
        let positional = summary
            .as_object()
            .and_then(|object| object.get("positional_args"))
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .expect("positional metadata");

        assert_eq!(
            positional.get("provider"),
            Some(&json!({
                "source": "projects:list",
                "binds": [{ "provider_key": "teamId", "from": "team_id" }]
            }))
        );
    }
}
