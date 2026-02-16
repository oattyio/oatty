//! Workflow MCP prompts for guided authoring, extension, validation repair, and run preparation.

use crate::server::workflow::errors::{invalid_params_error, not_found_error};
use rmcp::model::{GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole};
use serde_json::{Map, Value};

/// List workflow-oriented prompts exposed by the MCP server.
pub fn list_prompts() -> rmcp::model::ListPromptsResult {
    rmcp::model::ListPromptsResult::with_all_items(vec![
        prompt_definition(
            "workflow.author",
            "Author a new workflow manifest from a goal description.",
            vec![
                required_argument("goal", "Desired workflow outcome"),
                optional_argument("constraints", "Optional constraints"),
            ],
        ),
        prompt_definition(
            "workflow.extend",
            "Extend an existing workflow manifest with additional steps or inputs.",
            vec![
                required_argument("manifest", "Existing workflow manifest content"),
                required_argument("change_request", "Requested change"),
            ],
        ),
        prompt_definition(
            "workflow.fix_validation",
            "Repair a workflow manifest using validation violations.",
            vec![
                required_argument("manifest", "Workflow manifest content"),
                required_argument("violations", "Validation violation list as JSON"),
            ],
        ),
        prompt_definition(
            "workflow.run_with_inputs",
            "Generate an input map for workflow execution.",
            vec![
                required_argument("manifest", "Workflow manifest content"),
                optional_argument("partial_inputs", "Optional known input values as JSON"),
            ],
        ),
    ])
}

/// Resolve a workflow prompt by name and arguments.
pub fn get_prompt(name: &str, arguments: Option<&Map<String, Value>>) -> Result<GetPromptResult, rmcp::model::ErrorData> {
    match name {
        "workflow.author" => workflow_author_prompt(arguments),
        "workflow.extend" => workflow_extend_prompt(arguments),
        "workflow.fix_validation" => workflow_fix_validation_prompt(arguments),
        "workflow.run_with_inputs" => workflow_run_with_inputs_prompt(arguments),
        _ => Err(not_found_error(
            "WORKFLOW_PROMPT_NOT_FOUND",
            format!("prompt '{}' was not found", name),
            serde_json::json!({ "name": name }),
            "Call prompts/list to inspect available workflow prompts.",
        )),
    }
}

fn workflow_author_prompt(arguments: Option<&Map<String, Value>>) -> Result<GetPromptResult, rmcp::model::ErrorData> {
    let goal = require_string_argument(arguments, "goal")?;
    let constraints = optional_string_argument(arguments, "constraints").unwrap_or_else(|| "None".to_string());

    Ok(GetPromptResult {
        description: Some("Draft a schema-valid workflow manifest based on an outcome goal.".to_string()),
        messages: vec![PromptMessage::new_text(
            PromptMessageRole::User,
            format!(
                "Author a workflow manifest in YAML with these requirements:\n- Goal: {goal}\n- Constraints: {constraints}\n- Include inputs with validation where appropriate\n- Prefer provider-backed inputs whenever a provider can supply valid choices\n- Use provider-backed inputs for enumerable identifiers and list selections (for example owner_id, project_id, service_id, domain, env_group)\n- Keep manual inputs for transformation-heavy fields that require human mapping/decisions (for example serviceDetails/build/start/runtime/env transformations)\n- For manual/free-text inputs, include `placeholder`, `hint`, and `example` metadata so users understand expected values\n- For provider-backed inputs, include `provider`, `select`, and `provider_args`/`depends_on` bindings to earlier inputs or steps when relevant\n- `select.value_field` must be an explicit scalar path in provider item output (for example `owner.id`, not `id` when nested)\n- Avoid manual free-text inputs when a provider can discover the value\n- Include deterministic step IDs and explicit run commands\n- Use `${{{{ inputs.name }}}}` interpolation syntax for step inputs\n- Workflow intent guardrails:\n- When the user asks to create/author a workflow, use Oatty workflow tools first and produce a valid workflow manifest before proposing non-workflow repository artifacts.\n- Do not create docs/render.yaml/CI files unless explicitly requested.\n- Use this authoring sequence:\n  1) search_commands (include_inputs=required_only, limit 5-10)\n  2) If search results contain `provider_inputs`, prefer provider-backed workflow inputs unless the field is transformation-heavy\n  3) get_command for exact schema of selected canonical IDs\n  4) workflow.validate with a minimal manifest skeleton\n  5) expand manifest and validate again before saving\n- Apply this preflight checklist before finalizing:\n  1) Required command catalogs exist and are enabled\n  2) Required HTTP commands are discoverable\n  3) Provider-backed inputs are used for enumerable identifiers/list selections when provider contracts exist\n  4) If any check fails, import missing catalogs or fix provider wiring before authoring more steps\n- Decision rules:\n  1) If required commands are not found after two focused searches, stop and import missing OpenAPI catalogs\n  2) Treat unrelated catalogs as a hard stop for direct workflow authoring\n  3) REQUIRED provider coverage: all providers named in the user goal must have discoverable commands before drafting implementation\n  4) After selecting candidate canonical IDs, stop fuzzy search and use get_command for deterministic inspection\n  5) Use at most one include_inputs=full search per vendor/intent; then switch to get_command\n  6) Do not hand-author fallback command guesses; discover/import first\n  7) Do not produce file-only templates/blueprints unless user explicitly approves fallback after import failure\n  8) Avoid get_command_summaries_by_catalog unless deliberate large batch inspection is required"
            ),
        )],
    })
}

fn workflow_extend_prompt(arguments: Option<&Map<String, Value>>) -> Result<GetPromptResult, rmcp::model::ErrorData> {
    let manifest = require_string_argument(arguments, "manifest")?;
    let change_request = require_string_argument(arguments, "change_request")?;

    Ok(GetPromptResult {
        description: Some("Extend a workflow manifest while preserving existing behavior.".to_string()),
        messages: vec![PromptMessage::new_text(
            PromptMessageRole::User,
            format!(
                "Update this workflow manifest according to the change request.\n\nChange request:\n{change_request}\n\nManifest:\n{manifest}\n\nRules:\n- Preserve existing identifiers unless necessary\n- Keep schema validity\n- Preserve existing provider-backed inputs and bindings unless the change requires adjustment\n- Add provider-backed inputs when the change introduces values that can be discovered dynamically\n- For manual/free-text inputs, preserve or add `placeholder`, `hint`, and `example` metadata when missing\n- Avoid removing required inputs unless explicitly requested"
            ),
        )],
    })
}

fn workflow_fix_validation_prompt(arguments: Option<&Map<String, Value>>) -> Result<GetPromptResult, rmcp::model::ErrorData> {
    let manifest = require_string_argument(arguments, "manifest")?;
    let violations = require_string_argument(arguments, "violations")?;

    Ok(GetPromptResult {
        description: Some("Repair workflow manifest validation violations with minimal diff.".to_string()),
        messages: vec![PromptMessage::new_text(
            PromptMessageRole::User,
            format!(
                "Repair this workflow manifest so validation succeeds.\n\nViolations JSON:\n{violations}\n\nManifest:\n{manifest}\n\nRules:\n- Apply the smallest set of changes that resolve all violations\n- Preserve workflow intent and command behavior"
            ),
        )],
    })
}

fn workflow_run_with_inputs_prompt(arguments: Option<&Map<String, Value>>) -> Result<GetPromptResult, rmcp::model::ErrorData> {
    let manifest = require_string_argument(arguments, "manifest")?;
    let partial_inputs = optional_string_argument(arguments, "partial_inputs").unwrap_or_else(|| "{}".to_string());

    Ok(GetPromptResult {
        description: Some("Prepare workflow input values for workflow.resolve_inputs or workflow.run.".to_string()),
        messages: vec![PromptMessage::new_text(
            PromptMessageRole::User,
            format!(
                "Given this workflow manifest and partial inputs, produce a JSON object of input values to execute the workflow.\n\nManifest:\n{manifest}\n\nPartial inputs:\n{partial_inputs}\n\nRules:\n- Only include keys defined under workflow inputs\n- Preserve known values\n- Prefer explicit concrete values over placeholders"
            ),
        )],
    })
}

fn prompt_definition(name: &str, description: &str, arguments: Vec<PromptArgument>) -> Prompt {
    Prompt {
        name: name.to_string(),
        title: None,
        description: Some(description.to_string()),
        arguments: Some(arguments),
        icons: None,
        meta: None,
    }
}

fn required_argument(name: &str, description: &str) -> PromptArgument {
    PromptArgument {
        name: name.to_string(),
        title: None,
        description: Some(description.to_string()),
        required: Some(true),
    }
}

fn optional_argument(name: &str, description: &str) -> PromptArgument {
    PromptArgument {
        name: name.to_string(),
        title: None,
        description: Some(description.to_string()),
        required: Some(false),
    }
}

fn require_string_argument(arguments: Option<&Map<String, Value>>, key: &str) -> Result<String, rmcp::model::ErrorData> {
    match arguments.and_then(|args| args.get(key)).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => Ok(value.to_string()),
        _ => Err(invalid_params_error(
            "WORKFLOW_PROMPT_ARGUMENT_MISSING",
            format!("prompt argument '{}' is required", key),
            serde_json::json!({ "argument": key }),
            "Provide all required prompt arguments and retry.",
        )),
    }
}

fn optional_string_argument(arguments: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    arguments
        .and_then(|args| args.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_prompts_exposes_expected_workflow_prompt_names() {
        let prompts = list_prompts();
        let names = prompts.prompts.into_iter().map(|prompt| prompt.name).collect::<Vec<String>>();
        assert!(names.contains(&"workflow.author".to_string()));
        assert!(names.contains(&"workflow.extend".to_string()));
        assert!(names.contains(&"workflow.fix_validation".to_string()));
        assert!(names.contains(&"workflow.run_with_inputs".to_string()));
    }

    #[test]
    fn get_prompt_requires_mandatory_arguments() {
        let error = get_prompt("workflow.author", None).expect_err("author prompt should reject missing goal");
        assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn workflow_author_prompt_mentions_provider_guidance() {
        let args = serde_json::json!({
            "goal": "Create an app and attach an addon",
            "constraints": "Use existing app names"
        });
        let object = args.as_object().expect("object args");
        let prompt = get_prompt("workflow.author", Some(object)).expect("author prompt");
        let rendered = format!("{:?}", prompt.messages);

        assert!(rendered.contains("provider-backed inputs"));
        assert!(rendered.contains("provider_args"));
        assert!(rendered.contains("depends_on"));
        assert!(rendered.contains("enumerable identifiers"));
        assert!(rendered.contains("transformation-heavy fields"));
        assert!(rendered.contains("preflight checklist"));
        assert!(rendered.contains("authoring sequence"));
        assert!(rendered.contains("two focused searches"));
        assert!(rendered.contains("stop fuzzy search and use get_command"));
        assert!(rendered.contains("Avoid get_command_summaries_by_catalog"));
        assert!(rendered.contains("Provider-backed inputs are used"));
        assert!(rendered.contains("at most one include_inputs=full search"));
    }
}
