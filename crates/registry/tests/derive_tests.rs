use heroku_registry::Registry;

fn load_fixture() -> Registry {
    let schema = include_str!("data/schema_fixture.json");
    Registry::load_from_hyper_schema_str(schema).expect("load registry from fixture")
}

#[test]
fn derives_command_names_and_actions() {
    let reg = load_fixture();
    let names: Vec<_> = reg.commands.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"apps:list"), "expected apps:list, got {:?}", names);
    assert!(names.contains(&"apps:info"), "expected apps:info, got {:?}", names);
    assert!(names.contains(&"spaces:info"), "expected spaces:info, got {:?}", names);
    assert!(names.contains(&"apps:create"), "expected apps:create, got {:?}", names);
}

#[test]
fn resolves_positional_descriptions_with_anyof() {
    let reg = load_fixture();
    let cmd = reg.commands.iter().find(|c| c.name == "spaces:info").expect("spaces:info present");
    let desc = cmd.positional_help.get("space").cloned().unwrap_or_default();
    assert!(desc.contains("unique identifier of space"), "desc: {}", desc);
    assert!(desc.contains("unique name of space"), "desc: {}", desc);
    assert!(desc.contains("or"), "desc should join with 'or': {}", desc);
}

#[test]
fn resolves_flag_descriptions_and_required() {
    let reg = load_fixture();
    let cmd = reg.commands.iter().find(|c| c.name == "apps:create").expect("apps:create present");
    let flag = cmd.flags.iter().find(|f| f.name == "name").expect("name flag present");
    assert!(flag.required, "name should be required");
    let fdesc = flag.description.as_deref().unwrap_or("");
    assert!(fdesc.contains("unique name of app"), "flag desc not populated: {}", fdesc);
}

