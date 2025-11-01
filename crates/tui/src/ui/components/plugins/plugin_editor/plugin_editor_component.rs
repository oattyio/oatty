//! Add the Plugin component for the MCP plugins management interface.
//!
//! This module provides the UI component for adding new MCP plugins to the system.
//! It supports both Local (stdio) and Remote (HTTP/SSE) plugin types with appropriate
//! form fields and validation. The component handles keyboard input, focus management,
//! and rendering of the "edit plugin" interface.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::Effect;
// Focus management uses FocusFlag booleans on state; no ring needed here
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Borders, Paragraph},
};
use std::collections::HashMap;

use super::{
    key_value_editor::KeyValueEditorComponent,
    state::{PluginEditViewState, PluginTransport},
};
use crate::ui::theme::theme_helpers::build_syntax_highlighted_line;
use crate::{
    app::App,
    ui::{
        components::{common::TextInputState, component::Component, find_target_index_by_mouse_position},
        theme::{
            Theme,
            theme_helpers::{self, ButtonRenderOptions, create_radio_button, render_button},
        },
    },
};

struct EditPluginFormLayout {
    name_area: Rect,
    command_area: Rect,
    args_area: Rect,
    base_url_area: Rect,
}

struct ActionButtonLayout {
    btn_validate_area: Rect,
    btn_save_area: Rect,
    btn_cancel_area: Rect,
}

struct RadioButtonLayout {
    transport_local: Rect,
    transport_remote: Rect,
}

/// Component for the "edit plugin" plugin interface.
///
/// This component handles the UI for adding new MCP plugins to the system.
/// It provides form fields for plugin configuration, transport selection,
/// and action buttons for validation and saving. The component manages
/// keyboard input, focus navigation, and rendering of the plugin interface.
#[derive(Debug, Default)]
pub struct PluginsEditComponent {
    kv_component: KeyValueEditorComponent,
    // Map Focus widget IDs to persistent text input states for inline fields
    focus_id_to_input: HashMap<usize, TextInputState>,
}

impl PluginsEditComponent {
    fn ensure_inputs_initialized(&mut self, add_state: &PluginEditViewState) {
        // Reinitialize when the map is empty or does not contain the current field IDs
        let ids = [
            add_state.f_name.widget_id(),
            add_state.f_command.widget_id(),
            add_state.f_args.widget_id(),
            add_state.f_base_url.widget_id(),
        ];
        let needs_init = self.focus_id_to_input.is_empty() || ids.iter().any(|id| !self.focus_id_to_input.contains_key(id));
        if needs_init {
            self.focus_id_to_input.clear();
            let mk = |id: usize, input: &str, cursor: usize| -> (usize, TextInputState) {
                let mut ti = TextInputState::new();
                ti.set_input(input);
                ti.set_cursor(cursor);
                (id, ti)
            };
            let entries = [
                mk(add_state.f_name.widget_id(), &add_state.name, add_state.name_cursor),
                mk(add_state.f_command.widget_id(), &add_state.command, add_state.command_cursor),
                mk(add_state.f_args.widget_id(), &add_state.args, add_state.args_cursor),
                mk(add_state.f_base_url.widget_id(), &add_state.base_url, add_state.base_url_cursor),
            ];
            for (id, ti) in entries {
                self.focus_id_to_input.insert(id, ti);
            }
        }
    }

    fn sync_back(add_state: &mut PluginEditViewState, widget_id: usize, ti: &TextInputState) {
        if widget_id == add_state.f_name.widget_id() {
            add_state.name = ti.input().to_string();
            add_state.name_cursor = ti.cursor();
        } else if widget_id == add_state.f_command.widget_id() {
            add_state.command = ti.input().to_string();
            add_state.command_cursor = ti.cursor();
        } else if widget_id == add_state.f_args.widget_id() {
            add_state.args = ti.input().to_string();
            add_state.args_cursor = ti.cursor();
        } else if widget_id == add_state.f_base_url.widget_id() {
            add_state.base_url = ti.input().to_string();
            add_state.base_url_cursor = ti.cursor();
        }
    }

    fn edit_focused_input<F: FnOnce(&mut TextInputState)>(&mut self, app: &mut App, f: F) -> bool {
        let Some(widget_id) = app.focus.focused_widget_id() else {
            return false;
        };
        let Some(add_state) = app.plugins.add.as_mut() else {
            return false;
        };
        self.ensure_inputs_initialized(add_state);
        if let Some(ti) = self.focus_id_to_input.get_mut(&widget_id) {
            f(ti);
            Self::sync_back(add_state, widget_id, ti);
            return true;
        }
        false
    }

    fn edit_and_reset_validation<F: FnOnce(&mut TextInputState)>(&mut self, app: &mut App, f: F) {
        if !self.edit_focused_input(app, f) {
            return;
        }
        if let Some(add_state) = app.plugins.add.as_mut() {
            add_state.validation = Ok(String::new());
        }
    }
}

impl Component for PluginsEditComponent {
    /// Handles keyboard events for the "edit plugin" component.
    ///
    /// This method processes keyboard input for the "edit plugin" interface,
    /// including navigation, text input, and action triggers. It delegates
    /// to specialized handlers for different types of input.
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to the app state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// Returns a vector of effects that should be processed by the app.
    fn handle_key_events(&mut self, app: &mut App, key_event: crossterm::event::KeyEvent) -> Vec<Effect> {
        let Some(add_state) = app.plugins.add.as_mut() else {
            return Vec::new();
        };
        // Use focus flags directly to avoid building a focus ring repeatedly
        let is_transport_focused = add_state.f_transport.get();
        if add_state.is_key_value_editor_focused() {
            return self.kv_component.handle_key_events(app, key_event);
        }

        match key_event.code {
            KeyCode::Esc => {
                app.plugins.add = None;
            }
            KeyCode::Left if is_transport_focused => {
                add_state.transport = PluginTransport::Local;
            }
            KeyCode::Right if is_transport_focused => {
                add_state.transport = PluginTransport::Remote;
            }
            KeyCode::Left => {
                let _ = self.edit_focused_input(app, |ti| ti.move_left());
            }
            KeyCode::Right => {
                let _ = self.edit_focused_input(app, |ti| ti.move_right());
            }
            KeyCode::Char(' ') if is_transport_focused => {
                add_state.transport = match add_state.transport {
                    PluginTransport::Local => PluginTransport::Remote,
                    PluginTransport::Remote => PluginTransport::Local,
                };
            }
            KeyCode::Char('v') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsValidateAdd];
            }
            KeyCode::Char('s') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsSave];
            }
            KeyCode::Enter => {
                return handle_enter_key(app);
            }
            KeyCode::Backspace => self.edit_and_reset_validation(app, |ti| ti.backspace()),
            KeyCode::Char(character) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_and_reset_validation(app, |ti| ti.insert_char(character));
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) || app.plugins.add.is_none() {
            return Vec::new();
        }
        let edit_state = &mut app.plugins.add.as_mut().expect("add state should be something");
        let MouseEvent { column, row, .. } = mouse;
        let PluginEditViewState {
            last_area, per_item_area, ..
        } = edit_state;
        if let Some(idx) = find_target_index_by_mouse_position(last_area, per_item_area, column, row) {
            let focusables = [
                &edit_state.f_transport,    // local transport radio
                &edit_state.f_transport,    // remote transport radio
                &edit_state.f_name,         // Name
                &edit_state.f_command,      // Command
                &edit_state.f_args,         // Args
                &edit_state.f_base_url,     // Base URL
                &edit_state.f_btn_validate, // Validate button
                &edit_state.f_btn_save,     // Save button
                &edit_state.f_btn_cancel,   // Cancel button
            ];
            app.focus.focus(focusables[idx]);
            // normalize the index to the focusables array
            if (2..=8).contains(&idx) {
                return handle_enter_key(app);
            }
        }
        Vec::new()
    }

    /// Renders the "edit plugin" interface.
    ///
    /// This method renders the complete "edit plugin" plugin including the transport
    /// selection, form fields, and action buttons. It only renders when the
    /// "edit plugin" state is available.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `area` - The rectangular area available for rendering
    /// * `app` - Mutable reference to the app state
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let Some(add_state) = &mut app.plugins.add else { return };

        let theme = &*app.ctx.theme;

        let layout = Layout::vertical([
            Constraint::Min(1),          // Transport
            Constraint::Min(4),          // Form Fields
            Constraint::Percentage(100), // Env/Headers editors
            Constraint::Min(3),          // Action Buttons
        ])
        .split(area);

        let transport_layout = render_radio_buttons(frame, layout[0], theme, add_state);
        let form_layout = render_form_fields(frame, layout[1], theme, add_state);

        self.kv_component.render_with_state(frame, layout[2], theme, add_state);
        let button_layout = render_action_buttons(frame, layout[3], theme, add_state);
        // Position the cursor in the active input field
        position_cursor_in_active_field(frame, &form_layout, add_state);

        // Update the state with the new layout
        // so mouse events can be handled
        add_state.last_area = area;
        add_state.per_item_area = vec![
            transport_layout.transport_local,  // Local Radio
            transport_layout.transport_remote, // Remote radio
            form_layout.name_area,             // Name
            form_layout.command_area,          // Command
            form_layout.args_area,             // Args
            form_layout.base_url_area,         // Base URL
            button_layout.btn_validate_area,   // Validate button
            button_layout.btn_save_area,       // Save button
            button_layout.btn_cancel_area,     // Cancel button
        ];
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let add_state = app.plugins.add.as_ref().expect("add state should be something");
        let mut spans = vec![];

        if add_state.f_transport.get() {
            spans.extend(theme_helpers::build_hint_spans(theme, &[("Space bar", " Toggle ")]));
        }

        if add_state.is_key_value_editor_focused() {
            spans.extend(self.kv_component.get_hint_spans(app));
        } else {
            spans.extend(theme_helpers::build_hint_spans(theme, &[("Esc", " Cancel ")]));
            let (validate_enabled, save_enabled) = add_state.compute_button_enablement();
            if validate_enabled {
                spans.extend(theme_helpers::build_hint_spans(theme, &[("Ctrl+V", " Validate ")]));
            }
            if save_enabled {
                spans.extend(theme_helpers::build_hint_spans(theme, &[("Ctrl+S", " Save ")]));
            }
        }

        spans
    }
}

/// Handles Enter key presses in the "edit plugin" plugin.
///
/// This function processes Enter key events and triggers the appropriate
/// action based on the currently focused control. For buttons, it triggers
/// their associated effects. For the transport selector, it toggles between
/// Local and Remote modes.
///
/// # Arguments
///
/// * `add_state` - Mutable reference to the "edit plugin" plugin state
///
/// # Returns
///
/// Returns a vector of effects that should be processed by the app.
fn handle_enter_key(app: &mut App) -> Vec<Effect> {
    let Some(add_state) = &mut app.plugins.add else {
        return vec![];
    };
    let (validate_enabled, save_enabled) = add_state.compute_button_enablement();

    if add_state.f_btn_validate.get() {
        return if validate_enabled {
            vec![Effect::PluginsValidateAdd]
        } else {
            Vec::new()
        };
    }
    if add_state.f_btn_save.get() && save_enabled {
        add_state.validation = add_state.kv_editor.commit_edit();
        if add_state.validation.is_ok() {
            return vec![Effect::PluginsSave];
        }
    }
    if add_state.f_btn_cancel.get() {
        app.plugins.add = None;
        return vec![];
    }
    if add_state.f_transport.get() {
        add_state.transport = match add_state.transport {
            PluginTransport::Local => PluginTransport::Remote,
            PluginTransport::Remote => PluginTransport::Local,
        };
        return Vec::new();
    }

    Vec::new()
}

/// Renders the form fields section of the "edit plugin" plugin.
///
/// This function renders the input fields for plugin configuration based on
/// the selected transport type. It shows different fields for Local vs Remote
/// plugins and highlights the currently focused field.
///
/// # Arguments
///
/// * `frame` - Mutable reference to the terminal frame for rendering
/// * `fields_area` - The rectangular area allocated for form fields
/// * `theme` - Reference to the UI theme for styling
/// * `add_state` - Reference to the "edit plugin" plugin state
fn render_form_fields(frame: &mut Frame, fields_area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) -> EditPluginFormLayout {
    // Always allow for the max rows to prevent
    // layout jitter when toggling transport
    let constraints: Vec<Constraint> = vec![
        Constraint::Length(1), // Name
        Constraint::Length(1), // Command / Base URL
        Constraint::Length(1), // Args or gap
        Constraint::Length(1), // Validation message
    ];

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(fields_area);

    // Name field
    render_labeled_input_field(frame, sections[0], theme, "Name", &add_state.name, "github", add_state.f_name.get());

    // Command + Args or Base URL
    match add_state.transport {
        PluginTransport::Local => {
            render_labeled_input_field(
                frame,
                sections[1],
                theme,
                "Command",
                &add_state.command,
                "npx",
                add_state.f_command.get(),
            );

            render_labeled_input_field(
                frame,
                sections[2],
                theme,
                "Args",
                &add_state.args,
                "-y @modelcontextprotocol/server-github",
                add_state.f_args.get(),
            );
        }
        PluginTransport::Remote => {
            render_labeled_input_field(
                frame,
                sections[1],
                theme,
                "Base URL",
                &add_state.base_url,
                "https://mcp.example.com",
                add_state.f_base_url.get(),
            );
        }
    }
    render_validation_message(frame, sections[3], theme, &add_state.validation);
    EditPluginFormLayout {
        name_area: sections[0],
        command_area: sections[1],
        args_area: sections[2],
        base_url_area: sections[1],
    }
}

fn render_radio_buttons(frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) -> RadioButtonLayout {
    // Transport selection row
    let transport_layout = Layout::horizontal([
        Constraint::Length(11), // Transport label
        Constraint::Length(9),  // Local radio button
        Constraint::Length(2),  // Spacing in-between
        Constraint::Length(10), // Remote radio button
    ])
    .split(area);

    frame.render_widget(Span::styled("Transport: ", theme.text_secondary_style()), transport_layout[0]);
    let is_transport_focused = add_state.f_transport.get();

    let local_transport = create_radio_button(
        "Local",
        matches!(add_state.transport, PluginTransport::Local),
        is_transport_focused,
        theme,
    );
    let remote_transport = create_radio_button(
        "Remote",
        matches!(add_state.transport, PluginTransport::Remote),
        is_transport_focused,
        theme,
    );
    frame.render_widget(local_transport, transport_layout[1]);
    frame.render_widget(remote_transport, transport_layout[3]);

    RadioButtonLayout {
        transport_local: transport_layout[1],
        transport_remote: transport_layout[3],
    }
}

/// Render a single-line labeled input field with optional placeholder text.
fn render_labeled_input_field(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    label: &str,
    value: &str,
    placeholder: &str,
    focused: bool,
) {
    let line = build_syntax_highlighted_line(label, value, placeholder, focused, theme);
    let paragraph_style = if focused { theme.selection_style() } else { Style::default() };
    frame.render_widget(Paragraph::new(line).style(paragraph_style), area);
}

/// Render a validation message for the "edit plugin" form.
fn render_validation_message(frame: &mut Frame, area: Rect, theme: &dyn Theme, result: &Result<String, String>) {
    let (message, style) = match result {
        Ok(message) => (message, theme.status_success()),
        Err(message) => (message, theme.status_error()),
    };

    let spans = vec![
        Span::styled("  ", theme.text_secondary_style()),
        Span::styled(message.to_string(), style),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(theme.status_error());
    frame.render_widget(paragraph, area);
}

/// Renders the action buttons section of the "edit plugin".
///
/// This function renders the action buttons (Secrets, Validate, Save, Cancel)
/// with the appropriate styling based on their enabled state and focus status.
///
/// # Arguments
///
/// * `frame` - Mutable reference to the terminal frame for rendering
/// * `buttons_area` - The rectangular area allocated for action buttons
/// * `theme` - Reference to the UI theme for styling
/// * `add_state` - Reference to the "edit plugin" state
fn render_action_buttons(frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) -> ActionButtonLayout {
    let (validate_enabled, save_enabled) = add_state.compute_button_enablement();
    let button_columns = Layout::horizontal([
        Constraint::Length(12), // Validate button
        Constraint::Length(2),  // Spacer
        Constraint::Length(10), // Save button
        Constraint::Length(2),  // Spacer
        Constraint::Length(12), // Cancel button
    ])
    .split(area);
    render_button(
        frame,
        button_columns[0],
        "Validate",
        theme,
        ButtonRenderOptions::new(validate_enabled, add_state.f_btn_validate.get(), false, Borders::ALL, false),
    );
    render_button(
        frame,
        button_columns[2],
        "Save",
        theme,
        ButtonRenderOptions::new(save_enabled, add_state.f_btn_save.get(), false, Borders::ALL, true),
    );
    render_button(
        frame,
        button_columns[4],
        "Cancel",
        theme,
        ButtonRenderOptions::new(true, add_state.f_btn_cancel.get(), false, Borders::ALL, false),
    );
    ActionButtonLayout {
        btn_validate_area: button_columns[0],
        btn_save_area: button_columns[2],
        btn_cancel_area: button_columns[4],
    }
}

/// Positions the cursor in the currently focused input field.
///
/// This function calculates the appropriate cursor position based on the
/// currently focused control and the content of the input field. It handles
/// different field layouts for Local vs Remote transport types.
///
/// # Arguments
///
/// * `frame` - Mutable reference to the terminal frame for cursor positioning
/// * `layout` - Layout metadata generated during form rendering
/// * `add_state` - Reference to the "edit plugin" plugin state
///
fn position_cursor_in_active_field(frame: &mut Frame, layout: &EditPluginFormLayout, add_state: &PluginEditViewState) {
    if add_state.is_key_value_editor_focused() {
        // The key/value component manages cursor placement while editing.
        return;
    }

    if add_state.f_name.get() {
        let prefix = &add_state.name[..add_state.name_cursor.min(add_state.name.len())];
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.name_area, "Name", prefix.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
        return;
    }

    if add_state.f_command.get() {
        let prefix = &add_state.command[..add_state.command_cursor.min(add_state.command.len())];
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.command_area, "Command", prefix.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
        return;
    }

    if add_state.f_args.get() {
        let prefix = &add_state.args[..add_state.args_cursor.min(add_state.args.len())];
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.args_area, "Args", prefix.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
        return;
    }

    if add_state.f_base_url.get() {
        let prefix = &add_state.base_url[..add_state.base_url_cursor.min(add_state.base_url.len())];
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.base_url_area, "Base URL", prefix.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

/// Compute the cursor position for an inline labeled input field.
fn cursor_position_for_field(area: Rect, label: &str, value_length: usize) -> (u16, u16) {
    let label_prefix = format!("{}: ", label);
    let offset = 2 + label_prefix.chars().count();
    let cursor_x = area.x + offset as u16 + value_length as u16;
    let cursor_y = area.y;
    (cursor_x, cursor_y)
}
