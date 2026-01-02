use std::fs;
use std::path::PathBuf;

use oatty_registry_gen::openapi::derive_commands_from_openapi;
#[test]
fn openapi_v3_smoke_generates_commands() {
    // Load the example swagger.yaml we downloaded during planning
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let yaml_path = root.join("../../schemas/example-swagger.yaml");
    if !yaml_path.exists() {
        // Skip if fixture not present in this environment
        eprintln!("fixture missing: {}", yaml_path.display());
        return;
    }
    let text = fs::read_to_string(&yaml_path).expect("read example-swagger.yaml");
    let doc: serde_json::Value = serde_yaml::from_str(&text).expect("parse yaml as json value");
    let commands = derive_commands_from_openapi(&doc).expect("derive commands");

    assert!(!commands.is_empty(), "should produce some commands");

    // Find POST /data/postgres/v1/{addon}/pools and verify flags
    let pools_create_command = commands.iter().find(|c| {
        c.http()
            .map(|http| http.method == "POST" && http.path == "/data/postgres/v1/{addon}/pools")
            .unwrap_or(false)
    });
    assert!(pools_create_command.is_some(), "expected pools:create command");
    let pools_command = pools_create_command.unwrap();
    let flag_names: Vec<_> = pools_command.flags.iter().map(|f| f.name.as_str()).collect();
    // Body properties were: name, level, count
    for needed in ["name", "level", "count"] {
        assert!(flag_names.contains(&needed), "missing flag {}", needed);
    }

    // Find GET /data/postgres/v1/{addon}/expensive-queries and ensure limit flag exists
    let expensive_queries_command = commands.iter().find(|c| {
        c.http()
            .map(|http| http.method == "GET" && http.path == "/data/postgres/v1/{addon}/expensive-queries")
            .unwrap_or(false)
    });
    assert!(expensive_queries_command.is_some(), "expected expensive-queries:list command");
    let expensive_queries_command = expensive_queries_command.unwrap();
    let has_limit = expensive_queries_command
        .flags
        .iter()
        .any(|f| f.name == "limit" && f.default_value.as_deref() == Some("10"));
    assert!(has_limit, "expected limit flag with default 10");

    // Strict draft-04: no non-standard path param metadata; positional help may be absent.
}
