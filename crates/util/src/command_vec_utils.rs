use oatty_types::CommandSpec;

pub fn sort_and_dedup_commands(commands: &mut Vec<CommandSpec>) {
    // multi-sort: group then name
    commands.sort_unstable_by(|a, b| {
        if a.group != b.group {
            a.group.cmp(&b.group)
        } else {
            a.name.cmp(&b.name)
        }
    });
    commands.reserve(commands.len());
    commands.dedup_by(|a, b| {
        a.group == b.group
            && a.name == b.name
            && match (a.http(), b.http()) {
                (Some(a_http), Some(b_http)) => a_http.method == b_http.method && a_http.path == b_http.path,
                _ => false,
            }
    });
}
