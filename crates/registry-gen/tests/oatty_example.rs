use std::path::PathBuf;

use oatty_registry_gen::io::{ManifestInput, generate_catalog};

#[test]
fn imports_owned_example_with_query_body_path_and_response_schemas() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/samples/oatty-example.json");
    let catalog =
        generate_catalog(ManifestInput::new(Some(path), None, Some("example".to_string()))).expect("import the distributed Oatty example");

    assert_eq!(catalog.base_urls, vec!["http://127.0.0.1:4010"]);
    let manifest = catalog.manifest.expect("generated manifest");
    assert_eq!(manifest.commands.len(), 3);
    let find_command = |method: &str, path: &str| {
        manifest
            .commands
            .iter()
            .find(|command| command.http().is_some_and(|http| http.method == method && http.path == path))
            .expect("expected HTTP operation")
    };

    let list = find_command("GET", "/projects");
    let limit = list.flags.iter().find(|flag| flag.name == "limit").expect("query flag");
    assert_eq!(limit.r#type, "integer");
    assert_eq!(limit.default_value.as_deref(), Some("20"));
    assert_eq!(list.http().expect("HTTP command").list_response_path.as_deref(), Some("."));

    let create = find_command("POST", "/projects");
    let name = create.flags.iter().find(|flag| flag.name == "name").expect("body flag");
    assert!(name.required);
    assert_eq!(name.r#type, "string");

    let detail = find_command("GET", "/projects/{project_id}");
    assert_eq!(detail.positional_args.len(), 1);
    assert_eq!(detail.positional_args[0].name, "project_id");
    let schema = detail
        .http()
        .expect("HTTP command")
        .output_schema
        .as_ref()
        .expect("response schema");
    let properties = schema.properties.as_ref().expect("resolved Project properties");
    assert!(properties.contains_key("id"));
    assert!(properties.contains_key("name"));
}
