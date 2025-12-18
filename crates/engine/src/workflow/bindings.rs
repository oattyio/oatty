//! Dependent provider argument resolution.
//!
//! Workflows can declare provider arguments that reference previously resolved inputs or
//! step outputs. This module centralizes the logic for evaluating those bindings against the
//! current [`RunContext`](RunContext), applying the author-specified
//! `on_missing` policies, and producing outcomes that upstream callers can act upon
//! (auto-resolve, prompt the user, allow manual entry, or fail fast).

use std::fmt;

use indexmap::IndexMap;
use serde_json::Value;

use crate::resolve::{RunContext, interpolate_value};
use oatty_types::workflow::{WorkflowMissingBehavior, WorkflowProviderArgumentBinding, WorkflowProviderArgumentValue};

/// Resolves provider arguments declared on a workflow input into concrete values or follow-up
/// actions for the caller.
///
/// The resolver walks through each `provider_args` entry, evaluates literal templates using the
/// [`RunContext`], materializes structured bindings (`from_step` / `from_input`), and produces a
/// [`ProviderBindingOutcome`] that captures the next action. Missing values respect the
/// `on_missing` policy attached to the binding (`prompt`, `skip`, or `fail`).
pub struct ProviderArgumentResolver<'context> {
    context: &'context RunContext,
}

impl<'context> ProviderArgumentResolver<'context> {
    /// Creates a new resolver with a reference to the active execution context.
    pub fn new(context: &'context RunContext) -> Self {
        Self { context }
    }

    /// Resolves the entire map of provider arguments and returns the resulting outcomes keyed by
    /// argument name.
    pub fn resolve_arguments(
        &self,
        arguments: &IndexMap<String, WorkflowProviderArgumentValue>,
    ) -> IndexMap<String, ProviderBindingOutcome> {
        arguments
            .iter()
            .map(|(argument_name, argument_value)| (argument_name.clone(), self.resolve_argument(argument_name.as_str(), argument_value)))
            .collect()
    }

    /// Resolves the value of a given argument based on its specification.
    ///
    /// # Parameters
    /// - `argument_name`: A string slice that holds the name of the argument to be resolved.
    /// - `argument_value`: A reference to a `WorkflowProviderArgumentValue` enum, which specifies the type of value for the argument (either a literal or a binding).
    ///
    /// # Returns
    /// A `ProviderBindingOutcome` enum instance that represents the result of the resolution process. This can either be:
    /// - `ProviderBindingOutcome::Resolved` containing the resolved value (in the case of a literal value or a successfully resolved binding).
    /// - The output of the `resolve_binding` method when handling a complex binding resolution process.
    ///
    /// # Behavior
    /// - For arguments specified as `WorkflowProviderArgumentValue::Literal`, the method interpolates the provided value (`template`) using the current context
    ///   and wraps the resolved value in a `ProviderBindingOutcome::Resolved`.
    /// - For arguments specified as `WorkflowProviderArgumentValue::Binding`, the method delegates resolution to the `resolve_binding` function, passing along
    ///   the argument name and the binding.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let argument_name = "example_arg";
    /// let argument_value = WorkflowProviderArgumentValue::Literal("Hello {{name}}".to_string());
    /// let outcome = obj.resolve_argument(argument_name, &argument_value);
    ///
    /// match outcome {
    ///     ProviderBindingOutcome::Resolved(value) => println!("Resolved value: {}", value),
    ///     _ => println!("Unhandled outcome"),
    /// }
    /// ```
    ///
    /// # Notes
    /// - The actual interpolation process is handled by the `interpolate_value` function, which applies contextual data to resolve placeholders in the string.
    /// - The `resolve_binding` method is expected to handle complex binding scenarios where the argument is tied to a specific binding configuration.
    ///
    /// # Errors
    /// This method doesn't directly return errors but wraps all outcomes into the `ProviderBindingOutcome`. Error handling and propagation depend on the implementation
    /// of related functions (`interpolate_value` and `resolve_binding`).
    pub fn resolve_argument(&self, argument_name: &str, argument_value: &WorkflowProviderArgumentValue) -> ProviderBindingOutcome {
        match argument_value {
            WorkflowProviderArgumentValue::Literal(template) => {
                let value = interpolate_value(&Value::String(template.clone()), self.context);
                ProviderBindingOutcome::Resolved(value)
            }
            WorkflowProviderArgumentValue::Binding(binding) => self.resolve_binding(argument_name, binding.clone()),
        }
    }

    /// Resolves a binding for a workflow provider argument by identifying its source and extracting the corresponding value.
    ///
    /// # Parameters
    /// - `argument_name`: A string slice representing the name of the argument being resolved.
    /// - `binding`: The `WorkflowProviderArgumentBinding` struct containing the source and rules for resolving the argument.
    ///
    /// # Returns
    /// A `ProviderBindingOutcome` which could be:
    /// - `Resolved`: If the binding successfully resolves to a value.
    /// - `Error`: If the binding fails due to either a misconfiguration or a missing value.
    ///
    /// # Logic
    /// 1. Deconstructs the `binding` to extract the configuration for the resolution process.
    /// 2. Determines the source (`BindingSource`) of the binding:
    ///     - If `from_step` is provided, the source is a step.
    ///     - If `from_input` is provided, the source is an input.
    ///     - If both `from_step` and `from_input` are supplied, the method returns an `Error` indicating the misconfiguration.
    ///     - If neither `from_step` nor `from_input` are provided, the method returns an `Error` indicating missing source information.
    /// 3. Attempts to retrieve the base value associated with the determined `BindingSource`:
    ///     - If the base value is missing, it's handled based on the `required` flag and the configured `on_missing` behavior, returning an appropriate outcome.
    /// 4. If the base value is found, applies the optional `path` to retrieve a nested value:
    ///     - If the nested value is unavailable or `null`, it's treated as a missing value based on `on_missing` behavior.
    /// 5. If the value is resolved successfully, returns a `ProviderBindingOutcome::Resolved` with the selected value.
    ///
    /// # Error Scenarios
    /// - If both `from_step` and `from_input` are provided, the method returns an `Error` indicating that a single source should be specified.
    /// - If neither `from_step` nor `from_input` are provided, the method returns an `Error` indicating a lack of source.
    /// - If the selected value is not found or unavailable at the specified path, the method delegates to `handle_missing`, which determines the appropriate outcome based on the `required` flag and `on_missing` configuration.
    ///
    /// # Dependencies
    /// - Relies on `self.context` to access step or input values.
    /// - Invokes `select_path` to extract nested values based on the provided path.
    /// - Uses `handle_missing` for handling cases where the value is not found or unavailable.
    ///
    /// # Structs and Enums Used
    /// - `WorkflowProviderArgumentBinding`: Structure containing binding configurations such as `from_step`, `from_input`, `path`, `required`, and `on_missing`.
    /// - `BindingSource`: Enum representing the source of the binding (step, input, or invalid/multiple).
    /// - `ProviderBindingOutcome`: Enum representing the result of the binding resolution (`Resolved` or `Error`).
    /// - `BindingFailure`: Struct encapsulating details about a binding error.
    /// - `MissingContext`: Enum representing the reason for a missing value (`NotFound` or `PathUnavailable`).
    ///
    /// # Example
    /// ```rust,ignore
    /// let binding = WorkflowProviderArgumentBinding {
    ///     from_step: Some("step_id".to_string()),
    ///     from_input: None,
    ///     path: Some("nested.property".to_string()),
    ///     required: Some(true),
    ///     on_missing: Some(MissingBehavior::Error),
    /// };
    ///
    /// let outcome = resolver.resolve_binding("my_argument", binding);
    ///
    /// match outcome {
    ///     ProviderBindingOutcome::Resolved(value) => println!("Resolved value: {:?}", value),
    ///     ProviderBindingOutcome::Error(error) => eprintln!("Binding error: {:?}", error),
    /// }
    /// ```
    fn resolve_binding(&self, argument_name: &str, binding: WorkflowProviderArgumentBinding) -> ProviderBindingOutcome {
        let WorkflowProviderArgumentBinding {
            from_step,
            from_input,
            path,
            required,
            on_missing,
        } = binding;

        let source = match (&from_step, &from_input) {
            (Some(step_id), None) => BindingSource::Step { step_id: step_id.clone() },
            (None, Some(input_name)) => BindingSource::Input {
                input_name: input_name.clone(),
            },
            (Some(step_id), Some(input_name)) => {
                return ProviderBindingOutcome::Error(BindingFailure {
                    argument: argument_name.to_string(),
                    source: Some(BindingSource::Multiple {
                        step_id: step_id.clone(),
                        input_name: input_name.clone(),
                    }),
                    message: "provider argument binding must reference either a step or an input, not both".into(),
                });
            }
            (None, None) => {
                return ProviderBindingOutcome::Error(BindingFailure {
                    argument: argument_name.to_string(),
                    source: None,
                    message: "provider argument binding is missing a source (from_step or from_input)".into(),
                });
            }
        };

        let required_flag = required.unwrap_or(false);
        let base_value = match &source {
            BindingSource::Step { step_id } => self.context.steps.get(step_id),
            BindingSource::Input { input_name } => self.context.inputs.get(input_name),
            BindingSource::Multiple { .. } => None,
        };

        let Some(value) = base_value else {
            return self.handle_missing(
                argument_name,
                source,
                path.clone(),
                required_flag,
                on_missing.clone(),
                MissingContext::NotFound,
            );
        };

        let selected_value = match select_path(value, path.as_deref()) {
            Some(candidate) if !candidate.is_null() => candidate,
            _ => {
                return self.handle_missing(
                    argument_name,
                    source,
                    path,
                    required_flag,
                    on_missing,
                    MissingContext::PathUnavailable,
                );
            }
        };

        ProviderBindingOutcome::Resolved(selected_value)
    }

    fn handle_missing(
        &self,
        argument_name: &str,
        source: BindingSource,
        path: Option<String>,
        required: bool,
        on_missing: Option<WorkflowMissingBehavior>,
        missing_context: MissingContext,
    ) -> ProviderBindingOutcome {
        let policy = on_missing.unwrap_or_else(|| default_missing_policy(required));

        let missing_reason = MissingReason {
            message: missing_context.to_string(),
            path,
        };

        match policy {
            WorkflowMissingBehavior::Prompt => ProviderBindingOutcome::Prompt(ArgumentPrompt {
                argument: argument_name.to_string(),
                source,
                required,
                reason: missing_reason,
            }),
            WorkflowMissingBehavior::Skip => ProviderBindingOutcome::Skip(SkipDecision {
                argument: argument_name.to_string(),
                source,
                reason: missing_reason,
            }),
            WorkflowMissingBehavior::Fail => ProviderBindingOutcome::Error(BindingFailure {
                argument: argument_name.to_string(),
                source: Some(source),
                message: missing_reason.message,
            }),
        }
    }
}

fn default_missing_policy(is_required: bool) -> WorkflowMissingBehavior {
    if is_required {
        WorkflowMissingBehavior::Fail
    } else {
        WorkflowMissingBehavior::Prompt
    }
}

#[derive(Debug, Clone, PartialEq)]
enum MissingContext {
    NotFound,
    PathUnavailable,
}

impl fmt::Display for MissingContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MissingContext::NotFound => formatter.write_str("binding source was not present in the run context"),
            MissingContext::PathUnavailable => formatter.write_str("binding path did not resolve to a value"),
        }
    }
}

/// Outcome produced when resolving a provider argument.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderBindingOutcome {
    /// The argument successfully resolved to a concrete JSON value.
    Resolved(Value),
    /// Additional user input is required to continue (Field Picker, manual override, etc.).
    Prompt(ArgumentPrompt),
    /// The binding may be skipped and the provider should fall back to manual entry or defaults.
    Skip(SkipDecision),
    /// The binding failed and should halt the workflow until the error is addressed.
    Error(BindingFailure),
}

/// Metadata describing why additional input is required.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgumentPrompt {
    /// Name of the argument being resolved (for example, `app` or `pipeline`).
    pub argument: String,
    /// Origin of the data we attempted to use.
    pub source: BindingSource,
    /// Whether the provider argument was marked as required.
    pub required: bool,
    /// Human-readable reason explaining why the binding could not be satisfied automatically.
    pub reason: MissingReason,
}

/// Metadata describing why a binding can be skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct SkipDecision {
    /// Name of the argument being skipped.
    pub argument: String,
    /// Origin of the data we attempted to use.
    pub source: BindingSource,
    /// Context for the skip (missing source, missing path, etc.).
    pub reason: MissingReason,
}

/// Details about a binding failure that should halt execution.
#[derive(Debug, Clone, PartialEq)]
pub struct BindingFailure {
    /// Name of the argument being resolved.
    pub argument: String,
    /// Optional origin of the data (if known).
    pub source: Option<BindingSource>,
    /// Human-readable failure description.
    pub message: String,
}

/// Structured description of why a binding could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingReason {
    /// Human-readable description of the missing context.
    pub message: String,
    /// Optional path that failed to resolve.
    pub path: Option<String>,
}

/**
Represents the source of a binding in a workflow definition. This enum is used
to determine where a specific binding references its value. It can point to
either a prior step output, a workflow input, or an invalid combination of both
(which is captured for diagnostic purposes).

# Variants

- `Step`:
  Represents that the binding references an output of a prior step in the workflow.
  - `step_id`: The identifier of the step being referenced.

- `Input`:
  Represents that the binding references an input of the workflow.
  - `input_name`: The name of the workflow input being referenced.

- `Multiple`:
  Represents an invalid binding that references both a step and a workflow input.
  This is captured for diagnostic purposes.
  - `step_id`: The identifier of the step being referenced.
  - `input_name`: The name of the workflow input being referenced.

# Examples

```rust
use oatty_engine::BindingSource;

let step_binding = BindingSource::Step { step_id: "step_1".to_string() };
let input_binding = BindingSource::Input { input_name: "file_path".to_string() };
let invalid_binding = BindingSource::Multiple {
    step_id: "step_2".to_string(),
    input_name: "config_value".to_string(),
};

assert!(matches!(step_binding, BindingSource::Step { .. }));
assert!(matches!(input_binding, BindingSource::Input { .. }));
assert!(matches!(invalid_binding, BindingSource::Multiple { .. }));
```

# Traits Implemented
- `Debug`: Allows for the `BindingSource` enum to be formatted using the `{:?}` formatter.
- `Clone`: Enables cloning of `BindingSource` instances.
- `PartialEq`: Allows for comparisons of equality between `BindingSource` instances.
- `Eq`: Indicates that `PartialEq` comparisons are not approximate but are strict equality checks.
*/
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingSource {
    /// Binding referenced a prior step output.
    Step { step_id: String },
    /// Binding referenced a workflow input.
    Input { input_name: String },
    /// Binding attempted to reference both a step and an input (invalid but captured for diagnostics).
    Multiple { step_id: String, input_name: String },
}
/// Selects a nested value from a `Value` object based on a specified path.
///
/// # Parameters
/// - `value`: A reference to the root `Value` object from which the path starts.
/// - `path`: An optional string slice defining the path to traverse. The path can contain keys
///   (for interacting with JSON objects) or indices (for interacting with JSON arrays).
///
/// # Returns
/// - Returns an `Option<Value>`:
///   - `Some(Value)` if the given path resolves successfully.
///   - `None` if the path does not correspond to any value.
///
/// # Path Format
/// - Paths are specified as a string, with keys and indices separated by delimiters
///   (e.g., `key1.key2[0]`). Parsing and segmenting functionality depends on implementation details
///   of the `parse_path_segments` function.
/// - Leading or trailing whitespace in the `path` is trimmed, and empty paths (`None` or `""`) result
///   in the function returning the original `value`.
///
/// # Behavior
/// - If the path is empty or not provided, the function immediately returns a clone of the original `value`.
/// - The function attempts to resolve each segment in order:
///   - If a segment is a key, it attempts to retrieve the corresponding value from a JSON object.
///   - If a segment is an index, it attempts to retrieve the corresponding element from a JSON array.
/// - If at any point the segment cannot be resolved (e.g., key is missing, index is out of bounds, or type mismatches),
///   the function returns `None`.
/// - A special case applies to the first segment:
///   - If the first segment is the key `"output"`, the function prioritizes resolving the `"output"` field.
///
/// # Example
/// ```rust,ignore
/// use serde_json::Value;
///
/// let json = serde_json::json!({
///     "output": {
///         "results": [
///             { "name": "John", "age": 30 },
///             { "name": "Jane", "age": 25 }
///         ]
///     }
/// });
///
/// // Retrieving the first result's name
/// let path = Some("output.results[0].name");
/// let result = select_path(&json, path);
/// assert_eq!(result, Some(Value::String("John".to_string())));
///
/// // Invalid path
/// let invalid_path = Some("nonexistent.key");
/// let invalid_result = select_path(&json, invalid_path);
/// assert_eq!(invalid_result, None);
///
/// // No path provided
/// let no_path_result = select_path(&json, None);
/// assert_eq!(no_path_result, Some(json));
/// ```
///
/// # Notes
/// - This function relies on an external `parse_path_segments` function to decompose the path into segments.
/// - Expected structure of `PathSegment`:
///   - `PathSegment::Key`: Represents a key for accessing objects.
///   - `PathSegment::Index`: Represents an index for accessing arrays.
/// - The function requires `value` to be cloneable as it may return a cloned child element.
fn select_path(value: &Value, path: Option<&str>) -> Option<Value> {
    crate::resolve::select_path(value, path)
}

/// Parses a given string path into a vector of `PathSegment` objects, which represent the segments of the path.
///
/// The path may contain dot-separated keys (e.g., "key1.key2"), as well as index accessors within square brackets
/// (e.g., "array[0]" or "map['key']"). Keys and index accessors are processed and transformed into `PathSegment`
/// enums, which distinguish between keys and indices.
///
/// # Parameters
/// - `path`: A string slice representing the path to be parsed. The string can include dot-separated segments
///   and/or square-bracketed elements.
///
/// # Returns
/// A `Vec<PathSegment>` containing the parsed path segments, where each segment is either a key as a string
/// or an index as a numeric value.
///
/// # Panics
/// This function does not panic but may return incomplete or empty segments if the input path is invalid
/// (e.g., unbalanced square brackets) or empty.
///
/// # Examples
/// ```ignore
/// // Assume PathSegment is defined as:
/// // enum PathSegment {
/// //     Key(String),
/// //     Index(usize),
/// // }
///
/// let path = "root.data[3].items[10]";
/// let segments = parse_path_segments(path);
/// assert_eq!(
///     segments,
///     vec![
///         PathSegment::Key("root".to_string()),
///         PathSegment::Key("data".to_string()),
///         PathSegment::Index(3),
///         PathSegment::Key("items".to_string()),
///         PathSegment::Index(10),
///     ]
/// );
/// ```
///
/// # Implementation Details
/// - Keys are delimited by dots (`.`). Consecutive dots or a trailing dot will create an empty key segment.
/// - Indexes within square brackets (`[]`) are converted to `PathSegment::Index` if they can be parsed as
///   `usize`. If the content within brackets is non-numeric or not empty (e.g., `'key'`), the content is treated
///   as a string key and converted to `PathSegment::Key`.
/// - Whitespace inside or around square brackets will be trimmed.
///
/// # Limitations
/// - The square bracket parsing does not currently support complex expressions or nested brackets.
/// - Quotes (`'` or `"`) are not handled specifically for keys inside brackets; all content is treated as-is.
#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;
    use oatty_types::workflow::{WorkflowMissingBehavior, WorkflowProviderArgumentBinding, WorkflowProviderArgumentValue};
    use serde_json::json;

    fn resolver_with_context(context: RunContext) -> ProviderArgumentResolver<'static> {
        // Leak the context for the lifetime of the resolver during tests. This keeps the resolver
        // signature ergonomic while avoiding cloning the entire context for each resolution.
        let boxed = Box::leak(Box::new(context));
        ProviderArgumentResolver::new(boxed)
    }

    #[test]
    fn resolves_literal_templates() {
        let mut context = RunContext::default();
        context.inputs.insert("app".into(), json!("example-app"));
        let resolver = resolver_with_context(context);

        let outcome = resolver.resolve_argument("app", &WorkflowProviderArgumentValue::Literal("${{ inputs.app }}".into()));

        assert_eq!(outcome, ProviderBindingOutcome::Resolved(json!("example-app")));
    }

    #[test]
    fn resolves_binding_from_input_path() {
        let mut context = RunContext::default();
        context.inputs.insert(
            "app".into(),
            json!({
                "id": "app-123",
                "name": "example-app"
            }),
        );
        let resolver = resolver_with_context(context);

        let binding = WorkflowProviderArgumentBinding {
            from_step: None,
            from_input: Some("app".into()),
            path: Some("id".into()),
            required: Some(true),
            on_missing: Some(WorkflowMissingBehavior::Fail),
        };
        let outcome = resolver.resolve_argument("app", &WorkflowProviderArgumentValue::Binding(binding));

        assert_eq!(outcome, ProviderBindingOutcome::Resolved(json!("app-123")));
    }

    #[test]
    fn resolves_binding_from_step_output() {
        let mut context = RunContext::default();
        context.steps.insert(
            "create".into(),
            json!({
                "id": "app-456",
                "output": {
                    "name": "billing-app"
                }
            }),
        );
        let resolver = resolver_with_context(context);

        let binding = WorkflowProviderArgumentBinding {
            from_step: Some("create".into()),
            from_input: None,
            path: Some("output.name".into()),
            required: Some(false),
            on_missing: None,
        };
        let outcome = resolver.resolve_argument("app", &WorkflowProviderArgumentValue::Binding(binding));

        assert_eq!(outcome, ProviderBindingOutcome::Resolved(json!("billing-app")));
    }

    #[test]
    fn prompts_when_binding_source_missing() {
        let context = RunContext::default();
        let resolver = resolver_with_context(context);

        let binding = WorkflowProviderArgumentBinding {
            from_step: Some("missing".into()),
            from_input: None,
            path: Some("output.id".into()),
            required: Some(false),
            on_missing: None,
        };
        let outcome = resolver.resolve_argument("app", &WorkflowProviderArgumentValue::Binding(binding));

        assert!(matches!(outcome, ProviderBindingOutcome::Prompt(_)));
    }

    #[test]
    fn skips_when_policy_is_skip() {
        let context = RunContext::default();
        let resolver = resolver_with_context(context);

        let binding = WorkflowProviderArgumentBinding {
            from_step: Some("missing".into()),
            from_input: None,
            path: Some("output.id".into()),
            required: Some(false),
            on_missing: Some(WorkflowMissingBehavior::Skip),
        };
        let outcome = resolver.resolve_argument("app", &WorkflowProviderArgumentValue::Binding(binding));

        assert!(matches!(outcome, ProviderBindingOutcome::Skip(_)));
    }

    #[test]
    fn resolves_argument_map() {
        let mut context = RunContext::default();
        context.inputs.insert("app".into(), json!("example-app"));
        let resolver = resolver_with_context(context);

        let arguments = indexmap! {
            "app".to_string() => WorkflowProviderArgumentValue::Literal("${{ inputs.app }}".into())
        };
        let resolved = resolver.resolve_arguments(&arguments);
        assert_eq!(resolved["app"], ProviderBindingOutcome::Resolved(json!("example-app")));
    }

    #[test]
    fn errors_when_binding_refs_multiple_sources() {
        let context = RunContext::default();
        let resolver = resolver_with_context(context);

        let binding = WorkflowProviderArgumentBinding {
            from_step: Some("s1".into()),
            from_input: Some("app".into()),
            path: None,
            required: None,
            on_missing: None,
        };

        let outcome = resolver.resolve_argument("app", &WorkflowProviderArgumentValue::Binding(binding));
        assert!(matches!(outcome, ProviderBindingOutcome::Error(failure) if failure.message.contains("either a step or an input")));
    }
}
