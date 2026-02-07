//! Structured workflow error helpers.

use chrono::Utc;
use rmcp::model::ErrorData;
use serde_json::Value;

fn build_error_data(
    error_code: &str,
    category: &str,
    message: &str,
    context: Value,
    retryable: bool,
    suggested_action: &str,
    violations: Option<Vec<Value>>,
) -> Value {
    let mut payload = serde_json::json!({
        "error_code": error_code,
        "category": category,
        "message": message,
        "context": context,
        "retryable": retryable,
        "suggested_action": suggested_action,
        "correlation_id": format!("workflow-{}", Utc::now().timestamp_millis()),
    });
    if let Some(violations) = violations {
        payload["violations"] = Value::Array(violations);
    }
    payload
}

pub fn invalid_params_error(error_code: &str, message: impl Into<String>, context: Value, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::invalid_params(
        message.clone(),
        Some(build_error_data(
            error_code,
            "validation",
            &message,
            context,
            false,
            suggested_action,
            None,
        )),
    )
}

pub fn validation_error_with_violations(
    error_code: &str,
    message: impl Into<String>,
    context: Value,
    suggested_action: &str,
    violations: Vec<Value>,
) -> ErrorData {
    let message = message.into();
    ErrorData::invalid_params(
        message.clone(),
        Some(build_error_data(
            error_code,
            "validation",
            &message,
            context,
            false,
            suggested_action,
            Some(violations),
        )),
    )
}

pub fn conflict_error(error_code: &str, message: impl Into<String>, context: Value, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::invalid_request(
        message.clone(),
        Some(build_error_data(
            error_code,
            "conflict",
            &message,
            context,
            false,
            suggested_action,
            None,
        )),
    )
}

pub fn not_found_error(error_code: &str, message: impl Into<String>, context: Value, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::resource_not_found(
        message.clone(),
        Some(build_error_data(
            error_code,
            "not_found",
            &message,
            context,
            false,
            suggested_action,
            None,
        )),
    )
}

pub fn execution_error(error_code: &str, message: impl Into<String>, context: Value, retryable: bool, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::internal_error(
        message.clone(),
        Some(build_error_data(
            error_code,
            "execution",
            &message,
            context,
            retryable,
            suggested_action,
            None,
        )),
    )
}

pub fn internal_error(error_code: &str, message: impl Into<String>, context: Value, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::internal_error(
        message.clone(),
        Some(build_error_data(
            error_code,
            "internal",
            &message,
            context,
            false,
            suggested_action,
            None,
        )),
    )
}
