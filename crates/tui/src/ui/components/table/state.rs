use serde_json::Value;

use crate::ui::utils::infer_columns_from_json;

#[derive(Debug, Default)]
pub struct TableState {
    show: bool,
    offset: usize,
    result_json: Option<serde_json::Value>,
    cached_columns: Option<Vec<String>>,
}

// Default derived above

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
    pub fn cached_columns(&mut self) -> Option<&Vec<String>> {
        self.result_json.as_ref()?;
        if self.cached_columns.is_some() {
            return self.cached_columns.as_ref();
        }

        let json = self.result_json.as_ref().unwrap();
        let has_array = match json {
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => m.values().any(|v| matches!(v, Value::Array(_))),
            _ => false,
        };
        let cols = if has_array {
            Some(infer_columns_from_json(json))
        } else {
            None
        };
        self.cached_columns = cols;
        self.cached_columns.as_ref()
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
        self.cached_columns = None;
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
