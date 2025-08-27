use heroku_types::CommandSpec;

#[derive(Debug, Default, Clone)]
pub struct HelpState {
    visible: bool,
    spec: Option<CommandSpec>,
}

impl HelpState {
    pub fn toggle_visibility(&mut self, spec: Option<CommandSpec>) {
        self.visible = !self.visible;
        if self.visible {
            self.spec = spec;
        }
    }

    pub fn set_visibility(&mut self, val: bool) {
        self.visible = val;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn spec(&self) -> Option<&CommandSpec> {
        self.spec.as_ref()
    }

    pub fn set_spec(&mut self, spec: Option<CommandSpec>) {
        self.spec = spec;
    }
}
