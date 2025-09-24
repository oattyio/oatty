//! Add Plugin component for the MCP plugins management interface.
//!
//! This module provides the UI component for adding new MCP plugins to the system.
//! It supports both Local (stdio) and Remote (HTTP/SSE) plugin types with appropriate
//! form fields and validation. The component handles keyboard input, focus management,
//! and rendering of the add plugin plugin interface.

use crossterm::event::{KeyCode, KeyModifiers};
use heroku_types::Effect;
// Focus management uses FocusFlag booleans on state; no ring needed here
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::{components::component::Component, theme::theme_helpers::render_button};
use crate::{
    app::App,
    ui::theme::{Theme, theme_helpers},
};

use super::{
    key_value_editor::KeyValueEditorComponent,
    state::{PluginEditViewState, PluginTransport},
};

struct EditPluginFormLayout {
    name_area: Rect,
    command_area: Rect,
    args_area: Rect,
    base_url_area: Rect,
}

/// Component for the add plugin plugin interface.
///
/// This component handles the UI for adding new MCP plugins to the system.
/// It provides form fields for plugin configuration, transport selection,
/// and action buttons for validation and saving. The component manages
/// keyboard input, focus navigation, and rendering of the plugin interface.
#[derive(Debug, Default)]
pub struct PluginsEditComponent {
    kv_component: KeyValueEditorComponent,
}

impl Component for PluginsEditComponent {
    /// Handles keyboard events for the add plugin plugin.
    ///
    /// This method processes keyboard input for the add plugin interface,
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
    fn handle_key_events(&mut self, app: &mut crate::app::App, key_event: crossterm::event::KeyEvent) -> Vec<Effect> {
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
            KeyCode::Char(' ') if is_transport_focused => {
                add_state.transport = match add_state.transport {
                    PluginTransport::Local => PluginTransport::Remote,
                    PluginTransport::Remote => PluginTransport::Local,
                };
            }
            KeyCode::Char('v') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsValidateAdd];
            }
            KeyCode::Char('a') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsApplyAdd];
            }
            KeyCode::Enter => {
                let effects = handle_enter_key(app);
                if !effects.is_empty() {
                    return effects;
                }
            }
            KeyCode::Backspace => {
                if handle_backspace_key(add_state).is_some() {
                    add_state.validation = Ok(String::new()); // clear validation
                }
            }
            KeyCode::Char(character) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                if handle_character_input(add_state, character) {
                    add_state.validation = Ok(String::new()); // clear validation
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// Renders the add plugin plugin interface.
    ///
    /// This method renders the complete add plugin plugin including the transport
    /// selection, form fields, and action buttons. It only renders when the
    /// add plugin state is available.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `area` - The rectangular area available for rendering
    /// * `app` - Mutable reference to the app state
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let Some(add_state) = &app.plugins.add else { return };

        let theme = &*app.ctx.theme;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(add_state.focus.get()))
            .style(theme_helpers::panel_style(theme))
            .title(Span::styled("Add Plugin", theme.text_secondary_style()));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Min(1),          // Transport
            Constraint::Min(4),          // Form Fields
            Constraint::Percentage(100), // Env/Headers editors
            Constraint::Min(3),          // Action Buttons
        ])
        .split(inner_area);

        // Transport selection row
        let mut transport_spans: Vec<Span> = Vec::new();
        transport_spans.push(Span::styled("Transport: ", theme.text_secondary_style()));

        let create_radio_button = |label: &str, is_selected: bool| -> Vec<Span<'static>> {
            let mut radio_spans = Vec::new();
            radio_spans.push(Span::styled(
                if is_selected { "[✓]" } else { "[ ]" },
                if is_selected {
                    theme.status_success()
                } else {
                    theme.text_primary_style()
                },
            ));
            radio_spans.push(Span::raw(" "));
            radio_spans.push(Span::styled(label.to_string(), theme.text_primary_style()));
            radio_spans
        };

        for span in create_radio_button("Local", matches!(add_state.transport, PluginTransport::Local)) {
            transport_spans.push(span);
        }
        transport_spans.push(Span::raw("   "));
        for span in create_radio_button("Remote", matches!(add_state.transport, PluginTransport::Remote)) {
            transport_spans.push(span);
        }

        let transport_line = Line::from(transport_spans);
        let is_transport_focused = add_state.f_transport.get();
        let styled_transport_line = if is_transport_focused {
            transport_line.style(theme.selection_style())
        } else {
            transport_line
        };

        frame.render_widget(Paragraph::new(styled_transport_line).style(theme.text_primary_style()), layout[0]);

        let form_layout = render_form_fields(frame, layout[1], theme, add_state);

        self.kv_component.render_with_state(frame, layout[2], theme, add_state);
        render_action_buttons(frame, layout[3], theme, add_state);
        // Position the cursor in the active input field
        position_cursor_in_active_field(frame, &form_layout, add_state);
    }

    fn get_hint_spans(&self, app: &crate::app::App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let add_state = app.plugins.add.as_ref().expect("add state should be something");
        let mut spans = vec![];
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()))
        }

        if add_state.f_transport.get() {
            spans.extend([
                Span::styled("Space bar", theme.accent_emphasis_style()),
                Span::styled(" Toggle ", theme.text_muted_style()),
            ]);
        }

        if add_state.is_key_value_editor_focused() {
            spans.extend(self.kv_component.get_hint_spans(app, is_root));
        } else {
            spans.extend([
                Span::styled("Esc", theme.accent_emphasis_style()),
                Span::styled(" Cancel ", theme.text_muted_style()),
            ]);
            let (validate_enabled, save_enabled) = add_state.compute_button_enablement();
            if validate_enabled {
                spans.extend([
                    Span::styled("Ctrl+V", theme.accent_emphasis_style()),
                    Span::styled(" Validate ", theme.text_muted_style()),
                ]);
            }
            if save_enabled {
                spans.extend([
                    Span::styled("Ctrl+S", theme.accent_emphasis_style()),
                    Span::styled(" Save ", theme.text_muted_style()),
                ]);
            }
        }

        spans
    }
}

/// Handles Enter key presses in the add plugin plugin.
///
/// This function processes Enter key events and triggers the appropriate
/// action based on the currently focused control. For buttons, it triggers
/// their associated effects. For the transport selector, it toggles between
/// Local and Remote modes.
///
/// # Arguments
///
/// * `add_state` - Mutable reference to the add plugin plugin state
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
            return vec![Effect::PluginsApplyAdd];
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

/// Handles Backspace key presses in the add plugin plugin.
///
/// This function removes the last character from the currently focused
/// input field based on the transport type and focused control.
///
/// # Arguments
///
/// * `add_state` - Mutable reference to the add plugin plugin state
fn handle_backspace_key(add_state: &mut PluginEditViewState) -> Option<char> {
    if add_state.f_name.get() {
        return add_state.name.pop();
    }
    if add_state.f_command.get() {
        return add_state.command.pop();
    }
    if add_state.f_args.get() {
        return add_state.args.pop();
    }
    if add_state.f_base_url.get() {
        return add_state.base_url.pop();
    }
    None
}

/// Handles character input in the add plugin plugin.
///
/// This function adds the typed character to the currently focused
/// input field based on the transport type and focused control.
///
/// # Arguments
///
/// * `add_state` - Mutable reference to the add plugin plugin state
/// * `character` - The character to add to the input field
fn handle_character_input(add_state: &mut PluginEditViewState, character: char) -> bool {
    if add_state.f_name.get() {
        add_state.name.push(character);
        return true;
    }
    if add_state.f_command.get() {
        add_state.command.push(character);
        return true;
    }
    if add_state.f_args.get() {
        add_state.args.push(character);
        return true;
    }
    if add_state.f_base_url.get() {
        add_state.base_url.push(character);
        return true;
    }
    false
}

/// Renders the form fields section of the add plugin plugin.
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
/// * `add_state` - Reference to the add plugin plugin state
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
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(if focused { "› " } else { "  " }, theme.text_secondary_style()));
    spans.push(Span::styled(format!("{}: ", label), theme.text_primary_style()));
    if value.is_empty() {
        spans.push(Span::styled(placeholder.to_string(), theme.text_muted_style()));
    } else {
        spans.push(Span::styled(value.to_string(), theme.text_primary_style()));
    }

    let style = if focused {
        theme.selection_style()
    } else {
        theme.text_primary_style()
    };
    let paragraph = Paragraph::new(Line::from(spans)).style(style);
    frame.render_widget(paragraph, area);
}

/// Render a validation message for the add plugin form.
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

/// Renders the action buttons section of the add plugin plugin.
///
/// This function renders the action buttons (Secrets, Validate, Save, Cancel)
/// with appropriate styling based on their enabled state and focus status.
///
/// # Arguments
///
/// * `frame` - Mutable reference to the terminal frame for rendering
/// * `buttons_area` - The rectangular area allocated for action buttons
/// * `theme` - Reference to the UI theme for styling
/// * `add_state` - Reference to the add plugin plugin state
fn render_action_buttons(frame: &mut Frame, buttons_area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) {
    let button_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Flexible space
            Constraint::Length(12), // Validate button
            Constraint::Length(2),  // Spacer
            Constraint::Length(10), // Save button
            Constraint::Length(2),  // Spacer
            Constraint::Length(12), // Cancel button
        ])
        .split(buttons_area);

    let (validate_enabled, save_enabled) = add_state.compute_button_enablement();

    render_button(
        frame,
        button_columns[1],
        "Validate",
        validate_enabled,
        add_state.f_btn_validate.get(),
        false,
        theme,
        Borders::ALL,
    );
    render_button(
        frame,
        button_columns[3],
        "Save",
        save_enabled,
        add_state.f_btn_save.get(),
        false,
        theme,
        Borders::ALL,
    );
    render_button(
        frame,
        button_columns[5],
        "Cancel",
        true,
        add_state.f_btn_cancel.get(),
        false,
        theme,
        Borders::ALL,
    );
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
/// * `add_state` - Reference to the add plugin plugin state
/// Position the terminal cursor based on the currently focused input field.
fn position_cursor_in_active_field(frame: &mut Frame, layout: &EditPluginFormLayout, add_state: &PluginEditViewState) {
    if add_state.is_key_value_editor_focused() {
        // The key/value component manages cursor placement while editing.
        return;
    }

    if add_state.f_name.get() {
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.name_area, "Name", add_state.name.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
        return;
    }

    if add_state.f_command.get() {
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.command_area, "Command", add_state.command.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
        return;
    }

    if add_state.f_args.get() {
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.args_area, "Args", add_state.args.chars().count());
        frame.set_cursor_position((cursor_x, cursor_y));
        return;
    }

    if add_state.f_base_url.get() {
        let (cursor_x, cursor_y) = cursor_position_for_field(layout.base_url_area, "Base URL", add_state.base_url.chars().count());
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
