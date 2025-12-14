#[cfg(test)]
mod tests {
    use super::super::schema::*;
    use oatty_types::ServiceId;
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

    #[test]
    fn assigns_unique_short_names_without_collisions() {
        let schema_json = r#"
        {
            "links": [
                {
                    "href": "/apps",
                    "method": "POST",
                    "title": "Create app",
                    "schema": {
                        "properties": {
                            "app": {"type": "string"},
                            "addon": {"type": "string"},
                            "api": {"type": "string"}
                        }
                    }
                }
            ]
        }
        "#;
        let value: serde_json::Value = serde_json::from_str(schema_json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let flags = &commands[0].flags;

        let mut short_names: Vec<&str> = flags
            .iter()
            .filter_map(|flag| flag.short_name.as_deref())
            .collect();
        short_names.sort();

        assert_eq!(short_names, vec!["a", "ad", "ap"]);
    }

    #[test]
    fn assigns_short_names_with_many_similar_prefixes() {
        let schema_json = r#"
        {
            "links": [
                {
                    "href": "/apps",
                    "method": "POST",
                    "title": "Create app",
                    "schema": {
                        "properties": {
                            "app": {"type": "string"},
                            "application": {"type": "string"},
                            "append": {"type": "string"},
                            "app-log": {"type": "string"}
                        }
                    }
                }
            ]
        }
        "#;
        let value: serde_json::Value = serde_json::from_str(schema_json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let flags = &commands[0].flags;

        let expected = vec![
            ("app", Some("a".to_string())),
            ("application", Some("ap".to_string())),
            ("append", Some("app".to_string())),
            ("app-log", Some("al".to_string())),
        ];

        for (name, short) in expected {
            let flag = flags.iter().find(|flag| flag.name == name).expect("flag missing");
            assert_eq!(flag.short_name, short, "unexpected short name for {name}");
            if let Some(ref value) = flag.short_name {
                assert!(
                    (1..=3).contains(&value.chars().count()),
                    "short name length must be between 1 and 3"
                );
            }
        }
    }
}
