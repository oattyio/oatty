#[cfg(test)]
mod tests {
    use super::super::schema::*;
    use heroku_types::ServiceId;
    use serde_json::json;

    #[test]
    #[test]
    fn test_schema_flags() {
        let schema_json = r#"
        {
            "links": [
                {
                    "href": "/apps",
                    "method": "POST",
                    "title": "Create app",
                    "schema": {
                        "properties": {
                            "name": {"type": "string", "description": "App name"},
                            "region": {"type": "string", "description": "App region"}
                        },
                        "required": ["name"]
                    }
                }
            ]
        }
        "#;
        let v: serde_json::Value = serde_json::from_str(schema_json).unwrap();
        let cmds = derive_commands_from_schema(&v, ServiceId::CoreApi).unwrap();
        
        assert_eq!(cmds.len(), 1);
        let cmd = &cmds[0];
        assert_eq!(cmd.name, "create");
        assert_eq!(cmd.flags.len(), 2);
        
        let name_flag = cmd
            .flags
            .iter()
            .find(|f| f.name == "name")
            .expect("name flag not found");
        assert_eq!(name_flag.short_name, Some("n".to_string()));
        assert_eq!(name_flag.required, true);
        
        let region_flag = cmd
            .flags
            .iter()
            .find(|f| f.name == "region")
            .expect("region flag not found");
        assert_eq!(region_flag.short_name, Some("r".to_string()));
        assert_eq!(region_flag.required, false);
    }
}
