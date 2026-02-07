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
                "Author a workflow manifest in YAML with these requirements:\n- Goal: {goal}\n- Constraints: {constraints}\n- Include inputs with validation where appropriate\n- Include deterministic step IDs and explicit run commands\n- Use `${{{{ inputs.name }}}}` interpolation syntax for step inputs"
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
                "Update this workflow manifest according to the change request.\n\nChange request:\n{change_request}\n\nManifest:\n{manifest}\n\nRules:\n- Preserve existing identifiers unless necessary\n- Keep schema validity\n- Avoid removing required inputs unless explicitly requested"
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
}
