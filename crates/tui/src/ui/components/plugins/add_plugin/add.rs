use std::vec;

use crossterm::event::{KeyCode, KeyModifiers};
use rat_focus::FocusBuilder;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect}, text::{Line, Span}, widgets::{Block, Borders, Paragraph}, Frame
};

use crate::app::Effect;
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::state::{AddTransport, PluginAddViewState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddControl {
    Transport,
    Name,
    Command,
    Args,
    BaseUrl,
    KeyValuePairs,
    BtnSecrets,
    BtnValidate,
    BtnSave,
    BtnCancel,
}

#[derive(Debug, Default)]
pub struct PluginsAddComponent;

impl Component for PluginsAddComponent {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key: crossterm::event::KeyEvent) -> Vec<Effect> {
        let Some(add) = app.plugins.add.as_mut() else {
            return Vec::new();
        };
        let focused = focused_control(add);
        match key.code {
            KeyCode::Esc => {
                app.plugins.add = None;
                app.mark_dirty();
            }
            KeyCode::Left if matches!(focused, AddControl::Transport) => {
                add.transport = AddTransport::Local;
                app.mark_dirty();
            }
            KeyCode::Right if matches!(focused, AddControl::Transport) => {
                add.transport = AddTransport::Remote;
                app.mark_dirty();
            }
            KeyCode::Char(' ') if matches!(focused, AddControl::Transport) => {
                add.transport = match add.transport {
                    AddTransport::Local => AddTransport::Remote,
                    AddTransport::Remote => AddTransport::Local,
                };
                app.mark_dirty();
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsValidateAdd];
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::PluginsApplyAdd];
            }
            KeyCode::Enter => {
                let effects = handle_enter(add);
                if !effects.is_empty() {
                    return effects;
                }
            }
            KeyCode::Backspace => {
                handle_backspace(add);
                app.mark_dirty();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                handle_char(add, c);
                app.mark_dirty();
            }
            _ => {}
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        if let Some(add) = &app.plugins.add {
            render_add(frame, area, &*app.ctx.theme, add);
        }
    }
}

fn focused_control(add: &PluginAddViewState) -> AddControl {
    let mut b = FocusBuilder::new(None);
    b.widget(add);
    let f = b.build();
    if let Some(cur) = f.focused() {
        if cur == add.f_transport {
            return AddControl::Transport;
        }
        if cur == add.f_name {
            return AddControl::Name;
        }
        if cur == add.f_command {
            return AddControl::Command;
        }
        if cur == add.f_args {
            return AddControl::Args;
        }
        if cur == add.f_base_url {
            return AddControl::BaseUrl;
        }
        if cur == add.f_key_value_pairs {
            return AddControl::KeyValuePairs;
        }
        if cur == add.f_btn_secrets {
            return AddControl::BtnSecrets;
        }
        if cur == add.f_btn_validate {
            return AddControl::BtnValidate;
        }
        if cur == add.f_btn_save {
            return AddControl::BtnSave;
        }
        if cur == add.f_btn_cancel {
            return AddControl::BtnCancel;
        }
    }
    AddControl::Name
}

fn compute_button_enablement(add: &PluginAddViewState) -> (bool, bool) {
    let name_present = !add.name.trim().is_empty();
    match add.transport {
        AddTransport::Local => {
            let cmd = !add.command.trim().is_empty();
            (cmd, name_present && cmd)
        }
        AddTransport::Remote => {
            let base = !add.base_url.trim().is_empty();
            (base, name_present && base)
        }
    }
}

fn handle_enter(add: &mut PluginAddViewState) -> Vec<Effect> {
    let (_v, save) = compute_button_enablement(add);
    match focused_control(add) {
        AddControl::BtnValidate => vec![Effect::PluginsValidateAdd],
        AddControl::BtnSave if save => vec![Effect::PluginsApplyAdd],
        AddControl::BtnCancel => vec![Effect::PluginsCancel],
        AddControl::BtnSecrets => vec![Effect::PluginsOpenSecrets(add.name.clone())],
        AddControl::Transport => {
            add.transport = match add.transport {
                AddTransport::Local => AddTransport::Remote,
                AddTransport::Remote => AddTransport::Local,
            };
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn handle_backspace(add: &mut PluginAddViewState) {
    match focused_control(add) {
        AddControl::Name => {
            add.name.pop();
        }
        AddControl::Command => {
            add.command.pop();
        }
        AddControl::Args => {
            add.args.pop();
        }
        AddControl::BaseUrl => {
            add.base_url.pop();
        }
        AddControl::KeyValuePairs => match add.transport {
            AddTransport::Local => {
                add.env_input.pop();
            }
            AddTransport::Remote => {
                add.headers_input.pop();
            }
        },
        _ => {}
    }
}

fn handle_char(add: &mut PluginAddViewState, c: char) {
    match focused_control(add) {
        AddControl::Name => add.name.push(c),
        AddControl::Command => add.command.push(c),
        AddControl::Args => add.args.push(c),
        AddControl::BaseUrl => add.base_url.push(c),
        AddControl::KeyValuePairs => match add.transport {
            AddTransport::Local => add.env_input.push(c),
            AddTransport::Remote => add.headers_input.push(c),
        },
        _ => {}
    }
}

fn render_add(frame: &mut Frame, area: Rect, theme: &dyn Theme, add: &PluginAddViewState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style(true))
        .style(th::panel_style(theme))
        .title(Span::styled("Add Plugin", theme.text_secondary_style()));
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    // Transport row
    let switch_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled("Transport: ", theme.text_secondary_style()));
    let add_radio = |label: &str, selected: bool| -> Vec<Span<'static>> {
        let mut v = Vec::new();
        v.push(Span::styled(
            if selected { "[✓]" } else { "[ ]" },
            if selected {
                theme.status_success()
            } else {
                theme.text_primary_style()
            },
        ));
        v.push(Span::raw(" "));
        v.push(Span::styled(label.to_string(), theme.text_primary_style()));
        v
    };
    for s in add_radio("Local", matches!(add.transport, AddTransport::Local)) {
        spans.push(s);
    }
    spans.push(Span::raw("   "));
    for s in add_radio("Remote", matches!(add.transport, AddTransport::Remote)) {
        spans.push(s);
    }
    let line = Line::from(spans);
    let focused = matches!(focused_control(add), AddControl::Transport);
    let styled_line = if focused {
        line.style(theme.selection_style())
    } else {
        line
    };
    frame.render_widget(
        Paragraph::new(styled_line).style(theme.text_primary_style()),
        switch_area,
    );

    // Fields block
    let fields_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    render_fields(frame, fields_area, theme, add);

    // Buttons row
    let button_row_height = 3u16;
    let buttons_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(inner.height.saturating_sub(button_row_height)),
        width: inner.width,
        height: button_row_height,
    };
    render_buttons(frame, buttons_area, theme, add);
    // Position the cursor in the active input field
    position_cursor(frame, fields_area, add);
}

fn render_fields(frame: &mut Frame, fields_area: Rect, theme: &dyn Theme, add: &PluginAddViewState) {
    let mut lines: Vec<Line> = Vec::new();
    let render_line = |ctl: AddControl, label: &str, value: &str, placeholder: &str, theme: &dyn Theme| -> Line {
        let sel = focused_control(add) == ctl;
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            if sel { "› " } else { "  " },
            theme.text_secondary_style(),
        ));
        spans.push(Span::styled(format!("{}: ", label), theme.text_primary_style()));
        if value.is_empty() {
            spans.push(Span::styled(placeholder.to_string(), theme.text_muted_style()));
        } else {
            spans.push(Span::styled(value.to_string(), theme.text_primary_style()));
        }
        Line::from(spans)
    };
    lines.push(render_line(AddControl::Name, "Name", &add.name, "github", theme));
    match add.transport {
        AddTransport::Local => {
            lines.push(render_line(AddControl::Command, "Command", &add.command, "npx", theme));
            lines.push(render_line(
                AddControl::Args,
                "Args",
                &add.args,
                "-y @modelcontextprotocol/server-github",
                theme,
            ));
            lines.push(render_line(
                AddControl::KeyValuePairs,
                "Env Vars",
                &add.env_input,
                "FOO=bar, HEROKU_API_TOKEN=${env:HEROKU_API_TOKEN}",
                theme,
            ));
        }
        AddTransport::Remote => {
            lines.push(render_line(
                AddControl::BaseUrl,
                "Base URL",
                &add.base_url,
                "https://mcp.example.com",
                theme,
            ));
            lines.push(render_line(
                AddControl::KeyValuePairs,
                "Headers",
                &add.headers_input,
                "Authorization=Bearer ${secret:EXAMPLE_TOKEN}",
                theme,
            ));
        }
    }
    frame.render_widget(Paragraph::new(lines).style(theme.text_primary_style()), fields_area);
}

fn render_buttons(frame: &mut Frame, buttons_area: Rect, theme: &dyn Theme, add: &PluginAddViewState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(12),
            Constraint::Length(2),
            Constraint::Length(10),
            Constraint::Length(2),
            Constraint::Length(12),
        ])
        .split(buttons_area);
    let (validate_enabled, save_enabled) = compute_button_enablement(add);
    let focused = focused_control(add);
    let render_btn =
        |frame: &mut Frame, area: Rect, label: &str, enabled: bool, is_focused: bool, theme: &dyn Theme| {
            let border = if enabled {
                theme.border_style(is_focused)
            } else {
                theme.text_muted_style()
            };
            let style = if enabled {
                th::button_secondary_style(theme, true)
            } else {
                theme.text_muted_style()
            };
            frame.render_widget(
                Paragraph::new(label).alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).border_style(border))
                    .style(style),
                area,
            );
        };
    render_btn(
        frame,
        columns[0],
        "Secrets",
        true,
        matches!(focused, AddControl::BtnSecrets),
        theme,
    );
    render_btn(
        frame,
        columns[3],
        "Validate",
        validate_enabled,
        matches!(focused, AddControl::BtnValidate),
        theme,
    );
    render_btn(
        frame,
        columns[5],
        "Save",
        save_enabled,
        matches!(focused, AddControl::BtnSave),
        theme,
    );
    render_btn(
        frame,
        columns[7],
        "Cancel",
        true,
        matches!(focused, AddControl::BtnCancel),
        theme,
    );
}
fn position_cursor(frame: &mut Frame, fields_area: Rect, add: &PluginAddViewState) {
    let sel = focused_control(add);
    match sel {
        AddControl::Name | AddControl::Command | AddControl::Args | AddControl::BaseUrl | AddControl::KeyValuePairs => {
            let (line_index, label_len, value_len) = match add.transport {
                AddTransport::Local => match sel {
                    AddControl::Name => (0, 2 + "Name: ".len(), add.name.chars().count()),
                    AddControl::Command => (1, 2 + "Command: ".len(), add.command.chars().count()),
                    AddControl::Args => (2, 2 + "Args: ".len(), add.args.chars().count()),
                    AddControl::KeyValuePairs => (3, 2 + "Env Vars: ".len(), add.env_input.chars().count()),
                    _ => (0, 0, 0),
                },
                AddTransport::Remote => match sel {
                    AddControl::Name => (0, 2 + "Name: ".len(), add.name.chars().count()),
                    AddControl::BaseUrl => (1, 2 + "Base URL: ".len(), add.base_url.chars().count()),
                    AddControl::KeyValuePairs => (2, 2 + "Headers: ".len(), add.headers_input.chars().count()),
                    _ => (0, 0, 0),
                },
            };
            let x = fields_area.x + label_len as u16 + value_len as u16;
            let y = fields_area.y + line_index as u16;
            frame.set_cursor_position((x, y));
        }
        _ => {}
    }
}
