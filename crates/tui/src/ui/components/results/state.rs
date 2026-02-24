use crate::ui::theme::Theme;
use crate::ui::utils::normalize_result_payload_owned;
use crate::ui::{
    components::common::ScrollMetrics,
    theme::{
        roles::Theme as UiTheme,
        theme_helpers::{table_header_style, table_row_style},
    },
    utils::{
        ColumnWithSize, KeyScoreContext, get_scored_keys, get_scored_keys_with_context, infer_columns_with_sizes_from_json, is_status_like,
        normalize_header, render_value, status_color_for_value,
    },
};
use oatty_types::ExecOutcome;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span};
use ratatui::widgets::{ListState, TableState};
use ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    widgets::{Cell, Row},
};
use serde_json::Value;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct DrillFrame {
    pub label: String,
    pub value: Value,
}

#[derive(Debug)]
pub struct ResultsTableState<'a> {
    result_json: Option<Value>,
    drill_stack: Vec<DrillFrame>,
    key_score_context: KeyScoreContext,
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
    split_preview_pinned: bool,
    split_preview_scroll_metrics: ScrollMetrics,
    selected_kv_value_overflows: bool,
}

impl<'a> Default for ResultsTableState<'a> {
    fn default() -> Self {
        Self {
            result_json: None,
            drill_stack: Vec::new(),
            key_score_context: KeyScoreContext::Browsing,
            row_cells: None,
            columns: None,
            column_constraints: None,
            headers: None,
            kv_entries: Vec::new(),
            truncated_col_idx: 0,
            table_state: TableState::default(),
            list_state: ListState::default(),
            container_focus: FocusFlag::default(),
            grid_f: FocusFlag::default(),
            mouse_over_idx: None,
            split_preview_pinned: false,
            split_preview_scroll_metrics: ScrollMetrics::default(),
            selected_kv_value_overflows: false,
        }
    }
}

impl<'a> ResultsTableState<'a> {
    pub fn is_in_drill_mode(&self) -> bool {
        !self.drill_stack.is_empty()
    }

    pub fn breadcrumbs(&self) -> Vec<String> {
        let mut breadcrumbs = vec!["Root".to_string()];
        breadcrumbs.extend(self.drill_stack.iter().map(|frame| frame.label.clone()));
        breadcrumbs
    }

    pub fn drill_up(&mut self, theme: &dyn UiTheme) -> bool {
        if self.drill_stack.pop().is_none() {
            return false;
        }
        self.reset_render_cache(theme);
        true
    }

    pub fn drill_to_breadcrumb(&mut self, breadcrumb_index: usize, theme: &dyn UiTheme) -> bool {
        if breadcrumb_index == 0 {
            if self.drill_stack.is_empty() {
                return false;
            }
            self.drill_stack.clear();
            self.reset_render_cache(theme);
            return true;
        }

        let stack_index = breadcrumb_index.saturating_sub(1);
        if stack_index >= self.drill_stack.len() {
            return false;
        }
        self.drill_stack.truncate(stack_index + 1);
        self.reset_render_cache(theme);
        true
    }

    pub fn drill_into_selection(&mut self, theme: &dyn UiTheme) -> bool {
        if let Some((label, value)) = self.selected_container_value_for_drill() {
            self.drill_stack.push(DrillFrame { label, value });
            self.reset_render_cache(theme);
            return true;
        }
        false
    }

    pub fn selected_result_json(&self) -> Option<&Value> {
        self.current_result_json()
    }
    pub fn column_constraints(&self) -> Option<&Vec<Constraint>> {
        self.column_constraints.as_ref()
    }
    pub fn headers(&self) -> Option<&Vec<Cell<'_>>> {
        self.headers.as_ref()
    }
    pub fn selected_data(&self, idx: usize) -> Option<&Value> {
        if let Some(json_array) = Self::array_from_json(self.current_result_json()) {
            return json_array.get(idx);
        }
        None
    }

    pub fn column_count(&self) -> usize {
        self.columns.as_ref().map_or(0, Vec::len)
    }

    pub fn selected_column_key(&self) -> Option<String> {
        let selected_column = self.table_state.selected_column()?;
        let columns = self.columns.as_ref()?;
        columns.get(selected_column).map(|column| column.key.clone())
    }

    pub fn ensure_column_selected(&mut self) {
        if self.table_state.selected_column().is_none() && self.column_count() > 0 {
            self.table_state.select_column(Some(0));
        }
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

    /// Sets whether the selected key/value row currently overflows the list row width.
    pub fn set_selected_kv_value_overflows(&mut self, selected_kv_value_overflows: bool) {
        self.selected_kv_value_overflows = selected_kv_value_overflows;
        if !self.should_show_split_preview() {
            self.split_preview_scroll_metrics.scroll_to_top();
        }
    }

    /// Returns whether the split preview should be visible.
    pub fn should_show_split_preview(&self) -> bool {
        self.split_preview_pinned || self.selected_kv_value_overflows
    }

    /// Toggles whether the split preview remains visible even when content fits.
    pub fn toggle_split_preview_pinned(&mut self) {
        self.split_preview_pinned = !self.split_preview_pinned;
        if !self.should_show_split_preview() {
            self.split_preview_scroll_metrics.scroll_to_top();
        }
    }

    /// Updates split preview content and viewport metrics.
    pub fn update_split_preview_metrics(&mut self, content_height: u16, viewport_height: u16) {
        self.split_preview_scroll_metrics.update_content_height(content_height);
        self.split_preview_scroll_metrics.update_viewport_height(viewport_height);
    }

    /// Returns current split preview vertical scroll offset.
    pub const fn split_preview_scroll_offset(&self) -> u16 {
        self.split_preview_scroll_metrics.offset()
    }

    /// Scrolls split preview by line count.
    pub fn scroll_split_preview_lines(&mut self, delta: i16) {
        self.split_preview_scroll_metrics.scroll_lines(delta);
    }

    /// Scrolls split preview by viewport pages.
    pub fn scroll_split_preview_pages(&mut self, delta: i16) {
        self.split_preview_scroll_metrics.scroll_pages(delta);
    }

    /// Moves split preview scroll position to the first line.
    pub fn scroll_split_preview_to_top(&mut self) {
        self.split_preview_scroll_metrics.scroll_to_top();
    }

    /// Moves split preview scroll position to the last visible page.
    pub fn scroll_split_preview_to_bottom(&mut self) {
        self.split_preview_scroll_metrics.scroll_to_bottom();
    }

    pub fn apply_result_json(&mut self, value: Option<Value>, theme: &dyn UiTheme, rerank_columns: bool) {
        self.result_json = value;
        self.drill_stack.clear();
        let json_array = Self::array_from_json(self.current_result_json());
        self.columns = self.create_columns(json_array, rerank_columns);
        self.reset_render_cache(theme);
    }

    /// Sets the key-scoring context used by key/value rendering and refreshes the
    /// render cache so drill transitions retain the expected ordering.
    pub fn set_key_score_context(&mut self, key_score_context: KeyScoreContext, theme: &dyn UiTheme) {
        if self.key_score_context == key_score_context {
            return;
        }
        self.key_score_context = key_score_context;
        self.reset_render_cache(theme);
    }

    /// Reorders visible columns so prioritized keys appear first.
    ///
    /// This is used by provider-backed selectors where the chosen `value_field`
    /// or `display_field` can otherwise be omitted by generic column ranking.
    pub fn prioritize_columns(&mut self, prioritized_keys: &[String], theme: &dyn UiTheme) {
        let normalized_keys = Self::normalize_prioritized_keys(prioritized_keys);
        if normalized_keys.is_empty() {
            return;
        }
        let selected_column_key = self.selected_column_key();
        let Some(rows) = Self::array_from_json(self.current_result_json()) else {
            return;
        };
        let row_snapshot = rows.to_vec();
        let Some(current_columns) = self.columns.take() else {
            return;
        };

        let mut ordered_columns = Vec::with_capacity(current_columns.len() + normalized_keys.len());
        let mut remaining_columns = current_columns;

        for key in normalized_keys {
            if let Some(existing_index) = remaining_columns.iter().position(|column| column.key == key) {
                ordered_columns.push(remaining_columns.remove(existing_index));
                continue;
            }
            if let Some(derived_column) = Self::derive_column_for_key(&row_snapshot, &key) {
                ordered_columns.push(derived_column);
            }
        }

        if ordered_columns.is_empty() {
            self.columns = Some(remaining_columns);
            return;
        }

        ordered_columns.extend(remaining_columns);
        self.columns = Some(ordered_columns);
        self.reset_render_cache(theme);
        if let Some(selected_key) = selected_column_key
            && let Some(index) = self
                .columns
                .as_ref()
                .and_then(|columns| columns.iter().position(|column| column.key == selected_key))
        {
            self.table_state.select_column(Some(index));
        }
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

    pub fn move_selected_column_right(&mut self, table_width: u16) {
        if self.column_count() == 0 {
            return;
        }
        let max_column_index = self.column_count().saturating_sub(1);
        let selected_column = self.table_state.selected_column().unwrap_or(0);
        let next_column = selected_column.saturating_add(1).min(max_column_index);
        self.table_state.select_column(Some(next_column));
        self.ensure_selected_column_visible(table_width);
    }

    pub fn move_selected_column_left(&mut self, table_width: u16) {
        if self.column_count() == 0 {
            return;
        }
        let selected_column = self.table_state.selected_column().unwrap_or(0);
        let next_column = selected_column.saturating_sub(1);
        self.table_state.select_column(Some(next_column));
        self.ensure_selected_column_visible(table_width);
    }

    pub fn ensure_selected_column_visible(&mut self, table_width: u16) {
        if self.column_count() == 0 {
            self.truncated_col_idx = 0;
            return;
        }
        let selected_column = self.table_state.selected_column().unwrap_or(0);
        let max_column_index = self.column_count().saturating_sub(1);
        let selected_column = selected_column.min(max_column_index);
        self.table_state.select_column(Some(selected_column));

        if selected_column < self.truncated_col_idx {
            self.truncated_col_idx = selected_column;
        }

        loop {
            let visible_window = self.visible_column_window(table_width);
            let Some((window_start, window_end)) = visible_window else {
                self.truncated_col_idx = 0;
                break;
            };
            if selected_column < window_start {
                self.truncated_col_idx = selected_column;
                continue;
            }
            if selected_column >= window_end {
                self.truncated_col_idx = self.truncated_col_idx.saturating_add(1).min(max_column_index);
                continue;
            }
            break;
        }
    }

    pub fn hit_test_column(&self, relative_x: u16, table_width: u16) -> Option<usize> {
        let (window_start, window_end) = self.visible_column_window(table_width)?;
        let visible_columns = window_end.saturating_sub(window_start);
        if visible_columns == 0 {
            return None;
        }
        let widths = self.column_constraints.as_ref()?;
        let visible_constraints = widths.get(window_start..window_end)?;
        let layout_area = Rect {
            x: 0,
            y: 0,
            width: table_width,
            height: 1,
        };
        let column_areas = Layout::horizontal(visible_constraints.to_vec()).split(layout_area);
        column_areas
            .iter()
            .enumerate()
            .find(|(_, column_area)| relative_x >= column_area.x && relative_x < column_area.x.saturating_add(column_area.width))
            .map(|(visible_index, _)| window_start + visible_index)
    }

    pub fn select_cell(&mut self, row: usize, column: usize, table_width: u16) {
        self.table_state.select(Some(row));
        self.table_state.select_column(Some(column));
        self.ensure_selected_column_visible(table_width);
    }

    pub fn create_rows(&self, table_width: u16, theme: &dyn UiTheme) -> Option<(usize, Vec<Row<'a>>)> {
        let cols = self.columns.as_ref()?;
        let truncate_idx = self
            .visible_column_window(table_width)
            .map(|(window_start, _)| window_start)
            .unwrap_or_else(|| self.truncated_col_idx.min(cols.len().saturating_sub(1)));

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
                    let mut display_text = rendered_value.plain_text().to_owned();
                    let mut spans = rendered_value.into_spans();
                    if let Some(prefix) = container_preview_prefix(value) {
                        display_text = format!("{prefix} {display_text}");
                        let mut prefixed_spans = vec![Span::styled(format!("{prefix} "), theme.syntax_type_style())];
                        prefixed_spans.extend(spans);
                        spans = prefixed_spans;
                    }
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

    fn visible_column_window(&self, table_width: u16) -> Option<(usize, usize)> {
        let columns = self.columns.as_ref()?;
        if columns.is_empty() {
            return Some((0, 0));
        }

        let max_column_index = columns.len().saturating_sub(1);
        let window_start = self.truncated_col_idx.min(max_column_index);
        let constraints = self.column_constraints.as_ref()?;
        let visible_constraints = constraints.get(window_start..)?;
        if visible_constraints.is_empty() {
            return Some((window_start, window_start));
        }

        let layout_area = Rect {
            x: 0,
            y: 0,
            width: table_width,
            height: 1,
        };
        let column_areas = Layout::horizontal(visible_constraints.to_vec()).split(layout_area);
        let mut visible_count = 0usize;
        for column_area in column_areas.iter() {
            if column_area.width == 0 {
                break;
            }
            visible_count = visible_count.saturating_add(1);
        }
        if visible_count == 0 {
            visible_count = 1;
        }
        let window_end = (window_start + visible_count).min(columns.len());
        Some((window_start, window_end))
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
            return build_key_value_entries_with_context(json, self.key_score_context);
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

    fn reset_render_cache(&mut self, theme: &dyn UiTheme) {
        let json_array = Self::array_from_json(self.current_result_json());
        self.row_cells = self.create_row_cells(json_array, theme);
        self.headers = self.create_headers(theme);
        self.column_constraints = self.create_constraints();
        self.kv_entries = self.create_kv_entries(self.current_result_json());
        self.table_state = TableState::default();
        self.list_state = ListState::default();
        self.truncated_col_idx = 0;
        self.split_preview_scroll_metrics.reset();
        self.selected_kv_value_overflows = false;
        self.split_preview_pinned = false;
        if !self.kv_entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn normalize_prioritized_keys(prioritized_keys: &[String]) -> Vec<String> {
        let mut unique = Vec::new();
        for key in prioritized_keys {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                continue;
            }
            if unique.iter().any(|candidate: &String| candidate == trimmed) {
                continue;
            }
            unique.push(trimmed.to_string());
        }
        unique
    }

    fn derive_column_for_key(rows: &[Value], key: &str) -> Option<ColumnWithSize> {
        let header = normalize_header(key);
        let mut max_len = header.len();
        let mut found = false;

        for row in rows.iter().take(200) {
            let Value::Object(map) = row else {
                continue;
            };
            let Some(raw_value) = map.get(key) else {
                continue;
            };
            found = true;
            let mut rendered = render_value(key, raw_value, None).into_plain_text();
            if let Some(prefix) = container_preview_prefix(raw_value) {
                rendered = format!("{prefix} {rendered}");
            }
            max_len = max_len.max(rendered.len());
        }

        if !found {
            return None;
        }

        Some(ColumnWithSize {
            name: header,
            key: key.to_string(),
            max_len,
        })
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

    fn current_result_json(&self) -> Option<&Value> {
        if let Some(frame) = self.drill_stack.last() {
            return Some(&frame.value);
        }
        self.result_json.as_ref()
    }

    fn selected_container_value_for_drill(&self) -> Option<(String, Value)> {
        if self.has_rows() {
            return self.selected_table_container_for_drill();
        }
        self.selected_kv_container_for_drill()
    }

    fn selected_table_container_for_drill(&self) -> Option<(String, Value)> {
        let row_index = self.table_state.selected()?;
        let row = self.selected_data(row_index)?;
        let Value::Object(map) = row else {
            return None;
        };

        if let Some(column_key) = self.selected_column_key()
            && let Some(value) = map.get(column_key.as_str())
            && matches!(value, Value::Array(_) | Value::Object(_))
        {
            return Some((column_key, value.clone()));
        }

        let key = get_scored_keys(map).into_iter().find(|candidate| {
            map.get(candidate)
                .is_some_and(|value| matches!(value, Value::Array(_) | Value::Object(_)))
        })?;
        let value = map.get(key.as_str())?.clone();
        Some((key, value))
    }

    fn selected_kv_container_for_drill(&self) -> Option<(String, Value)> {
        let entry_index = self.list_state.selected()?;
        let entry = self.selected_kv_entry(entry_index)?;
        if !matches!(entry.raw_value, Value::Array(_) | Value::Object(_)) {
            return None;
        }
        Some((entry.key.clone(), entry.raw_value.clone()))
    }
}

fn container_preview_prefix(value: &Value) -> Option<&'static str> {
    match value {
        Value::Object(_) => Some("{}"),
        Value::Array(_) => Some("[]"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct KeyValueEntry {
    pub key: String,
    pub display_key: String,
    pub raw_value: Value,
}

pub fn build_key_value_entries(value: &Value) -> Vec<KeyValueEntry> {
    build_key_value_entries_with_context(value, KeyScoreContext::Browsing)
}

/// Builds key/value rows using context-aware key ranking.
pub fn build_key_value_entries_with_context(value: &Value, context: KeyScoreContext) -> Vec<KeyValueEntry> {
    match value {
        Value::Object(map) => get_scored_keys_with_context(map, context)
            .into_iter()
            .take(24)
            .map(|key| {
                let raw_value = map.get(&key).cloned().unwrap_or(Value::Null);
                KeyValueEntry {
                    key: key.clone(),
                    display_key: normalize_header(&key),
                    raw_value,
                }
            })
            .collect(),
        Value::Array(array) if !array.is_empty() => build_key_value_entries_with_context(array.first().unwrap(), context),
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

#[cfg(test)]
mod tests {
    use super::ResultsTableState;
    use crate::ui::theme::dracula::DraculaTheme;
    use serde_json::json;

    #[test]
    fn prioritize_columns_promotes_requested_keys() {
        let mut table = ResultsTableState::default();
        let theme = DraculaTheme::new();
        table.apply_result_json(
            Some(json!([
                {"name": "alpha", "id": "app-1", "status": "ok"},
                {"name": "beta", "id": "app-2", "status": "ok"}
            ])),
            &theme,
            true,
        );

        let before = table.columns.as_ref().expect("columns");
        assert_ne!(before.first().map(|column| column.key.as_str()), Some("id"));

        table.prioritize_columns(&["id".to_string()], &theme);

        let after = table.columns.as_ref().expect("columns");
        assert_eq!(after.first().map(|column| column.key.as_str()), Some("id"));
    }

    #[test]
    fn drill_into_selection_and_up_round_trip() {
        let mut table = ResultsTableState::default();
        let theme = DraculaTheme::new();
        table.apply_result_json(
            Some(json!([
                {"service": {"id": "srv-1", "name": "api"}, "status": "ok"}
            ])),
            &theme,
            true,
        );
        table.table_state.select(Some(0));
        table.table_state.select_column(Some(0));
        assert!(table.drill_into_selection(&theme));
        assert!(table.is_in_drill_mode());
        assert!(table.drill_up(&theme));
        assert!(!table.is_in_drill_mode());
    }

    #[test]
    fn breadcrumbs_include_root_and_nested_labels() {
        let mut table = ResultsTableState::default();
        let theme = DraculaTheme::new();
        table.apply_result_json(Some(json!([{"service": {"id": "srv-1"}}])), &theme, true);
        table.table_state.select(Some(0));
        table.table_state.select_column(Some(0));
        assert!(table.drill_into_selection(&theme));
        assert_eq!(table.breadcrumbs(), vec!["Root".to_string(), "service".to_string()]);
    }

    #[test]
    fn value_selection_entries_prioritize_identifier_fields() {
        use super::build_key_value_entries_with_context;
        use crate::ui::utils::KeyScoreContext;

        let value = json!({
            "name": "api-service",
            "id": "srv-1234"
        });

        let entries = build_key_value_entries_with_context(&value, KeyScoreContext::ValueSelection);
        assert_eq!(entries.first().map(|entry| entry.key.as_str()), Some("id"));
    }

    #[test]
    fn value_selection_context_persists_during_drill_navigation() {
        use crate::ui::utils::KeyScoreContext;

        let mut table = ResultsTableState::default();
        let theme = DraculaTheme::new();
        table.set_key_score_context(KeyScoreContext::ValueSelection, &theme);
        table.apply_result_json(
            Some(json!({
                "item": {
                    "name": "service-a",
                    "id": "srv-1234"
                }
            })),
            &theme,
            false,
        );

        assert_eq!(table.kv_entries().first().map(|entry| entry.key.as_str()), Some("item"));
        table.list_state.select(Some(0));
        assert!(table.drill_into_selection(&theme));
        assert_eq!(table.kv_entries().first().map(|entry| entry.key.as_str()), Some("id"));
    }
}
