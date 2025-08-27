#[derive(Debug)]
pub struct TableState {
    pub show: bool,
    pub offset: usize,
    pub result_json: Option<serde_json::Value>,
}

impl Default for TableState {
    fn default() -> Self {
        TableState {
            show: false,
            offset: 0,
            result_json: None,
        }
    }
}

impl TableState {
    // Selectors
    pub fn is_visible(&self) -> bool {
        self.show
    }
    pub fn count_offset(&self) -> usize {
        self.offset
    }
    pub fn selected_result_json(&self) -> Option<&serde_json::Value> {
        self.result_json.as_ref()
    }

    // Reducers
    pub fn toggle_show(&mut self) {
        self.show = !self.show;
        if self.show {
            self.offset = 0;
        }
    }

    pub fn apply_show(&mut self, show: bool) {
        self.show = show;
        if show {
            self.offset = 0;
        }
    }

    pub fn apply_result_json(&mut self, value: Option<serde_json::Value>) {
        self.result_json = value;
    }

    pub fn reduce_scroll(&mut self, delta: isize) {
        let new_offset = if delta > 0 {
            self.offset.saturating_add(delta as usize)
        } else {
            self.offset.saturating_sub((-delta) as usize)
        };
        self.offset = new_offset;
    }

    pub fn reduce_home(&mut self) {
        self.offset = 0;
    }

    pub fn reduce_end(&mut self) {
        self.offset = usize::MAX;
    }
}
