use rat_focus::FocusFlag;
use ratatui::{
    layout::Constraint,
    style::Style,
    widgets::{Cell, Row},
};
use serde_json::Value;

use crate::ui::{
    theme::{
        helpers::{table_header_style, table_row_style},
        roles::Theme as UiTheme,
    },
    utils::{ColumnWithSize, infer_columns_with_sizes_from_json, is_status_like, render_value, status_color_for_value},
};

#[derive(Debug, Default)]
pub struct TableState<'a> {
    visible: bool,
    offset: usize,
    selected: usize,
    visible_rows: usize,
    result_json: Option<serde_json::Value>,
    rows: Option<Vec<Row<'a>>>,
    columns: Option<Vec<ColumnWithSize>>,
    column_constraints: Option<Vec<Constraint>>,
    headers: Option<Vec<Cell<'a>>>,
    pub grid_f: FocusFlag,
}

// Default derived above

impl<'a> TableState<'_> {
    // Selectors
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn count_offset(&self) -> usize {
        self.offset
    }
    pub fn selected_index(&self) -> usize {
        self.selected
    }
    pub fn visible_rows(&self) -> usize {
        self.visible_rows
    }
    pub fn set_visible_rows(&mut self, rows: usize) {
        self.visible_rows = rows;
    }
    pub fn selected_result_json(&self) -> Option<&serde_json::Value> {
        self.result_json.as_ref()
    }
    pub fn rows(&self) -> Option<&Vec<Row<'_>>> {
        self.rows.as_ref()
    }
    pub fn columns(&self) -> Option<&Vec<ColumnWithSize>> {
        self.columns.as_ref()
    }
    pub fn column_constraints(&self) -> Option<&Vec<Constraint>> {
        self.column_constraints.as_ref()
    }
    pub fn headers(&self) -> Option<&Vec<Cell<'_>>> {
        self.headers.as_ref()
    }
    pub fn grid_focus(&self) -> &FocusFlag {
        &self.grid_f
    }
    pub fn selected_data(&self) -> Option<&Value> {
        if let Some(json_array) = Self::array_from_json(self.result_json.as_ref()) {
            return json_array.get(self.selected);
        }
        None
    }

    // Reducers
    pub fn toggle_show(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.offset = 0;
            self.selected = 0;
            self.grid_f.set(true);
        }
    }

    pub fn apply_visible(&mut self, show: bool) {
        // Only reset and focus grid when transitioning from hidden -> visible.
        if show {
            if !self.visible {
                self.visible = true;
                self.offset = 0;
                self.selected = 0;
                self.grid_f.set(true);
            }
        } else {
            self.visible = false;
        }
    }

    pub fn apply_result_json(&mut self, value: Option<Value>, theme: &dyn UiTheme) {
        let json_array = Self::array_from_json(value.as_ref());
        self.columns = self.create_columns(json_array);
        self.rows = self.create_rows(json_array, theme);
        self.headers = self.create_headers(theme);
        self.column_constraints = self.create_constraints();
        self.result_json = value;
        self.offset = 0;
        self.selected = 0;
    }

    pub fn reduce_scroll(&mut self, delta: isize) {
        if let Some(rows) = self.rows.as_ref() {
            let len = rows.len();
            if len == 0 {
                self.offset = 0;
                self.selected = 0;
                return;
            }
            let new_selected = if delta >= 0 {
                self.selected.saturating_add(delta as usize).min(len.saturating_sub(1))
            } else {
                self.selected.saturating_sub((-delta) as usize)
            };

            // Adjust offset only if selection moves outside the viewport
            let vis = self.visible_rows.max(1);
            let mut new_offset = self.offset;
            if new_selected < self.offset {
                new_offset = new_selected;
            } else if new_selected >= self.offset + vis {
                new_offset = new_selected.saturating_sub(vis - 1);
            }

            self.selected = new_selected;
            self.offset = new_offset;
        }
    }

    pub fn reduce_home(&mut self) {
        self.offset = 0;
        self.selected = 0;
    }

    pub fn reduce_end(&mut self) {
        if let Some(rows) = self.rows.as_ref() {
            let len = rows.len();
            if len == 0 {
                self.offset = 0;
                self.selected = 0;
            } else {
                self.offset = len.saturating_sub(1);
                self.selected = self.offset;
            }
        } else {
            self.offset = 0;
            self.selected = 0;
        }
    }

    fn create_rows(&self, maybe_value: Option<&[Value]>, theme: &dyn UiTheme) -> Option<Vec<Row<'a>>> {
        if let Some(value) = maybe_value
            && self.columns.is_some()
        {
            let columns: &Vec<ColumnWithSize> = self.columns.as_ref().unwrap();
            let mut rows: Vec<Row> = vec![];
            for (idx, item) in value.iter().enumerate() {
                let mut cells: Vec<Cell> = Vec::with_capacity(columns.len());
                for col in columns.iter() {
                    let key = &col.key;
                    let val = item.get(key).unwrap_or(&Value::Null);
                    let txt = render_value(key, val);
                    let mut style = theme.text_primary_style();
                    if is_status_like(key)
                        && let Some(color) = status_color_for_value(&txt, theme)
                    {
                        style = Style::default().fg(color);
                    }
                    cells.push(Cell::from(txt).style(style));
                }
                // Alternating row backgrounds using theme helper (no dim modifier).
                let row_style = table_row_style(theme, idx);
                rows.push(Row::new(cells).style(row_style));
            }
            return Some(rows);
        }
        None
    }

    fn create_columns(&self, value: Option<&[Value]>) -> Option<Vec<ColumnWithSize>> {
        if let Some(json) = value {
            return Some(infer_columns_with_sizes_from_json(json, 200));
        }
        None
    }

    fn create_constraints(&self) -> Option<Vec<Constraint>> {
        if let Some(columns) = self.columns.as_ref() {
            let mut widths: Vec<Constraint> = Vec::new();
            if columns.is_empty() {
                widths.push(Constraint::Percentage(100));
            } else {
                for col in columns.iter() {
                    // Use measured max length with a small padding, with a sensible floor/ceiling
                    let w = (col.max_len + 2).clamp(4, 60) as u16;
                    widths.push(Constraint::Min(w));
                }
            }
            return Some(widths);
        }
        None
    }

    fn create_headers(&self, theme: &dyn UiTheme) -> Option<Vec<Cell<'a>>> {
        if let Some(columns) = self.columns.as_ref() {
            let headers: Vec<Cell> = columns
                .iter()
                .map(|col| Cell::from(col.name.clone()).style(table_header_style(theme)))
                .collect();

            return Some(headers);
        }
        None
    }

    fn array_from_json(value: Option<&Value>) -> Option<&[Value]> {
        if let Some(json) = value {
            let arr_opt = match json {
                Value::Array(a) => Some(a.as_slice()),
                Value::Object(m) => m.values().find_map(|v| match v {
                    Value::Array(a) => Some(a.as_slice()),
                    _ => None,
                }),
                _ => None,
            };
            return arr_opt;
        }
        None
    }
}
