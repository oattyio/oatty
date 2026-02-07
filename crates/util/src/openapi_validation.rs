//! OpenAPI document preflight validation helpers.
//!
//! This module provides lightweight, reusable validation for OpenAPI sources before
//! expensive command generation is attempted.

use serde_json::Value;

/// Represents a structured OpenAPI preflight validation violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiValidationViolation {
    /// JSON path where the violation occurred.
    pub path: String,
    /// Stable rule identifier for machine-readable handling.
    pub rule: String,
    /// Human-readable validation error message.
    pub message: String,
}

impl OpenApiValidationViolation {
    /// Creates a new validation violation.
    pub fn new(path: impl Into<String>, rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Converts this violation into a JSON object for transport layers.
    pub fn to_json_value(&self) -> Value {
        serde_json::json!({
            "path": self.path,
            "rule": self.rule,
            "message": self.message,
        })
    }
}

/// Validates an OpenAPI document for basic import readiness.
///
/// This preflight ensures:
/// - The document declares `openapi` as a `3.x` version string.
/// - `paths` exists and is an object.
/// - At least one HTTP operation exists under `paths`.
pub fn collect_openapi_preflight_violations(document: &Value) -> Vec<OpenApiValidationViolation> {
    let mut violations = Vec::new();

    match document.get("openapi") {
        Some(Value::String(version)) if version.starts_with("3.") => {}
        Some(Value::String(version)) => violations.push(OpenApiValidationViolation::new(
            "$.openapi",
            "openapi_version",
            format!("unsupported OpenAPI version '{}'; expected a 3.x document", version),
        )),
        Some(_) => violations.push(OpenApiValidationViolation::new(
            "$.openapi",
            "openapi_version",
            "field `openapi` must be a string and start with `3.`",
        )),
        None => {
            if let Some(swagger_version) = document.get("swagger").and_then(Value::as_str) {
                violations.push(OpenApiValidationViolation::new(
                    "$.swagger",
                    "openapi_version",
                    format!(
                        "Swagger/OpenAPI 2.x document detected ('{}'); OpenAPI 3.x is required",
                        swagger_version
                    ),
                ));
            } else {
                violations.push(OpenApiValidationViolation::new(
                    "$.openapi",
                    "openapi_version",
                    "missing required `openapi` field; expected an OpenAPI 3.x document",
                ));
            }
        }
    }

    let paths = match document.get("paths") {
        Some(Value::Object(paths)) => Some(paths),
        Some(_) => {
            violations.push(OpenApiValidationViolation::new(
                "$.paths",
                "paths_type",
                "field `paths` must be an object",
            ));
            None
        }
        None => {
            violations.push(OpenApiValidationViolation::new(
                "$.paths",
                "paths_required",
                "missing required `paths` object",
            ));
            None
        }
    };

    if let Some(paths) = paths {
        let operation_count = paths
            .values()
            .filter_map(Value::as_object)
            .map(|path_item| {
                path_item
                    .keys()
                    .filter(|key| matches!(key.as_str(), "get" | "post" | "put" | "patch" | "delete" | "options" | "head"))
                    .count()
            })
            .sum::<usize>();
        if operation_count == 0 {
            violations.push(OpenApiValidationViolation::new(
                "$.paths",
                "operations_presence",
                "no HTTP operations were found under `paths`",
            ));
        }
    }

    violations
}

/// Returns `Ok(())` when preflight validation passes, otherwise returns all violations.
pub fn validate_openapi_preflight(document: &Value) -> Result<(), Vec<OpenApiValidationViolation>> {
    let violations = collect_openapi_preflight_violations(document);
    if violations.is_empty() {
        return Ok(());
    }
    Err(violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_missing_openapi_version() {
        let document = json!({
            "paths": {
                "/apps": {
                    "get": {}
                }
            }
        });

        let violations = collect_openapi_preflight_violations(&document);

        assert!(violations.iter().any(|violation| violation.path == "$.openapi"));
    }

    #[test]
    fn reports_swagger_v2_document() {
        let document = json!({
            "swagger": "2.0",
            "paths": {
                "/apps": {
                    "get": {}
                }
            }
        });

        let violations = collect_openapi_preflight_violations(&document);

        assert!(violations.iter().any(|violation| violation.path == "$.swagger"));
    }

    #[test]
    fn reports_missing_operations() {
        let document = json!({
            "openapi": "3.0.3",
            "paths": {}
        });

        let violations = collect_openapi_preflight_violations(&document);

        assert!(violations.iter().any(|violation| violation.rule == "operations_presence"));
    }

    #[test]
    fn accepts_minimal_valid_openapi3_document() {
        let document = json!({
            "openapi": "3.0.3",
            "paths": {
                "/apps": {
                    "get": {}
                }
            }
        });

        assert!(validate_openapi_preflight(&document).is_ok());
    }
}
