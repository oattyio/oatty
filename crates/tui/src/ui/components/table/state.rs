use crate::ui::theme::Theme;
use crate::ui::utils::normalize_result_payload;
use crate::ui::{
    components::pagination::state::PaginationState,
    theme::{
        roles::Theme as UiTheme,
        theme_helpers::{table_header_style, table_row_style},
    },
    utils::{
        ColumnWithSize, get_scored_keys, infer_columns_with_sizes_from_json, is_status_like, normalize_header, render_value,
        status_color_for_value,
    },
};
use oatty_types::ExecOutcome;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span};
use ratatui::widgets::{ListState, TableState};
use ratatui::{
    layout::Constraint,
    style::Style,
    widgets::{Cell, Row},
};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct ResultsTableState<'a> {
    result_json: Option<Value>,
    rows: Option<Vec<Row<'a>>>,
    columns: Option<Vec<ColumnWithSize>>,
    column_constraints: Option<Vec<Constraint>>,
    headers: Option<Vec<Cell<'a>>>,
    kv_entries: Vec<KeyValueEntry>,
    // table and list states are supplied here
    // for use by the caller to render the table
    pub table_state: TableState,
    pub list_state: ListState,
    pub pagination_state: PaginationState,
    pub container_focus: FocusFlag,
    pub grid_f: FocusFlag,
    pub mouse_over_idx: Option<usize>,
}

impl<'a> ResultsTableState<'_> {
    pub fn selected_result_json(&self) -> Option<&Value> {
        self.result_json.as_ref()
    }
    pub fn rows(&self) -> Option<&Vec<Row<'_>>> {
        self.rows.as_ref()
    }
    pub fn column_constraints(&self) -> Option<&Vec<Constraint>> {
        self.column_constraints.as_ref()
    }
    pub fn headers(&self) -> Option<&Vec<Cell<'_>>> {
        self.headers.as_ref()
    }
    pub fn selected_data(&self, idx: usize) -> Option<&Value> {
        if let Some(json_array) = Self::array_from_json(self.result_json.as_ref()) {
            return json_array.get(idx);
        }
        None
    }

    pub fn kv_entries(&self) -> &[KeyValueEntry] {
        &self.kv_entries
    }
    pub fn set_kv_entries(&mut self, entries: Vec<KeyValueEntry>) {
        self.kv_entries = entries;
    }

    pub fn selected_kv_entry(&self, idx: usize) -> Option<&KeyValueEntry> {
        if self.kv_entries.is_empty() {
            return None;
        }
        let index = idx.min(self.kv_entries.len() - 1);
        self.kv_entries.get(index)
    }

    pub fn apply_result_json(&mut self, value: Option<Value>, theme: &dyn UiTheme, rerank_columns: bool) {
        self.result_json = value;
        let json_array = Self::array_from_json(self.result_json.as_ref());
        self.columns = self.create_columns(json_array, rerank_columns);
        self.rows = self.create_rows(json_array, theme);
        self.headers = self.create_headers(theme);
        self.column_constraints = self.create_constraints();
        self.kv_entries = self.create_kv_entries(self.result_json.as_ref());
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
                    let value = item.get(key).unwrap_or(&Value::Null);
                    let rendered_value = render_value(key, value, Some(theme));
                    let display_text = rendered_value.plain_text().to_owned();
                    let mut spans = rendered_value.into_spans();
                    if is_status_like(key)
                        && let Some(color) = status_color_for_value(&display_text, theme)
                    {
                        spans = vec![Span::styled(display_text.clone(), Style::default().fg(color))];
                    }
                    let cell = Cell::from(Line::from(spans)).style(theme.text_primary_style());
                    cells.push(cell);
                }
                // Alternating row backgrounds using a theme helper.
                let row_style = table_row_style(theme, idx);
                rows.push(Row::new(cells).style(row_style));
            }
            return Some(rows);
        }
        None
    }

    fn create_columns(&self, value: Option<&[Value]>, rerank_columns: bool) -> Option<Vec<ColumnWithSize>> {
        if let Some(json) = value {
            return Some(infer_columns_with_sizes_from_json(json, 200, rerank_columns));
        }
        None
    }

    fn create_constraints(&self) -> Option<Vec<Constraint>> {
        if let Some(columns) = self.columns.as_ref() {
            let mut widths: Vec<Constraint> = Vec::new();
            if columns.is_empty() {
                widths.push(Constraint::Percentage(100));
            } else {
                // determine if we have any columns that exceed 60
                // and switch to using a fixed value for all others
                let large_cols: Vec<&ColumnWithSize> = columns.iter().filter(|c| c.max_len > 60).collect();
                let use_fixed = !large_cols.is_empty();
                for col in columns.iter() {
                    // Use measured max length with a small padding, with a sensible floor/ceiling
                    let w = (col.max_len + 2).clamp(4, 60) as u16;
                    if use_fixed && !large_cols.contains(&col) {
                        widths.push(Constraint::Length(w))
                    } else {
                        widths.push(Constraint::Min(w));
                    }
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

    fn create_kv_entries(&self, value: Option<&Value>) -> Vec<KeyValueEntry> {
        if let Some(json) = value {
            return build_key_value_entries(json);
        }
        Vec::new()
    }

    fn array_from_json(value: Option<&Value>) -> Option<&[Value]> {
        if let Some(json) = value {
            let arr_opt = match json {
                Value::Array(a) => Some(a.as_slice()),
                _ => None,
            };
            return arr_opt;
        }
        None
    }

    /// Processes general command execution results (non-plugin specific).
    ///
    /// This method handles the standard processing of command results, including
    /// logging, table updates, and pagination information.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    pub(crate) fn process_general_execution_result(&mut self, execution_outcome: &ExecOutcome, theme: &dyn Theme) {
        let maybe_value = match execution_outcome {
            ExecOutcome::Http {
                payload: value,
                request_id,
                ..
            } => {
                let mut cloned_value = value.clone();
                if let Some(array) = cloned_value.as_array_mut()
                    && self.pagination_state.should_reverse(*request_id)
                {
                    array.reverse();
                    serde_json::to_value(array).ok()
                } else {
                    Some(cloned_value)
                }
            }

            ExecOutcome::Mcp { payload: value, .. } => Some(value.clone()),
            _ => None,
        };

        if let Some(value) = maybe_value {
            let normalized_value = normalize_result_payload(value.clone());
            self.apply_result_json(Some(normalized_value), theme, true);
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyValueEntry {
    pub key: String,
    pub display_key: String,
    pub display_value: String,
    pub raw_value: Value,
}

pub fn build_key_value_entries(value: &Value) -> Vec<KeyValueEntry> {
    if let Value::Object(map) = value {
        let keys = get_scored_keys(map);
        return keys
            .into_iter()
            .take(24)
            .map(|key| {
                let raw_value = map.get(&key).cloned().unwrap_or(Value::Null);
                let display_value = render_value(&key, &raw_value, None).into_plain_text();
                KeyValueEntry {
                    key: key.clone(),
                    display_key: normalize_header(&key),
                    display_value,
                    raw_value,
                }
            })
            .collect();
    }

    Vec::new()
}

impl HasFocus for ResultsTableState<'_> {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // Single focusable grid area; treat as a leaf.
        builder.leaf_widget(&self.grid_f);
        builder.widget(&self.pagination_state);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
