use crate::ui::theme::Theme;
use crate::ui::utils::normalize_result_payload_owned;
use crate::ui::{
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
use std::borrow::Cow;

#[derive(Debug, Default)]
pub struct ResultsTableState<'a> {
    result_json: Option<Value>,
    // Vec of rows by column value
    row_cells: Option<Vec<Vec<Cell<'a>>>>,
    columns: Option<Vec<ColumnWithSize>>,
    column_constraints: Option<Vec<Constraint>>,
    headers: Option<Vec<Cell<'a>>>,
    kv_entries: Vec<KeyValueEntry>,
    truncated_col_idx: usize,
    // results and list states are supplied here
    // for use by the caller to render the results
    pub table_state: TableState,
    pub list_state: ListState,
    pub container_focus: FocusFlag,
    pub grid_f: FocusFlag,
    pub mouse_over_idx: Option<usize>,
}

impl<'a> ResultsTableState<'a> {
    pub fn selected_result_json(&self) -> Option<&Value> {
        self.result_json.as_ref()
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
        self.row_cells = self.create_row_cells(json_array, theme);
        self.headers = self.create_headers(theme);
        self.column_constraints = self.create_constraints();
        self.kv_entries = self.create_kv_entries(self.result_json.as_ref());
        self.table_state = TableState::default();
        self.truncated_col_idx = 0;
    }

    pub fn move_right(&mut self) {
        let max_col_idx = self.columns.as_ref().map(|cols| cols.len().saturating_sub(1)).unwrap_or(0);
        let proposed_col_idx = self.truncated_col_idx.saturating_add(1);
        if proposed_col_idx > max_col_idx {
            return;
        }
        self.truncated_col_idx = proposed_col_idx;
    }

    pub fn move_left(&mut self) {
        self.truncated_col_idx = self.truncated_col_idx.saturating_sub(1);
    }

    pub fn has_rows(&self) -> bool {
        self.row_cells.as_ref().map(|row_cells| !row_cells.is_empty()).unwrap_or(false)
    }

    pub fn num_rows(&self) -> usize {
        self.row_cells.as_ref().map(|row_cells| row_cells.len()).unwrap_or(0)
    }

    pub fn create_rows(&self, table_width: u16, theme: &dyn UiTheme) -> Option<(usize, Vec<Row<'a>>)> {
        let cols = self.columns.as_ref()?;
        let mut asked_width = cols.iter().fold(0, |acc, col| acc + col.max_len) as u16;
        let mut truncate_idx = 0;
        while asked_width > table_width && truncate_idx < self.truncated_col_idx {
            asked_width = asked_width.saturating_sub(cols[truncate_idx].max_len as u16);
            truncate_idx += 1;
        }

        let row_cells = self.row_cells.as_ref()?;
        let mut rows = Vec::with_capacity(row_cells.len());
        // iterate each row and trim columns after truncate_idx
        for (idx, cells) in row_cells.iter().enumerate() {
            let style = table_row_style(theme, idx);
            let truncated_rows = &cells[truncate_idx..];

            rows.push(Row::new(truncated_rows.to_vec()).style(style));
        }

        Some((truncate_idx, rows))
    }

    fn create_row_cells(&self, maybe_value: Option<&[Value]>, theme: &dyn UiTheme) -> Option<Vec<Vec<Cell<'a>>>> {
        if let Some(value) = maybe_value
            && self.columns.is_some()
        {
            let columns: &Vec<ColumnWithSize> = self.columns.as_ref().unwrap();
            let mut rows: Vec<Vec<Cell>> = Vec::with_capacity(value.len());
            for item in value.iter() {
                let mut cells: Vec<Cell> = Vec::with_capacity(value.len());
                for col in columns.iter() {
                    let key = &col.key;
                    let value = item.get(key).unwrap_or(&Value::Null);
                    let rendered_value = render_value(key, value, Some(theme));
                    let display_text = rendered_value.plain_text().to_owned();
                    let mut spans = rendered_value.into_spans();
                    if is_status_like(key)
                        && let Some(color) = status_color_for_value(&display_text, theme)
                    {
                        spans = vec![Span::styled(Cow::from(display_text), Style::default().fg(color))];
                    }
                    let cell = Cell::from(Line::from(spans)).style(theme.text_primary_style());
                    cells.push(cell);
                }
                rows.push(cells);
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
    /// logging, results updates, and pagination information.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    pub(crate) fn process_general_execution_result(&mut self, execution_outcome: ExecOutcome, theme: &dyn Theme) {
        let maybe_value = match execution_outcome {
            ExecOutcome::Http { payload: value, .. } => Some(value),

            ExecOutcome::Mcp { payload: value, .. } => Some(value.clone()),
            _ => None,
        };

        if let Some(value) = maybe_value {
            let normalized_value = normalize_result_payload_owned(value);
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
    match value {
        Value::Object(map) => get_scored_keys(map)
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
            .collect(),
        Value::Array(array) if !array.is_empty() => build_key_value_entries(array.first().unwrap()),
        _ => vec![],
    }
}

impl HasFocus for ResultsTableState<'_> {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // Single focusable grid area; treat as a leaf.
        builder.leaf_widget(&self.grid_f);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
