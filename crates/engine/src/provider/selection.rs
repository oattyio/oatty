use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::contract::ProviderContract;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SelectionSource {
    Explicit,
    ByTags,
    ByNames,
    #[default]
    RequiresChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSelection {
    pub value_field: String,
    pub display_field: String,
    #[serde(default)]
    pub id_field: Option<String>,
    #[serde(skip)]
    pub source: SelectionSource,
}

impl FieldSelection {
    pub fn explicit(value_field: String, display_field: String, id_field: Option<String>) -> Self {
        Self {
            value_field,
            display_field,
            id_field,
            source: SelectionSource::Explicit,
        }
    }

    pub fn with_source(mut self, source: SelectionSource) -> Self {
        self.source = source;
        self
    }
}

impl Default for FieldSelection {
    fn default() -> Self {
        FieldSelection {
            value_field: String::new(),
            display_field: String::new(),
            id_field: None,
            source: SelectionSource::RequiresChoice,
        }
    }
}

// Default is derived on SelectionSource

/// Infer a `FieldSelection` from an explicit mapping or a provider contract.
///
/// The precedence is:
/// - Use explicit selection if provided by the workflow input.
/// - Prefer fields tagged with `id`/`identifier` for value and `display`/`name` for display.
/// - Fall back to common names: `id` for value, `name` for display.
/// - Otherwise, choose first as value and second as display (if present), and require a choice.
///
/// Examples
/// ```rust
/// use heroku_engine::provider::{ProviderReturns, ReturnField};
/// use heroku_engine::provider::{infer_selection};
///
/// let contract = heroku_engine::provider::ProviderContract {
///     args: serde_json::Map::new(),
///     returns: ProviderReturns { fields: vec![
///         ReturnField { name: "uuid".into(), r#type: Some("string".into()), tags: vec!["identifier".into()] },
///         ReturnField { name: "display".into(), r#type: Some("string".into()), tags: vec!["display".into()] },
///     ]}
/// };
/// let sel = infer_selection(None, Some(&contract));
/// assert_eq!(sel.value_field, "uuid");
/// assert_eq!(sel.display_field, "display");
/// ```
pub fn infer_selection(
    explicit: Option<crate::model::SelectSpec>,
    contract: Option<&ProviderContract>,
) -> FieldSelection {
    if let Some(sel) = explicit {
        return FieldSelection::explicit(sel.value_field, sel.display_field, sel.id_field)
            .with_source(SelectionSource::Explicit);
    }

    let mut selection = FieldSelection {
        value_field: String::new(),
        display_field: String::new(),
        id_field: None,
        source: SelectionSource::RequiresChoice,
    };

    if let Some(contract) = contract {
        let (id_candidate, id_from_tags, display_candidate, display_from_tags) = scan_contract_for_candidates(contract);

        match (id_candidate, display_candidate) {
            (Some(vf), Some(df)) => {
                selection.value_field = vf.clone();
                selection.display_field = df;
                selection.id_field = Some(vf);
                selection.source = if id_from_tags || display_from_tags {
                    SelectionSource::ByTags
                } else {
                    SelectionSource::ByNames
                };
                return selection;
            }
            (Some(vf), None) => {
                selection.value_field = vf.clone();
                selection.display_field = "name".into();
                selection.id_field = Some(vf);
                selection.source = if id_from_tags {
                    SelectionSource::ByTags
                } else {
                    SelectionSource::ByNames
                };
                return selection;
            }
            (None, Some(df)) => {
                selection.value_field = "id".into();
                selection.display_field = df;
                selection.id_field = Some("id".into());
                selection.source = if display_from_tags {
                    SelectionSource::ByTags
                } else {
                    SelectionSource::ByNames
                };
                return selection;
            }
            (None, None) => {}
        }
    }

    if let Some(contract) = contract {
        if let Some(first) = contract.returns.fields.first() {
            selection.value_field = first.name.clone();
        }
        if let Some(second) = contract.returns.fields.get(1) {
            selection.display_field = second.name.clone();
        }
    }
    if selection.value_field.is_empty() {
        selection.value_field = "id".into();
    }
    if selection.display_field.is_empty() {
        selection.display_field = "name".into();
    }
    selection.id_field = Some(selection.value_field.clone());
    selection.source = SelectionSource::RequiresChoice;
    selection
}

fn scan_contract_for_candidates(contract: &ProviderContract) -> (Option<String>, bool, Option<String>, bool) {
    let mut id_candidate: Option<String> = None;
    let mut id_from_tags = false;
    let mut display_candidate: Option<String> = None;
    let mut display_from_tags = false;
    for field in &contract.returns.fields {
        if field.tags.iter().any(|t| t == "id" || t == "identifier") && id_candidate.is_none() {
            id_candidate = Some(field.name.clone());
            id_from_tags = true;
        }
        if field.name == "id" && id_candidate.is_none() {
            id_candidate = Some("id".to_string());
        }
        if field.tags.iter().any(|t| t == "display") && display_candidate.is_none() {
            display_candidate = Some(field.name.clone());
            display_from_tags = true;
        }
        if field.name == "name" && display_candidate.is_none() {
            display_candidate = Some("name".to_string());
        }
    }
    (id_candidate, id_from_tags, display_candidate, display_from_tags)
}

pub fn coerce_value(value: &Value, target_type: Option<&str>, selection: Option<&FieldSelection>) -> Value {
    let base = match (value, selection) {
        (Value::Object(map), Some(sel)) => map.get(&sel.value_field).cloned().unwrap_or(Value::Null),
        _ => value.clone(),
    };
    match target_type.unwrap_or("string") {
        "string" => match base {
            Value::String(s) => Value::String(s),
            Value::Null => Value::String(String::new()),
            other => Value::String(other.to_string()),
        },
        "number" => match base {
            Value::Number(n) => Value::Number(n),
            Value::String(s) => s
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            _ => Value::Null,
        },
        "boolean" => match base {
            Value::Bool(b) => Value::Bool(b),
            Value::String(s) => Value::Bool(matches!(s.as_str(), "true" | "1" | "yes")),
            Value::Number(n) => Value::Bool(n.as_i64().unwrap_or(0) != 0),
            _ => Value::Bool(false),
        },
        _ => base,
    }
}

#[cfg(test)]
mod doctests_like {
    use super::*;
    use serde_json::json;

    #[test]
    fn coerce_edge_cases() {
        let sel = FieldSelection::explicit("val".into(), "label".into(), Some("val".into()));
        let obj = json!({"val": "not-a-number"});
        assert_eq!(coerce_value(&obj, Some("number"), Some(&sel)), Value::Null);
        assert_eq!(coerce_value(&json!(0), Some("boolean"), None), json!(false));
        assert_eq!(coerce_value(&json!(1), Some("boolean"), None), json!(true));
        assert_eq!(coerce_value(&json!("yes"), Some("boolean"), None), json!(true));
    }
}
