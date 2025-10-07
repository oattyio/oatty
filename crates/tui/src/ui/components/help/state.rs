use heroku_types::CommandSpec;

#[derive(Debug, Default, Clone)]
pub struct HelpState {
    spec: Option<CommandSpec>,
}

impl HelpState {
    pub fn spec(&self) -> Option<&CommandSpec> {
        self.spec.as_ref()
    }

    pub fn set_spec(&mut self, spec: Option<CommandSpec>) {
        self.spec = spec;
    }
}
