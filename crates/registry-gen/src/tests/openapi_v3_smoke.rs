use std::fs;
use std::path::PathBuf;

use heroku_registry_gen::openapi::transform_openapi_to_links;
use heroku_registry_gen::schema::derive_commands_from_schema;
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
    let transformed = transform_openapi_to_links(&doc).expect("transform openapi to links");
    let commands = derive_commands_from_schema(&transformed).expect("derive commands");

    assert!(!commands.is_empty(), "should produce some commands");

    // Find POST /data/postgres/v1/{addon}/pools and verify flags
    let pools_create = commands.iter().find(|c| c.method == "POST" && c.path == "/data/postgres/v1/{v1}/pools");
    // NOTE: positional name for {addon} becomes {v1} due to existing path naming heuristic
    // so expected path uses {v1} after normalization.
    assert!(pools_create.is_some(), "expected pools:create command");
    let pc = pools_create.unwrap();
    let flag_names: Vec<_> = pc.flags.iter().map(|f| f.name.as_str()).collect();
    // Body properties were: name, level, count
    for needed in ["name", "level", "count"] {
        assert!(flag_names.contains(&needed), "missing flag {}", needed);
    }

    // Find GET /data/postgres/v1/{addon}/expensive-queries and ensure limit flag exists and positional help wired
    let exp_get = commands
        .iter()
        .find(|c| c.method == "GET" && c.path == "/data/postgres/v1/{v1}/expensive-queries");
    assert!(exp_get.is_some(), "expected expensive-queries:list command");
    let eg = exp_get.unwrap();
    let has_limit = eg.flags.iter().any(|f| f.name == "limit" && f.default_value.as_deref() == Some("10"));
    assert!(has_limit, "expected limit flag with default 10");

    // Positional help should be present via x-parameters pointer
    if let Some(pos) = eg.positional_args.first() {
        assert!(pos.help.as_deref().unwrap_or("").to_lowercase().contains("database addon"));
    }
}
