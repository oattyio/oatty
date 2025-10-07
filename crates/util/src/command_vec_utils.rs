use heroku_types::CommandSpec;

pub fn sort_and_dedup_commands(commands: &mut Vec<CommandSpec>) {
    // multi-sort: group then name
    commands.sort_by(|a, b| {
        if a.group != b.group {
            a.group.cmp(&b.group)
        } else {
            a.name.cmp(&b.name)
        }
    });
    commands.dedup_by(|a, b| {
        if a.group != b.group {
            return false;
        }
        if let (Some(a_http), Some(b_http)) = (a.http(), b.http()) {
            return a.name == b.name && a_http.method == b_http.method && a_http.path == b_http.path;
        }
        false
    });
}
