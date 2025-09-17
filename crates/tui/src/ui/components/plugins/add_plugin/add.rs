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

use crate::ui::theme::{Theme, helpers as theme_helpers};
use crate::ui::{components::component::Component, theme::helpers::render_button};

use super::state::{AddTransport, PluginAddViewState};

/// Component for the add plugin plugin interface.
///
/// This component handles the UI for adding new MCP plugins to the system.
/// It provides form fields for plugin configuration, transport selection,
/// and action buttons for validation and saving. The component manages
/// keyboard input, focus navigation, and rendering of the plugin interface.
#[derive(Debug, Default)]
pub struct PluginsAddComponent;

impl Component for PluginsAddComponent {
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

        match key_event.code {
            KeyCode::Esc => {
                app.plugins.add = None;
            }
            KeyCode::Left if is_transport_focused => {
                add_state.transport = AddTransport::Local;
            }
            KeyCode::Right if is_transport_focused => {
                add_state.transport = AddTransport::Remote;
            }
            KeyCode::Char(' ') if is_transport_focused => {
                add_state.transport = match add_state.transport {
                    AddTransport::Local => AddTransport::Remote,
                    AddTransport::Remote => AddTransport::Local,
                };
            }
            KeyCode::Char('v') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsValidateAdd];
            }
            KeyCode::Char('a') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsApplyAdd];
            }
            KeyCode::Enter => {
                let effects = handle_enter_key(add_state);
                if !effects.is_empty() {
                    return effects;
                }
            }
            KeyCode::Backspace => {
                handle_backspace_key(add_state);
            }
            KeyCode::Char(character) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                handle_character_input(add_state, character);
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
        if let Some(add_state) = &app.plugins.add {
            let theme = &*app.ctx.theme;
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style(add_state.focus.get()))
                .style(theme_helpers::panel_style(theme))
                .title(Span::styled("Add Plugin", theme.text_secondary_style()));

            let inner_area = block.inner(area);
            frame.render_widget(block, area);

            // Transport selection row
            let transport_area = Rect {
                x: inner_area.x,
                y: inner_area.y,
                width: inner_area.width,
                height: 1,
            };

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

            for span in create_radio_button("Local", matches!(add_state.transport, AddTransport::Local)) {
                transport_spans.push(span);
            }
            transport_spans.push(Span::raw("   "));
            for span in create_radio_button("Remote", matches!(add_state.transport, AddTransport::Remote)) {
                transport_spans.push(span);
            }

            let transport_line = Line::from(transport_spans);
            let is_transport_focused = add_state.f_transport.get();
            let styled_transport_line = if is_transport_focused {
                transport_line.style(theme.selection_style())
            } else {
                transport_line
            };

            frame.render_widget(
                Paragraph::new(styled_transport_line).style(theme.text_primary_style()),
                transport_area,
            );

            // Form fields section
            let fields_area = Rect {
                x: inner_area.x,
                y: inner_area.y + 1,
                width: inner_area.width,
                height: inner_area.height.saturating_sub(1),
            };
            render_form_fields(frame, fields_area, theme, add_state);

            // Action buttons section
            let button_row_height = 3u16;
            let buttons_area = Rect {
                x: inner_area.x,
                y: inner_area
                    .y
                    .saturating_add(inner_area.height.saturating_sub(button_row_height)),
                width: inner_area.width,
                height: button_row_height,
            };
            render_action_buttons(frame, buttons_area, theme, add_state);

            // Position the cursor in the active input field
            position_cursor_in_active_field(frame, fields_area, add_state);
        }
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
fn handle_enter_key(state: &mut PluginAddViewState) -> Vec<Effect> {
    let (validate_enabled, save_enabled) = state.compute_button_enablement();

    if state.f_btn_validate.get() {
        return if validate_enabled {
            vec![Effect::PluginsValidateAdd]
        } else {
            Vec::new()
        };
    }
    if state.f_btn_save.get() && save_enabled {
        return vec![Effect::PluginsApplyAdd];
    }
    if state.f_btn_cancel.get() {
        return vec![Effect::PluginsCancel];
    }
    if state.f_btn_secrets.get() {
        return vec![Effect::PluginsOpenSecrets(state.name.clone())];
    }
    if state.f_transport.get() {
        state.transport = match state.transport {
            AddTransport::Local => AddTransport::Remote,
            AddTransport::Remote => AddTransport::Local,
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
fn handle_backspace_key(add_state: &mut PluginAddViewState) {
    if add_state.f_name.get() {
        add_state.name.pop();
        return;
    }
    if add_state.f_command.get() {
        add_state.command.pop();
        return;
    }
    if add_state.f_args.get() {
        add_state.args.pop();
        return;
    }
    if add_state.f_base_url.get() {
        add_state.base_url.pop();
        return;
    }
    if add_state.f_key_value_pairs.get() {
        match add_state.transport {
            AddTransport::Local => add_state.env_input.pop(),
            AddTransport::Remote => add_state.headers_input.pop(),
        };
    }
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
fn handle_character_input(add_state: &mut PluginAddViewState, character: char) {
    if add_state.f_name.get() {
        add_state.name.push(character);
        return;
    }
    if add_state.f_command.get() {
        add_state.command.push(character);
        return;
    }
    if add_state.f_args.get() {
        add_state.args.push(character);
        return;
    }
    if add_state.f_base_url.get() {
        add_state.base_url.push(character);
        return;
    }
    if add_state.f_key_value_pairs.get() {
        match add_state.transport {
            AddTransport::Local => add_state.env_input.push(character),
            AddTransport::Remote => add_state.headers_input.push(character),
        }
    }
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
fn render_form_fields(frame: &mut Frame, fields_area: Rect, theme: &dyn Theme, add_state: &PluginAddViewState) {
    let mut field_lines: Vec<Line> = Vec::new();

    let create_field_line =
        |is_focused: bool, label: &str, value: &str, placeholder: &str, theme: &dyn Theme| -> Line {
            let mut field_spans: Vec<Span> = Vec::new();

            // Add focus indicator
            field_spans.push(Span::styled(
                if is_focused { "› " } else { "  " },
                theme.text_secondary_style(),
            ));

            // Add field label
            field_spans.push(Span::styled(format!("{}: ", label), theme.text_primary_style()));

            // Add field value or placeholder
            if value.is_empty() {
                field_spans.push(Span::styled(placeholder.to_string(), theme.text_muted_style()));
            } else {
                field_spans.push(Span::styled(value.to_string(), theme.text_primary_style()));
            }

            Line::from(field_spans)
        };

    // Always show the name field
    field_lines.push(create_field_line(
        add_state.f_name.get(),
        "Name",
        &add_state.name,
        "github",
        theme,
    ));

    // Show transport-specific fields
    match add_state.transport {
        AddTransport::Local => {
            field_lines.push(create_field_line(
                add_state.f_command.get(),
                "Command",
                &add_state.command,
                "npx",
                theme,
            ));
            field_lines.push(create_field_line(
                add_state.f_args.get(),
                "Args",
                &add_state.args,
                "-y @modelcontextprotocol/server-github",
                theme,
            ));
            field_lines.push(create_field_line(
                add_state.f_key_value_pairs.get(),
                "Env Vars",
                &add_state.env_input,
                "FOO=bar, HEROKU_API_TOKEN=${env:HEROKU_API_TOKEN}",
                theme,
            ));
        }
        AddTransport::Remote => {
            field_lines.push(create_field_line(
                add_state.f_base_url.get(),
                "Base URL",
                &add_state.base_url,
                "https://mcp.example.com",
                theme,
            ));
            field_lines.push(create_field_line(
                add_state.f_key_value_pairs.get(),
                "Headers",
                &add_state.headers_input,
                "Authorization=Bearer ${secret:EXAMPLE_TOKEN}",
                theme,
            ));
        }
    }

    if let Some(message) = add_state.validation.clone() {}

    frame.render_widget(
        Paragraph::new(field_lines).style(theme.text_primary_style()),
        fields_area,
    );
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
fn render_action_buttons(frame: &mut Frame, buttons_area: Rect, theme: &dyn Theme, add_state: &PluginAddViewState) {
    let button_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12), // Secrets button
            Constraint::Length(2),  // Spacer
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
        button_columns[0],
        "Secrets",
        save_enabled,
        add_state.f_btn_secrets.get(),
        false,
        theme,
        Borders::ALL,
    );
    render_button(
        frame,
        button_columns[3],
        "Validate",
        validate_enabled,
        add_state.f_btn_validate.get(),
        false,
        theme,
        Borders::ALL,
    );
    render_button(
        frame,
        button_columns[5],
        "Save",
        save_enabled,
        add_state.f_btn_save.get(),
        false,
        theme,
        Borders::ALL,
    );
    render_button(
        frame,
        button_columns[7],
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
/// * `fields_area` - The rectangular area containing the form fields
/// * `add_state` - Reference to the add plugin plugin state
fn position_cursor_in_active_field(frame: &mut Frame, fields_area: Rect, add_state: &PluginAddViewState) {
    // Only place the cursor for editable fields
    let mut maybe_line_label_value: Option<(u16, usize, usize)> = None;

    match add_state.transport {
        AddTransport::Local => {
            if add_state.f_name.get() {
                maybe_line_label_value = Some((0, 2 + "Name: ".len(), add_state.name.chars().count()));
            } else if add_state.f_command.get() {
                maybe_line_label_value = Some((1, 2 + "Command: ".len(), add_state.command.chars().count()));
            } else if add_state.f_args.get() {
                maybe_line_label_value = Some((2, 2 + "Args: ".len(), add_state.args.chars().count()));
            } else if add_state.f_key_value_pairs.get() {
                maybe_line_label_value = Some((3, 2 + "Env Vars: ".len(), add_state.env_input.chars().count()));
            }
        }
        AddTransport::Remote => {
            if add_state.f_name.get() {
                maybe_line_label_value = Some((0, 2 + "Name: ".len(), add_state.name.chars().count()));
            } else if add_state.f_base_url.get() {
                maybe_line_label_value = Some((1, 2 + "Base URL: ".len(), add_state.base_url.chars().count()));
            } else if add_state.f_key_value_pairs.get() {
                maybe_line_label_value = Some((2, 2 + "Headers: ".len(), add_state.headers_input.chars().count()));
            }
        }
    }

    if let Some((line_index, label_length, value_length)) = maybe_line_label_value {
        let cursor_x = fields_area.x + label_length as u16 + value_length as u16;
        let cursor_y = fields_area.y + line_index as u16;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
