#[cfg(test)]
mod tests {
    use super::super::schema::*;
    use serde_json::json;

    #[test]
    fn test_get_description_recursive() {
        let root_json = r##"
        {
            "definitions": {
                "id": {"description": "unique id"},
                "email": {"description": "email address"},
                "nested": {
                    "anyOf": [
                        {"description": "nested1"},
                        {"$ref": "#/definitions/id"}
                    ]
                },
                "union": {
                    "anyOf": [
                        {"$ref": "#/definitions/email"},
                        {"$ref": "#/definitions/nested"}
                    ]
                }
            }
        }
"##;
        let root: serde_json::Value = serde_json::from_str(root_json).unwrap();

        let schema = json!({"$ref": "#/definitions/union"});
        assert_eq!(
            get_description(&schema, &root),
            Some("email address or nested1 or unique id".to_string())
        );

        let schema_direct = json!({"description": "direct"});
        assert_eq!(
            get_description(&schema_direct, &root),
            Some("direct".to_string())
        );

        let schema_empty = json!({});
        assert_eq!(get_description(&schema_empty, &root), None);
    }

    #[test]
    fn test_get_type() {
        let root: serde_json::Value = json!({
            "definitions": {
                "str": {"type": "string"},
                "union": {
                    "anyOf": [
                        {"$ref": "#/definitions/str"},
                        {"type": "string"}
                    ]
                },
                "mixed": {
                    "anyOf": [
                        {"type": "string"},
                        {"type": "integer"}
                    ]
                }
            }
        });

        let schema = json!({"$ref": "#/definitions/union"});
        assert_eq!(get_type(&schema, &root), "string");

        let schema_mixed = json!({"$ref": "#/definitions/mixed"});
        assert_eq!(get_type(&schema_mixed, &root), "string"); // default

        let schema_no_type = json!({});
        assert_eq!(get_type(&schema_no_type, &root), "string");
    }

    #[test]
    fn test_get_enum_values() {
        let root: serde_json::Value = json!({
            "definitions": {
                "enum1": {"enum": ["a", "b"]},
                "enum2": {"enum": ["c"]},
                "union": {
                    "anyOf": [
                        {"$ref": "#/definitions/enum1"},
                        {"$ref": "#/definitions/enum2"}
                    ]
                }
            }
        });

        let schema = json!({"$ref": "#/definitions/union"});
        let enums = get_enum_values(&schema, &root);
        assert!(enums.contains(&"a".to_string()));
        assert!(enums.contains(&"b".to_string()));
        assert!(enums.contains(&"c".to_string()));
        assert_eq!(enums.len(), 3);
    }

    #[test]
    fn test_get_default() {
        let root: serde_json::Value = json!({
            "definitions": {
                "def1": {"default": "val1"},
                "def2": {"default": true},
                "union": {
                    "anyOf": [
                        {"$ref": "#/definitions/def1"},
                        {"$ref": "#/definitions/def2"}
                    ]
                }
            }
        });

        let schema = json!({"$ref": "#/definitions/union"});
        assert_eq!(get_default(&schema, &root), Some("val1".to_string())); // first
    }
}
