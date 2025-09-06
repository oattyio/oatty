#[cfg(test)]
mod tests {
    use heroku_registry_gen::provider_resolver::resolve_and_infer_providers;
    use heroku_types::{CommandFlag, CommandSpec, PositionalArgument, ServiceId, ValueProvider};

    #[test]
    fn resolves_positional_provider_from_path_with_verification() {
        // Arrange: have a group with a list command and a command with a positional path
        let list = CommandSpec {
            group: "apps".into(),
            name: "list".into(),
            summary: "list".into(),
            positional_args: vec![],
            flags: vec![],
            method: "GET".into(),
            path: "/apps".into(),
            ranges: vec![],
            providers: vec![],
            service_id: ServiceId::CoreApi,
        };
        let mut info = CommandSpec {
            group: "apps".into(),
            name: "info".into(),
            summary: "info".into(),
            positional_args: vec![PositionalArgument { name: "app".into(), help: None, provider: None }],
            flags: vec![],
            method: "GET".into(),
            path: "/apps/{app}".into(),
            ranges: vec![],
            providers: vec![],
            service_id: ServiceId::CoreApi,
        };
        let mut commands = vec![list, info.clone()];

        // Act
        resolve_and_infer_providers(&mut commands);

        // Assert: positional provider is set on the positional argument
        let info_after = commands.iter().find(|c| c.group == "apps" && c.name == "info").unwrap();
        assert!(matches!(
            info_after.positional_args[0].provider,
            Some(ValueProvider::Command { ref command_id }) if command_id == "apps:list"
        ));
    }

    #[test]
    fn resolves_flag_provider_from_synonym_with_verification() {
        // Arrange: have a group with a list command and a command with a flag name mapping to that group
        let list = CommandSpec {
            group: "addons".into(),
            name: "list".into(),
            summary: "list".into(),
            positional_args: vec![],
            flags: vec![],
            method: "GET".into(),
            path: "/addons".into(),
            ranges: vec![],
            providers: vec![],
            service_id: ServiceId::CoreApi,
        };
        let mut update = CommandSpec {
            group: "addons".into(),
            name: "config:update".into(),
            summary: "update".into(),
            positional_args: vec![],
            flags: vec![CommandFlag { name: "addon".into(), short_name: None, required: false, r#type: "string".into(), enum_values: vec![], default_value: None, description: None, provider: None }],
            method: "PATCH".into(),
            path: "/addons/{addon}/config".into(),
            ranges: vec![],
            providers: vec![],
            service_id: ServiceId::CoreApi,
        };
        let mut commands = vec![list, update.clone()];

        // Act
        resolve_and_infer_providers(&mut commands);

        // Assert: flag provider is set on the flag
        let update_after = commands.iter().find(|c| c.group == "addons" && c.name == "config:update").unwrap();
        let flag = update_after.flags.iter().find(|f| f.name == "addon").unwrap();
        assert!(matches!(flag.provider, Some(ValueProvider::Command { ref command_id }) if command_id == "addons:list"));
    }
}
