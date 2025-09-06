#[cfg(test)]
mod tests {
    use super::super::schema::*;
    use heroku_types::ServiceId;
    use serde_json::json;

    #[test]
    fn test_derive_commands_from_schema() -> anyhow::Result<()> {
        let schema_json = r#"
        {
            "links": [
                {
                    "href": "/apps",
                    "method": "GET",
                    "title": "List apps",
                    "description": "List all apps"
                }
            ]
        }
        "#;
        let v: serde_json::Value = serde_json::from_str(schema_json)?;
        let cmds = derive_commands_from_schema(&v, ServiceId::CoreApi)?;
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "list");
        assert_eq!(cmds[0].summary, "List all apps");
        Ok(())
    }

    #[test]
    fn test_path_and_vars_with_help() {
        let root: serde_json::Value = json!({
            "/definitions/param": {
                "anyOf": [
                    {"description": "desc1"},
                    {"description": "desc2"}
                ]
            }
        });

        let (path, args) = path_and_vars_with_help("/test/{ (#/definitions/param) }", &root);
        assert_eq!(path, "/test/{param}");
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "param");
        assert_eq!(args[0].help.as_deref(), Some("desc1 or desc2"));
    }
}
