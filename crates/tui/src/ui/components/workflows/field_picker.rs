//! Field picker pane for selecting JSON fields from the workflow context.
//!
//! The pane powers the inline experience embedded in the Guided Input
//! Collector. It exposes navigation helpers, rendering hooks, and reusable
//! formatting utilities so other surfaces can present the same tree view when
//! needed.
#![allow(dead_code)]

use heroku_engine::WorkflowRunState;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::HashSet;

use crate::ui::components::workflows::format_preview;
use crate::ui::theme::theme_helpers::highlight_segments;
use crate::ui::theme::{roles::Theme, theme_helpers as th};

fn render_filter(frame: &mut Frame, area: Rect, theme: &dyn Theme, filter: &str, active: bool) {
    let title = Line::from(Span::styled(
        "Filter Fields",
        theme.text_secondary_style().add_modifier(Modifier::BOLD),
    ));
    let mut block = th::block(theme, None, active);
    block = block.title(title);

    let highlight = theme.search_highlight_style();
    let content_spans = if filter.is_empty() {
        vec![Span::styled("[type to filter]", theme.text_muted_style())]
    } else {
        highlight_segments(filter, filter, theme.syntax_keyword_style(), highlight)
    };

    let paragraph = Paragraph::new(Line::from(content_spans)).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_body(frame: &mut Frame, area: Rect, theme: &dyn Theme, tree: &PickerTree, filter: &str) {
    let segments = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_tree_list(frame, segments[0], theme, tree, filter);
    render_detail_panel(frame, segments[1], theme, tree.current_node());
}

fn render_tree_list(frame: &mut Frame, area: Rect, theme: &dyn Theme, tree: &PickerTree, filter: &str) {
    let block = th::block(theme, Some("Workflow Context"), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if tree.visible.is_empty() {
        let message = if filter.is_empty() {
            "No inputs or step outputs available."
        } else {
            "No fields match the current filter."
        };
        let paragraph = Paragraph::new(message).style(theme.text_muted_style()).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
        return;
    }

    let highlight = theme.search_highlight_style();
    let mut lines = Vec::with_capacity(tree.visible.len());
    for (row, index) in tree.visible.iter().enumerate() {
        let node = &tree.nodes[*index];
        let mut spans: Vec<Span<'static>> = Vec::new();

        let indent = "  ".repeat(node.depth);
        if !indent.is_empty() {
            spans.push(Span::raw(indent));
        }

        let marker = if node.has_children {
            if tree.is_expanded(&node.path) { "▾" } else { "▸" }
        } else {
            " "
        };
        spans.push(Span::styled(marker.to_string(), theme.syntax_keyword_style()));
        spans.push(Span::raw(" "));

        spans.extend(highlight_segments(filter, &node.label, theme.syntax_type_style(), highlight));

        if !node.has_children {
            let preview = format_preview(&node.value);
            if !preview.is_empty() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled("→ ", theme.syntax_keyword_style()));
                spans.extend(highlight_segments(
                    filter,
                    &preview,
                    value_preview_style(theme, &node.value),
                    highlight,
                ));
            }
        }

        let mut line = Line::from(spans);
        if row == tree.selected_index {
            line = line.style(theme.selection_style());
        }
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_detail_panel(frame: &mut Frame, area: Rect, theme: &dyn Theme, node: Option<&PickerNode>) {
    let block = th::block(theme, Some("Details"), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(node) = node {
        let preview = format_preview(&node.value);
        lines.push(Line::from(vec![
            Span::styled("Path: ", theme.text_secondary_style()),
            Span::styled(node.path.clone(), theme.syntax_type_style()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Type: ", theme.text_secondary_style()),
            Span::styled(value_type_label(&node.value), theme.syntax_keyword_style()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Value: ", theme.text_secondary_style()),
            Span::styled(preview, value_preview_style(theme, &node.value)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "Select a node to preview its value.",
            theme.text_muted_style(),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn render_footer(frame: &mut Frame, area: Rect, theme: &dyn Theme) {
    let line = Line::from(vec![
        Span::styled("[↑/↓] move  ", theme.text_secondary_style()),
        Span::styled("[←/→] collapse/expand  ", theme.text_secondary_style()),
        Span::styled("[Enter] select  ", theme.text_secondary_style()),
        Span::styled("[Esc] back  ", theme.text_secondary_style()),
        Span::styled("[/] filter  ", theme.text_secondary_style()),
        Span::styled("[f] close", theme.text_secondary_style()),
    ]);
    frame.render_widget(Paragraph::new(line).wrap(Wrap { trim: true }), area);
}

fn value_preview_style(theme: &dyn Theme, value: &JsonValue) -> Style {
    match value {
        JsonValue::String(_) => theme.syntax_string_style(),
        JsonValue::Number(_) => theme.syntax_number_style(),
        JsonValue::Bool(_) => theme.syntax_keyword_style(),
        JsonValue::Null => theme.text_muted_style(),
        JsonValue::Array(_) | JsonValue::Object(_) => theme.syntax_type_style(),
    }
}

#[derive(Debug, Default)]
struct PickerTree {
    nodes: Vec<PickerNode>,
    roots: Vec<usize>,
    visible: Vec<usize>,
    selected_index: usize,
    expanded_paths: HashSet<String>,
    selected_path: Option<String>,
    filter: Option<String>,
}

impl PickerTree {
    fn rebuild(&mut self, state: &WorkflowRunState) {
        let previous_selection = self.selected_path.clone();
        self.nodes.clear();
        self.roots.clear();

        if !state.run_context.inputs.is_empty() {
            self.expanded_paths.insert("inputs".to_string());
            let root_value = JsonValue::Object(state.run_context.inputs.clone());
            let root_index = self.add_node(None, "inputs".to_string(), "inputs".to_string(), root_value, 0);
            self.roots.push(root_index);
        }

        if !state.run_context.steps.is_empty() {
            self.expanded_paths.insert("steps".to_string());
            let mut map = JsonMap::new();
            for (key, value) in state.run_context.steps.iter() {
                map.insert(key.clone(), value.clone());
            }
            let root_index = self.add_node(None, "steps".to_string(), "steps".to_string(), JsonValue::Object(map), 0);
            self.roots.push(root_index);
        }

        self.selected_path = previous_selection;
        self.rebuild_visible_order();
    }

    fn clear(&mut self) {
        *self = PickerTree::default();
    }

    fn add_node(&mut self, parent: Option<usize>, label: String, path: String, value: JsonValue, depth: usize) -> usize {
        let has_children =
            matches!(&value, JsonValue::Object(map) if !map.is_empty()) || matches!(&value, JsonValue::Array(items) if !items.is_empty());

        let mut child_specs: Vec<(String, String, JsonValue)> = Vec::new();
        match &value {
            JsonValue::Object(map) => {
                for (key, child_value) in map.iter() {
                    let child_path = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                    child_specs.push((key.clone(), child_path, child_value.clone()));
                }
            }
            JsonValue::Array(items) => {
                for (idx, child_value) in items.iter().enumerate() {
                    let child_label = format!("[{idx}]");
                    let child_path = format!("{path}[{idx}]");
                    child_specs.push((child_label, child_path, child_value.clone()));
                }
            }
            _ => {}
        }

        let index = self.nodes.len();
        self.nodes.push(PickerNode {
            label,
            path: path.clone(),
            value,
            children: Vec::new(),
            parent,
            depth,
            has_children,
        });

        for (child_label, child_path, child_value) in child_specs {
            let child_index = self.add_node(Some(index), child_label, child_path, child_value, depth + 1);
            self.nodes[index].children.push(child_index);
        }

        index
    }

    fn rebuild_visible_order(&mut self) {
        self.visible.clear();
        let filter_owned = self.filter.clone();
        let filter = filter_owned.as_deref();
        let roots = self.roots.clone();
        for root in roots {
            self.push_visible(root, filter);
        }

        if self.visible.is_empty() {
            self.selected_index = 0;
            self.selected_path = None;
            return;
        }

        if let Some(path) = self.selected_path.clone() {
            if let Some((pos, _)) = self.visible.iter().enumerate().find(|(_, index)| self.nodes[**index].path == path) {
                self.selected_index = pos;
            } else {
                self.selected_index = self.selected_index.min(self.visible.len() - 1);
                self.selected_path = Some(self.nodes[self.visible[self.selected_index]].path.clone());
            }
        } else {
            self.selected_index = 0;
            self.selected_path = Some(self.nodes[self.visible[0]].path.clone());
        }
    }

    fn push_visible(&mut self, index: usize, filter: Option<&str>) {
        if !self.include_node(index, filter) {
            return;
        }

        self.visible.push(index);
        let always_expand = filter.is_some();
        if self.nodes[index].has_children && (always_expand || self.expanded_paths.contains(&self.nodes[index].path)) {
            let children = self.nodes[index].children.clone();
            for child in children {
                self.push_visible(child, filter);
            }
        }
    }

    fn current_node(&self) -> Option<&PickerNode> {
        self.visible.get(self.selected_index).and_then(|index| self.nodes.get(*index))
    }

    fn is_expanded(&self, path: &str) -> bool {
        self.expanded_paths.contains(path)
    }

    fn select_next(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.visible.len();
        self.update_selected_path();
    }

    fn select_prev(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.visible.len() - 1;
        } else {
            self.selected_index -= 1;
        }
        self.update_selected_path();
    }

    fn expand_selected(&mut self) {
        let Some((path, children)) = self.current_node().map(|node| (node.path.clone(), node.children.clone())) else {
            return;
        };

        if children.is_empty() {
            return;
        }

        if self.expanded_paths.insert(path.clone()) {
            self.selected_path = Some(path);
            self.rebuild_visible_order();
        } else if let Some(first_child) = children.first()
            && let Some(child_node) = self.nodes.get(*first_child)
        {
            self.selected_path = Some(child_node.path.clone());
            self.rebuild_visible_order();
        }
    }

    fn collapse_selected(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let index = self.visible[self.selected_index];
        let path = self.nodes[index].path.clone();

        if self.nodes[index].has_children && self.expanded_paths.remove(&path) {
            self.expanded_paths
                .retain(|other| !other.starts_with(&format!("{path}.")) && !other.starts_with(&format!("{path}[")));
            self.selected_path = Some(path);
            self.rebuild_visible_order();
        } else if let Some(parent_index) = self.nodes[index].parent {
            let parent_path = self.nodes[parent_index].path.clone();
            self.selected_path = Some(parent_path);
            self.rebuild_visible_order();
        }
    }

    fn update_selected_path(&mut self) {
        if let Some(index) = self.visible.get(self.selected_index) {
            self.selected_path = Some(self.nodes[*index].path.clone());
        }
    }

    fn set_filter(&mut self, filter: Option<String>) {
        self.filter = filter.map(|value| value.to_lowercase());
        self.rebuild_visible_order();
    }

    fn include_node(&self, index: usize, filter: Option<&str>) -> bool {
        match filter {
            None => true,
            Some(filter) => self.matches_filter(index, filter) || self.node_has_match(index, filter),
        }
    }

    fn matches_filter(&self, index: usize, filter: &str) -> bool {
        let node = &self.nodes[index];
        if node.label.to_lowercase().contains(filter) || node.path.to_lowercase().contains(filter) {
            return true;
        }
        let preview = format_preview(&node.value).to_lowercase();
        !preview.is_empty() && preview.contains(filter)
    }

    fn node_has_match(&self, index: usize, filter: &str) -> bool {
        for child in &self.nodes[index].children {
            if self.matches_filter(*child, filter) || self.node_has_match(*child, filter) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct FieldPickerPane {
    filter: String,
    tree: PickerTree,
}

impl FieldPickerPane {
    pub fn reset(&mut self) {
        self.filter.clear();
        self.tree.clear();
    }
    pub fn sync_from_run_state(&mut self, state: Option<&WorkflowRunState>) {
        if let Some(run_state) = state {
            self.tree.rebuild(run_state);
        } else {
            self.tree.clear();
        }
        let filter = if self.filter.is_empty() { None } else { Some(self.filter.clone()) };
        self.tree.set_filter(filter);
    }

    pub fn select_next(&mut self) {
        self.tree.select_next();
    }

    pub fn select_prev(&mut self) {
        self.tree.select_prev();
    }

    pub fn expand_selected(&mut self) {
        self.tree.expand_selected();
    }

    pub fn collapse_selected(&mut self) {
        self.tree.collapse_selected();
    }

    pub fn current_value(&self) -> Option<JsonValue> {
        self.tree.current_node().map(|node| node.value.clone())
    }

    pub fn clear_filter(&mut self) {
        if self.filter.is_empty() {
            return;
        }
        self.filter.clear();
        self.tree.set_filter(None);
    }

    pub fn push_filter_char(&mut self, character: char) {
        self.filter.push(character);
        self.tree.set_filter(Some(self.filter.clone()));
    }

    pub fn pop_filter_char(&mut self) {
        if self.filter.is_empty() {
            return;
        }
        self.filter.pop();
        let filter = if self.filter.is_empty() { None } else { Some(self.filter.clone()) };
        self.tree.set_filter(filter);
    }

    pub fn render_inline(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, filter_active: bool) {
        self.render_contents(frame, area, theme, filter_active);
    }

    fn render_contents(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, filter_active: bool) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        render_filter(frame, layout[0], theme, &self.filter, filter_active);
        render_body(frame, layout[1], theme, &self.tree, &self.filter);
        render_footer(frame, layout[2], theme);
    }
}

#[derive(Debug, Clone)]
struct PickerNode {
    label: String,
    path: String,
    value: JsonValue,
    children: Vec<usize>,
    parent: Option<usize>,
    depth: usize,
    has_children: bool,
}

fn value_type_label(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::String(_) => "string",
        JsonValue::Number(_) => "number",
        JsonValue::Bool(_) => "bool",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
        JsonValue::Null => "null",
    }
}
