#[cfg(test)]
mod tests {
    use super::super::schema::*;
    use serde_json::json;

    #[test]
    fn test_workflow_flags() {
        let mut commands = Vec::new();
        crate::workflow::add_workflow_commands(&mut commands);
        
        let run_cmd = commands
            .iter()
            .find(|cmd| cmd.name == "workflow:run")
            .expect("workflow:run command not found");
        
        assert_eq!(run_cmd.flags.len(), 2);
        
        let file_flag = run_cmd
            .flags
            .iter()
            .find(|f| f.name == "file")
            .expect("file flag not found");
        assert_eq!(file_flag.short_name, Some("f".to_string()));
        assert_eq!(file_flag.description, Some("Path to workflow YAML/JSON".to_string()));
        
        let name_flag = run_cmd
            .flags
            .iter()
            .find(|f| f.name == "name")
            .expect("name flag not found");
        assert_eq!(name_flag.short_name, Some("n".to_string()));
        assert_eq!(name_flag.description, Some("Workflow name within the file".to_string()));
    }

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
        let cmds = derive_commands_from_schema(&v).unwrap();
        
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
