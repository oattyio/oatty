//! Add Plugin panel component.
//! 
//! This component renders a small add_view_state that allows users to register
//! a new MCP plugin, either via a Local command or a Remote Base URL.
//! It manages focus, keyboard handling, validation/apply actions, and
//! displays validation/preview messages.
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Alignment,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::Effect};
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::state::{AddTransport, PluginAddViewState};

// Focus indices for fields and buttons. Keep in sync with render order.
const TRANSPORT: usize = 0;
const NAME: usize = 1;
const COMMAND: usize = 2;
const ARGS: usize = 3;
const BASE_URL: usize = 4;
const BTN_VALIDATE: usize = 5;
const BTN_SAVE: usize = 6;
const BTN_CANCEL: usize = 7;
const MAX: usize = BTN_CANCEL;

/// Component responsible for the Add Plugin panel.
/// 
/// The panel implements:
/// - Transport toggle (Local/Remote)
/// - Text inputs depending on transport
/// - Button row (Validate, Save, Cancel)
/// - Validation/preview messages area
/// 
/// Focus order is defined by the `*` constants and some indices are
/// conditionally hidden depending on the selected `AddTransport`.
#[derive(Debug, Default)]
pub struct PluginsAddComponent;

impl Component for PluginsAddComponent {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key: crossterm::event::KeyEvent) -> Vec<Effect> {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mut add_clone = app.plugins.add.clone();
        let Some(add_view_state) = add_clone.as_mut() else {
            return Vec::new();
        };

        let (validate_enabled, save_enabled, cancel_enabled) =
            Self::compute_button_enablement(&add_view_state);

        match key.code {
            KeyCode::Esc => {
                app.plugins.add = None;
                app.mark_dirty();
            }
            // Radio-group navigation on the Transport row
            KeyCode::Left if add_view_state.selected == TRANSPORT => {
                add_view_state.transport = AddTransport::Local;
                app.mark_dirty();
            }
            KeyCode::Right if add_view_state.selected == TRANSPORT => {
                add_view_state.transport = AddTransport::Remote;
                app.mark_dirty();
            }
            KeyCode::Char(' ') if add_view_state.selected == TRANSPORT => {
                add_view_state.transport = match add_view_state.transport {
                    AddTransport::Local => AddTransport::Remote,
                    AddTransport::Remote => AddTransport::Local,
                };
                app.mark_dirty();
            }
            KeyCode::Enter => {
                return Self::handle_enter(app, add_view_state, validate_enabled, save_enabled, cancel_enabled);
            }
            KeyCode::Tab => {
                Self::handle_tab_forward(add_view_state, validate_enabled, save_enabled, cancel_enabled);
                app.mark_dirty();
            }
            KeyCode::BackTab => {
                Self::handle_tab_backward(add_view_state, validate_enabled, save_enabled, cancel_enabled);
                app.mark_dirty();
            }
            KeyCode::Left if add_view_state.selected >= BTN_VALIDATE => {
                Self::handle_button_left(add_view_state, validate_enabled, save_enabled, cancel_enabled);
                app.mark_dirty();
            }
            KeyCode::Right if add_view_state.selected >= BTN_VALIDATE => {
                Self::handle_button_right(add_view_state, validate_enabled, save_enabled, cancel_enabled);
                app.mark_dirty();
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsValidateAdd];
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsApplyAdd];
            }
            KeyCode::Backspace => {
                Self::handle_backspace(add_view_state);
                app.mark_dirty();
            }
            KeyCode::Char(c) => {
                Self::handle_char(add_view_state, c);
                app.mark_dirty();
            }
            _ => (),
        }
        app.plugins.add = add_clone;
        return vec![]
    }

    fn update(&mut self, _app: &mut crate::app::App, _msg: &crate::app::Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        if let Some(add_view_state) = &app.plugins.add {
            let theme = &*app.ctx.theme;
            self.render_add_add_view_state(frame, area, theme, add_view_state);
        }
    }
}

impl PluginsAddComponent {
    /// Skip to next visible field when tabbing forward, given transport.
    fn skip_hidden_fields_forward(idx: usize, transport: AddTransport) -> usize {
        match transport {
            AddTransport::Local => match idx {
                BASE_URL => BTN_VALIDATE, // skip remote-only field
                _ => idx,
            },
            AddTransport::Remote => match idx {
                COMMAND | ARGS => BASE_URL, // jump to Base URL
                _ => idx,
            },
        }
    }

    /// Skip to previous visible field when tabbing backward, given transport.
    fn skip_hidden_fields_backward(idx: usize, transport: AddTransport) -> usize {
        match transport {
            AddTransport::Local => match idx {
                BASE_URL => ARGS, // go to last local field
                _ => idx,
            },
            AddTransport::Remote => match idx {
                ARGS => NAME,    // jump back to Name
                COMMAND => NAME, // jump back to Name
                _ => idx,
            },
        }
    }
    /// Render the entire Add Plugin add_view_state.
    fn render_add_add_view_state(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, add_view_state: &PluginAddViewState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .title(Span::styled(
                "Add Plugin — [Ctrl+V] validate  [Ctrl+A] apply  [Esc] cancel",
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(block.clone(), area);
        let inner = block.inner(area);

        self.render_transport_switch(frame, inner, theme, add_view_state);

        let selected_index = add_view_state.selected;
        let fields_area = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: inner.height.saturating_sub(1) };
        self.render_fields(frame, fields_area, theme, add_view_state, selected_index);

        // Render buttons pinned to the lower-right (consistent with pagination)
        let button_row_height: u16 = 3;
        let buttons_area = Rect {
            x: inner.x,
            y: inner.y.saturating_add(inner.height.saturating_sub(button_row_height)),
            width: inner.width,
            height: button_row_height,
        };
        self.render_buttons(frame, buttons_area, theme, add_view_state);

        self.render_messages(frame, inner, buttons_area, theme, add_view_state);

        self.position_cursor(frame, fields_area, add_view_state, selected_index);
    }
}

impl PluginsAddComponent {
    /// Render a small bordered buttonlike widget.
    fn render_button(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        enabled: bool,
        focused: bool,
        theme: &dyn Theme,
    ) {
        let button_style = if enabled {
            th::button_secondary_style(theme, true)
        } else {
            theme.text_muted_style().add_modifier(Modifier::DIM)
        };
        let border_style = if enabled {
            theme.border_style(focused)
        } else {
            theme.text_muted_style()
        };
        let button = Paragraph::new(label)
            .block(Block::default().borders(Borders::ALL).border_style(border_style))
            .style(button_style)
            .alignment(Alignment::Center);
        frame.render_widget(button, area);
    }

    /// Compute whether Validate/Save/Cancel buttons are enabled for the current input.
    fn compute_button_enablement(add_view_state: &PluginAddViewState) -> (bool, bool, bool) {
        let name_present = !add_view_state.name.trim().is_empty();
        match add_view_state.transport {
            AddTransport::Local => {
                let command_present = !add_view_state.command.trim().is_empty();
                (command_present, name_present && command_present, true)
            }
            AddTransport::Remote => {
                let base_url_present = !add_view_state.base_url.trim().is_empty();
                (base_url_present, name_present && base_url_present, true)
            }
        }
    }

    /// Pick the next enabled button index moving rightward (wrapping within buttons).
    fn ensure_enabled_button_forward(idx: usize, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) -> usize {
        if idx < BTN_VALIDATE { return idx; }
        for i in idx..=MAX {
            match i {
                BTN_VALIDATE if validate_enabled => return BTN_VALIDATE,
                BTN_SAVE if save_enabled => return BTN_SAVE,
                BTN_CANCEL if cancel_enabled => return BTN_CANCEL,
                _ => {}
            }
        }
        for i in BTN_VALIDATE..=MAX {
            match i {
                BTN_VALIDATE if validate_enabled => return BTN_VALIDATE,
                BTN_SAVE if save_enabled => return BTN_SAVE,
                BTN_CANCEL if cancel_enabled => return BTN_CANCEL,
                _ => {}
            }
        }
        BTN_CANCEL
    }

    /// Pick the previous enabled button index moving leftward.
    fn ensure_enabled_button_backward(idx: usize, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) -> usize {
        if idx < BTN_VALIDATE { return idx; }
        let start = if idx > MAX { MAX } else { idx };
        for i in (BTN_VALIDATE..=start).rev() {
            match i {
                BTN_CANCEL if cancel_enabled => return BTN_CANCEL,
                BTN_SAVE if save_enabled => return BTN_SAVE,
                BTN_VALIDATE if validate_enabled => return BTN_VALIDATE,
                _ => {}
            }
        }
        for i in (BTN_VALIDATE..=MAX).rev() {
            match i {
                BTN_CANCEL if cancel_enabled => return BTN_CANCEL,
                BTN_SAVE if save_enabled => return BTN_SAVE,
                BTN_VALIDATE if validate_enabled => return BTN_VALIDATE,
                _ => {}
            }
        }
        BTN_CANCEL
    }

    /// Advance focus on Tab.
    fn handle_tab_forward(add_view_state: &mut PluginAddViewState, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) {
        add_view_state.selected = (add_view_state.selected + 1) % (MAX + 1);
        add_view_state.selected = Self::skip_hidden_fields_forward(add_view_state.selected, add_view_state.transport);
        if add_view_state.selected >= BTN_VALIDATE {
            add_view_state.selected = Self::ensure_enabled_button_forward(add_view_state.selected, validate_enabled, save_enabled, cancel_enabled);
        }
    }

    /// Move focus backward on Shift-Tab.
    fn handle_tab_backward(add_view_state: &mut PluginAddViewState, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) {
        if add_view_state.selected == TRANSPORT { add_view_state.selected = MAX; } else { add_view_state.selected -= 1; }
        add_view_state.selected = Self::skip_hidden_fields_backward(add_view_state.selected, add_view_state.transport);
        if add_view_state.selected >= BTN_VALIDATE {
            add_view_state.selected = Self::ensure_enabled_button_backward(add_view_state.selected, validate_enabled, save_enabled, cancel_enabled);
        }
    }

    /// Navigate left among buttons.
    fn handle_button_left(add_view_state: &mut PluginAddViewState, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) {
        let previous = add_view_state.selected.saturating_sub(1).max(BTN_VALIDATE);
        add_view_state.selected = Self::ensure_enabled_button_backward(previous, validate_enabled, save_enabled, cancel_enabled);
    }

    /// Navigate right among buttons.
    fn handle_button_right(add_view_state: &mut PluginAddViewState, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) {
        if add_view_state.selected < MAX {
            add_view_state.selected += 1;
        }
        add_view_state.selected = Self::ensure_enabled_button_forward(add_view_state.selected, validate_enabled, save_enabled, cancel_enabled);
    }

    /// Handle Enter activation on focused control.
    fn handle_enter(app: &mut crate::app::App, add_view_state: &mut PluginAddViewState, validate_enabled: bool, save_enabled: bool, cancel_enabled: bool) -> Vec<Effect> {
        match add_view_state.selected {
            BTN_VALIDATE if validate_enabled => vec![Effect::PluginsValidateAdd],
            BTN_SAVE if save_enabled => vec![Effect::PluginsApplyAdd],
            BTN_CANCEL if cancel_enabled => {
                app.plugins.add = None;
                app.mark_dirty();
                Vec::new()
            }
            TRANSPORT => {
                add_view_state.transport = match add_view_state.transport {
                    AddTransport::Local => AddTransport::Remote,
                    AddTransport::Remote => AddTransport::Local,
                };
                app.mark_dirty();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Handle backspace within text inputs.
    fn handle_backspace(add_view_state: &mut PluginAddViewState) {
        match add_view_state.selected {
            NAME => { add_view_state.name.pop(); }
            COMMAND => { add_view_state.command.pop(); }
            ARGS => { add_view_state.args.pop(); }
            BASE_URL => { add_view_state.base_url.pop(); }
            _ => {}
        }
    }

    /// Handle char input into text fields.
    fn handle_char(add_view_state: &mut PluginAddViewState, c: char) {
        match add_view_state.selected {
            NAME => add_view_state.name.push(c),
            COMMAND => add_view_state.command.push(c),
            ARGS => add_view_state.args.push(c),
            BASE_URL => add_view_state.base_url.push(c),
            _ => {}
        }
    }

    /// Render the Local/Remote transport radio group (single-select).
    fn render_transport_switch(&self, frame: &mut Frame, inner: Rect, theme: &dyn Theme, add_view_state: &PluginAddViewState) {
        // Draw a single line: "Transport: [✓] Local   [ ] Remote"
        let switch_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };

        let mut spans: Vec<Span> = Vec::new();
        // Label
        spans.push(Span::styled("Transport: ", theme.text_secondary_style()));

        // Helper to render a radio item styled like builder's create_field_value
        let render_radio = |label: &str, selected: bool| -> Vec<Span<'static>> {
            let mut v: Vec<Span<'static>> = Vec::new();
            if selected {
                v.push(Span::styled("[✓]", theme.status_success()));
            } else {
                v.push(Span::styled("[ ]", theme.text_primary_style()));
            }
            v.push(Span::raw(" "));
            v.push(Span::styled(label.to_string(), theme.text_primary_style()));
            v
        };

        let is_local = matches!(add_view_state.transport, AddTransport::Local);
        for s in render_radio("Local", is_local) { spans.push(s); }
        spans.push(Span::raw("   "));
        for s in render_radio("Remote", !is_local) { spans.push(s); }

        // If the control is focused, emphasize the line
        let line = Line::from(spans);
        let styled_line = if add_view_state.selected == TRANSPORT {
            line.style(theme.selection_style())
        } else {
            line
        };

        let paragraph = Paragraph::new(styled_line).style(theme.text_primary_style());
        frame.render_widget(paragraph, switch_area);
    }

    /// Render form fields (labels + values) with focus markers.
    fn render_fields(&self, frame: &mut Frame, fields_area: Rect, theme: &dyn Theme, add_view_state: &PluginAddViewState, selected_index: usize) {
        let mut lines: Vec<Line> = Vec::new();
        let field_line = |idx: usize, label: &str, value: &str, placeholder: &str| {
            let mut spans: Vec<Span> = Vec::new();
            let prefix = if selected_index == idx { "› " } else { "  " };
            spans.push(Span::styled(prefix, theme.text_secondary_style()));
            spans.push(Span::styled(format!("{}: ", label), theme.text_secondary_style()));
            if value.is_empty() {
                spans.push(Span::styled(placeholder.to_string(), theme.text_muted_style().add_modifier(Modifier::DIM)));
            } else {
                spans.push(Span::styled(value.to_string(), theme.text_primary_style()));
            }
            Line::from(spans)
        };

        lines.push(field_line(NAME, "Name", &add_view_state.name, "github"));
        match add_view_state.transport {
            AddTransport::Local => {
                lines.push(field_line(COMMAND, "Command", &add_view_state.command, "npx"));
                lines.push(field_line(ARGS, "Args", &add_view_state.args, "-y @modelcontextprotocol/server-github"));
            }
            AddTransport::Remote => {
                lines.push(field_line(BASE_URL, "Base URL", &add_view_state.base_url, "https://mcp.example.com"));
            }
        }

        let paragraph = Paragraph::new(lines).style(theme.text_primary_style());
        frame.render_widget(paragraph, fields_area);
    }

    /// Render Validate/Save/Cancel buttons pinned to the lower-right.
    fn render_buttons(&self, frame: &mut Frame, buttons_area: Rect, theme: &dyn Theme, add_view_state: &PluginAddViewState) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),     // spacer pushes buttons to the right
                Constraint::Length(12), // Validate
                Constraint::Length(2),  // gap
                Constraint::Length(10), // Save
                Constraint::Length(2),  // gap
                Constraint::Length(12), // Cancel
            ])
            .split(buttons_area);

        let (validate_enabled, save_enabled, _cancel_enabled) = Self::compute_button_enablement(add_view_state);
        self.render_button(frame, columns[1], "Validate", validate_enabled, add_view_state.selected == BTN_VALIDATE, theme);
        self.render_button(frame, columns[3], "Save", save_enabled, add_view_state.selected == BTN_SAVE, theme);
        self.render_button(frame, columns[5], "Cancel", true, add_view_state.selected == BTN_CANCEL, theme);
    }

    /// Render validation and preview messages above the buttons.
    fn render_messages(&self, frame: &mut Frame, inner: Rect, buttons_area: Rect, theme: &dyn Theme, add_view_state: &PluginAddViewState) {
        let visible_fields_count: u16 = match add_view_state.transport { AddTransport::Local => 3, AddTransport::Remote => 2 };
        let messages_top = inner.y.saturating_add(1 + visible_fields_count + 1); // switch + fields + spacer
        let messages_height = buttons_area.y.saturating_sub(messages_top);
        let messages_area = Rect { x: inner.x, y: messages_top, width: inner.width, height: messages_height };

        let mut lines: Vec<String> = Vec::new();
        if let Some(message) = &add_view_state.validation {
            lines.push(format!("Validation: {}", message));
        }
        if let Some(preview) = &add_view_state.preview {
            lines.push(String::new());
            lines.push("Preview:".to_string());
            lines.push(preview.clone());
        }
        if !lines.is_empty() {
            let paragraph = Paragraph::new(lines.join("\n")).style(theme.text_primary_style());
            frame.render_widget(paragraph, messages_area);
        }
    }

    /// Place the cursor at the end of the active input value.
    fn position_cursor(&self, frame: &mut Frame, fields_area: Rect, add_view_state: &PluginAddViewState, selected_index: usize) {
        match selected_index {
            NAME | COMMAND | ARGS | BASE_URL => {
                let line_index = match add_view_state.transport {
                    AddTransport::Local => match selected_index {
                        NAME => 0,
                        COMMAND => 1,
                        ARGS => 2,
                        _ => 0,
                    },
                    AddTransport::Remote => match selected_index {
                        NAME => 0,
                        BASE_URL => 1,
                        _ => 0,
                    },
                };
                let (label_len, value_len) = match selected_index {
                    NAME => (2 + "Name: ".len(), add_view_state.name.chars().count()),
                    COMMAND => (2 + "Command: ".len(), add_view_state.command.chars().count()),
                    ARGS => (2 + "Args: ".len(), add_view_state.args.chars().count()),
                    _ => (2 + "Base URL: ".len(), add_view_state.base_url.chars().count()),
                };
                let x = fields_area.x + label_len as u16 + value_len as u16;
                let y = fields_area.y + line_index as u16;
                frame.set_cursor_position((x, y));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_add_component_constructs() {
        let _c = PluginsAddComponent::default();
        assert!(true);
    }
}
