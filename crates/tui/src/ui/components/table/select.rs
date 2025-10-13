//! Selectable table rendering helpers tailored for provider-backed pickers.
//!
//! Workflows rely on table-centric layouts when presenting provider results
//! (see `specs/WORKFLOW_TUI.md` §§2.3–2.4).  This module exposes small helpers
//! that extend the baseline table rendering with selection markers, TTL badges,
//! and chip-style summaries so higher-level components (collector, inputs view)
//! do not need to duplicate layout code.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Cell, Row, Table, TableState},
};

use crate::ui::theme::{
    roles::Theme as UiTheme,
    theme_helpers::{badge_style, block, panel_style, table_header_row_style, table_row_style},
};

/// Defines how selection indicators should be rendered for a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Only one row can be selected at a time; the highlight arrow is used.
    Single,
    /// Multiple rows can be selected; checkbox markers are rendered.
    Multiple,
}

impl SelectionMode {
    fn marker_for(self, is_selected: bool, is_highlighted: bool) -> &'static str {
        match self {
            SelectionMode::Single => {
                if is_highlighted {
                    "▸"
                } else {
                    " "
                }
            }
            SelectionMode::Multiple => {
                if is_selected {
                    "☑"
                } else {
                    "☐"
                }
            }
        }
    }
}

/// Describes a single row that should be rendered inside the selectable table.
#[derive(Debug, Clone)]
pub struct SelectableTableRow {
    /// Primary cell values for the row (not including the leading marker column).
    pub value_cells: Vec<String>,
    /// Optional badges rendered in the trailing column (`[tag]` style).
    pub metadata_badges: Vec<String>,
    /// Indicates whether the row is currently selected (relevant for multi-select mode).
    pub is_selected: bool,
}

impl SelectableTableRow {
    /// Creates a new row with optional metadata badges.
    pub fn new(value_cells: Vec<String>, metadata_badges: Vec<String>, is_selected: bool) -> Self {
        Self {
            value_cells,
            metadata_badges,
            is_selected,
        }
    }

    fn metadata_display(&self) -> String {
        if self.metadata_badges.is_empty() {
            return String::new();
        }

        self.metadata_badges
            .iter()
            .map(|badge| format!("[{badge}]"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Configuration describing how to render a selectable table.
#[derive(Debug, Clone)]
pub struct SelectableTableConfig {
    /// Title rendered in the surrounding block.
    pub title: String,
    /// Column headers excluding the leading marker column.
    pub column_titles: Vec<String>,
    /// Collection of rows to display.
    pub rows: Vec<SelectableTableRow>,
    /// Index of the row that is currently highlighted (if any).
    pub highlight_index: Option<usize>,
    /// Selection behaviour for the table.
    pub selection_mode: SelectionMode,
    /// Chip-style summary of already-selected values.
    pub selected_labels: Vec<String>,
    /// Optional status badge (for example, cache age or TTL).
    pub status_badge: Option<String>,
    /// Title for the metadata column; defaults to "Details" when `None`.
    pub metadata_title: Option<String>,
    /// Controls border emphasis to reflect focus.
    pub focused: bool,
}

impl SelectableTableConfig {
    /// Creates a configuration with sensible defaults.
    pub fn new(title: impl Into<String>, column_titles: Vec<String>, selection_mode: SelectionMode) -> Self {
        Self {
            title: title.into(),
            column_titles,
            rows: Vec::new(),
            highlight_index: None,
            selection_mode,
            selected_labels: Vec::new(),
            status_badge: None,
            metadata_title: None,
            focused: false,
        }
    }

    fn requires_metadata_column(&self) -> bool {
        if self.metadata_title.is_some() {
            return true;
        }
        self.rows.iter().any(|row| !row.metadata_badges.is_empty())
    }
}

/// Renders a selectable table using Ratatui primitives and theme helpers.
pub fn render_selectable_table(frame: &mut Frame, area: Rect, config: &SelectableTableConfig, theme: &dyn UiTheme) {
    let border = block(theme, Some(&config.title), config.focused);
    let inner_area = border.inner(area);
    frame.render_widget(border, area);

    let mut sections: Vec<Constraint> = Vec::new();
    if config.status_badge.is_some() {
        sections.push(Constraint::Length(1));
    }
    if !config.selected_labels.is_empty() {
        sections.push(Constraint::Length(1));
    }
    sections.push(Constraint::Min(1));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(sections)
        .split(inner_area);

    let mut next_index = 0;
    if let Some(status) = &config.status_badge {
        render_status_line(frame, layout[next_index], status, theme);
        next_index += 1;
    }

    if !config.selected_labels.is_empty() {
        render_selected_chips(frame, layout[next_index], &config.selected_labels, theme);
        next_index += 1;
    }

    render_table_rows(frame, layout[next_index], config, theme);
}

fn render_status_line(frame: &mut Frame, area: Rect, status: &str, theme: &dyn UiTheme) {
    let line = Line::from(vec![
        Span::styled("Status: ", theme.text_secondary_style()),
        Span::styled(status, theme.text_primary_style().add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(
        ratatui::widgets::Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}

fn render_selected_chips(frame: &mut Frame, area: Rect, labels: &[String], theme: &dyn UiTheme) {
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled("Selected: ", theme.text_secondary_style()));

    if labels.is_empty() {
        spans.push(Span::styled("<none>", theme.text_muted_style()));
    } else {
        for (index, label) in labels.iter().enumerate() {
            spans.push(Span::styled(format!("[{}]", label), badge_style(theme)));
            if index < labels.len() - 1 {
                spans.push(Span::raw(" "));
            }
        }
    }

    frame.render_widget(
        ratatui::widgets::Paragraph::new(Line::from(spans)).wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}

fn render_table_rows(frame: &mut Frame, area: Rect, config: &SelectableTableConfig, theme: &dyn UiTheme) {
    if config.rows.is_empty() {
        frame.render_widget(
            ratatui::widgets::Paragraph::new("No candidates available.")
                .style(theme.text_muted_style())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            area,
        );
        return;
    }

    let metadata_column = config.requires_metadata_column();
    let mut header_cells: Vec<Cell> = Vec::new();
    header_cells.push(Cell::from(String::new()).style(theme.text_secondary_style()));
    for title in &config.column_titles {
        header_cells.push(Cell::from(title.clone()).style(table_header_row_style(theme)));
    }
    if metadata_column {
        let title = config.metadata_title.clone().unwrap_or_else(|| "Details".to_string());
        header_cells.push(Cell::from(title).style(table_header_row_style(theme)));
    }

    let mut rows: Vec<Row> = Vec::new();
    for (index, row) in config.rows.iter().enumerate() {
        let highlight = config.highlight_index.map(|cursor| cursor == index).unwrap_or(false);
        let leading = config.selection_mode.marker_for(row.is_selected, highlight).to_string();
        let mut cells: Vec<Cell> = Vec::with_capacity(header_cells.len());
        cells.push(Cell::from(leading).style(theme.text_secondary_style()));
        for value in &row.value_cells {
            cells.push(Cell::from(value.clone()).style(theme.text_primary_style()));
        }
        if metadata_column {
            cells.push(Cell::from(row.metadata_display()).style(theme.text_secondary_style()));
        }
        rows.push(Row::new(cells).style(table_row_style(theme, index)));
    }

    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(2));

    let total_columns = header_cells.len();
    let remaining_columns = total_columns.saturating_sub(1);
    if remaining_columns == 0 {
        constraints[0] = Constraint::Percentage(100);
    } else {
        let base_percentage = 98 / remaining_columns as u16;
        for _ in 0..remaining_columns {
            constraints.push(Constraint::Percentage(base_percentage));
        }
    }

    let mut table_state = TableState::default();
    table_state.select(config.highlight_index);

    let table = Table::new(rows, constraints)
        .header(Row::new(header_cells))
        .column_spacing(2)
        .row_highlight_style(theme.selection_style())
        .block(
            ratatui::widgets::Block::default()
                .style(panel_style(theme))
                .borders(ratatui::widgets::Borders::NONE),
        );

    frame.render_stateful_widget(table, area, &mut table_state);
}
