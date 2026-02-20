//! MCP HTTP server control view.

use crate::app::App;
use crate::ui::components::Component;
use crate::ui::components::mcp_server::state::{McpHttpServerState, McpHttpServerStatus};
use crate::ui::theme::theme_helpers::{self as th, ButtonRenderOptions, ButtonType, create_checkbox, render_button};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::Effect;
use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    widgets::Paragraph,
};

#[derive(Debug, Default)]
pub struct McpHttpServerLayout {
    pub start_stop_button: Rect,
    pub auto_start_checkbox: Rect,
    pub status_area: Rect,
    pub controls_area: Rect,
    pub details_area: Rect,
}

#[derive(Debug, Default)]
pub struct McpHttpServerComponent {
    layout: McpHttpServerLayout,
}

impl McpHttpServerComponent {
    fn can_start(state: &McpHttpServerState) -> bool {
        matches!(state.status, McpHttpServerStatus::Stopped | McpHttpServerStatus::Error)
    }

    fn can_stop(state: &McpHttpServerState) -> bool {
        matches!(state.status, McpHttpServerStatus::Running)
    }

    fn start_stop_label(state: &McpHttpServerState) -> &'static str {
        if matches!(state.status, McpHttpServerStatus::Running | McpHttpServerStatus::Stopping) {
            "Stop"
        } else {
            "Start"
        }
    }

    fn start_stop_button_type(state: &McpHttpServerState) -> ButtonType {
        if Self::can_stop(state) {
            ButtonType::Destructive
        } else {
            ButtonType::Primary
        }
    }

    fn handle_start_stop(&self, app: &mut App) -> Vec<Effect> {
        let state = &mut app.mcp_http_server;
        if Self::can_start(state) {
            state.mark_starting();
            return vec![Effect::McpHttpServerStart];
        }
        if Self::can_stop(state) {
            state.mark_stopping();
            return vec![Effect::McpHttpServerStop];
        }
        Vec::new()
    }

    fn handle_auto_start_toggle(&self, app: &mut App) -> Vec<Effect> {
        app.mcp_http_server.toggle_auto_start();
        vec![Effect::McpHttpServerSetAutostart {
            auto_start: app.mcp_http_server.auto_start,
        }]
    }

    fn render_status(&mut self, frame: &mut Frame, app: &App) {
        let theme = &*app.ctx.theme;
        let status_style = match app.mcp_http_server.status {
            McpHttpServerStatus::Running => theme.status_success(),
            McpHttpServerStatus::Starting | McpHttpServerStatus::Stopping => theme.status_warning(),
            McpHttpServerStatus::Error => theme.status_error(),
            McpHttpServerStatus::Stopped => theme.text_muted_style(),
        };
        let status_line = Line::from(vec![
            Span::styled("Status: ", theme.text_muted_style()),
            Span::styled(app.mcp_http_server.status.label(), status_style),
        ]);
        let paragraph = Paragraph::new(status_line);
        frame.render_widget(paragraph, self.layout.status_area);
    }

    fn render_controls(&mut self, frame: &mut Frame, app: &App) {
        let theme = &*app.ctx.theme;
        let controls = Layout::horizontal([
            Constraint::Length(12), // Start/Stop button
            Constraint::Length(2),  // Spacer
            Constraint::Length(20), // Auto-start checkbox
            Constraint::Min(0),
        ])
        .split(self.layout.controls_area);
        self.layout.start_stop_button = controls[0];
        self.layout.auto_start_checkbox = controls[2];

        let state = &app.mcp_http_server;
        let enabled = Self::can_start(state) || Self::can_stop(state);
        let button_opts = ButtonRenderOptions::new(
            enabled,
            state.start_stop_focus.get(),
            false,
            ratatui::widgets::Borders::ALL,
            Self::start_stop_button_type(state),
        );
        render_button(
            frame,
            self.layout.start_stop_button,
            Self::start_stop_label(state),
            theme,
            button_opts,
        );

        let checkbox = create_checkbox(Some("Auto-start"), state.auto_start, state.auto_start_focus.get(), theme);
        frame.render_widget(Paragraph::new(checkbox), self.layout.auto_start_checkbox);
    }

    fn render_details(&mut self, frame: &mut Frame, app: &App) {
        let theme = &*app.ctx.theme;
        let configured = &app.mcp_http_server.configured_bind_address;
        let bound = app.mcp_http_server.bound_address.as_deref().unwrap_or("not running");
        let endpoint = if matches!(app.mcp_http_server.status, McpHttpServerStatus::Running) {
            format!("http://{bound}/mcp")
        } else {
            "not running".to_string()
        };
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Configured bind: ", theme.text_muted_style()),
                Span::styled(configured.clone(), theme.syntax_string_style()),
            ]),
            Line::from(vec![
                Span::styled("Active endpoint: ", theme.text_muted_style()),
                Span::styled(endpoint, theme.syntax_string_style()),
            ]),
            Line::from(vec![
                Span::styled("Connected clients: ", theme.text_muted_style()),
                Span::styled(app.mcp_http_server.connected_clients.to_string(), theme.syntax_number_style()),
            ]),
        ];
        if let Some(error) = app.mcp_http_server.last_error.as_ref() {
            lines.push(Line::from(vec![
                Span::styled("Last error: ", theme.text_muted_style()),
                Span::styled(error.clone(), theme.status_error()),
            ]));
        }
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, self.layout.details_area);
    }
}

impl Component for McpHttpServerComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if app.mcp_http_server.start_stop_focus.get() {
                    return self.handle_start_stop(app);
                }
                if app.mcp_http_server.auto_start_focus.get() {
                    return self.handle_auto_start_toggle(app);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let pos = Position::new(mouse.column, mouse.row);
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if self.layout.start_stop_button.contains(pos) {
                app.focus.focus(&app.mcp_http_server.start_stop_focus);
                return self.handle_start_stop(app);
            }
            if self.layout.auto_start_checkbox.contains(pos) {
                app.focus.focus(&app.mcp_http_server.auto_start_focus);
                return self.handle_auto_start_toggle(app);
            }
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let block = th::block(theme, Some("MCP HTTP Server"), app.mcp_http_server.container_focus.get());
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        let sections = Layout::vertical([
            Constraint::Length(1), // Status
            Constraint::Length(3), // Controls
            Constraint::Min(1),    // Details
        ])
        .split(inner);
        self.layout.status_area = sections[0];
        self.layout.controls_area = sections[1];
        self.layout.details_area = sections[2];

        self.render_status(frame, app);
        self.render_controls(frame, app);
        self.render_details(frame, app);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let mut hints = Vec::new();
        if app.mcp_http_server.start_stop_focus.get() {
            hints.push(("Enter/Space", " Start/Stop "));
        }
        if app.mcp_http_server.auto_start_focus.get() {
            hints.push(("Enter/Space", " Toggle auto-start "));
        }
        th::build_hint_spans(&*app.ctx.theme, &hints)
    }
}
