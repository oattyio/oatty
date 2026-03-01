//! MCP HTTP server control view.

use std::borrow::Cow;

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

#[derive(Debug, Clone)]
struct McpClientConfigSnippet {
    title: &'static str,
    content: String,
}

#[derive(Debug, Default)]
pub struct McpHttpServerLayout {
    pub start_stop_button: Rect,
    pub auto_start_checkbox: Rect,
    pub status_area: Rect,
    pub controls_area: Rect,
    pub message_area: Rect,
    pub details_area: Rect,
    pub config_area: Rect,
    pub config_copy_areas: Vec<(usize, Rect)>,
}

impl From<Vec<Rect>> for McpHttpServerLayout {
    fn from(value: Vec<Rect>) -> Self {
        Self {
            status_area: value[0],
            controls_area: value[1],
            details_area: value[2],
            start_stop_button: value[3],
            auto_start_checkbox: value[4],
            message_area: value[5],
            config_area: value[6],
            config_copy_areas: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct McpHttpServerComponent {
    layout: McpHttpServerLayout,
}

impl McpHttpServerComponent {
    fn resolve_mcp_endpoint_url(state: &McpHttpServerState) -> String {
        let address = if matches!(state.status, McpHttpServerStatus::Running) {
            state.bound_address.as_deref().unwrap_or(&state.configured_bind_address)
        } else {
            &state.configured_bind_address
        };
        format!("http://{address}/mcp")
    }

    fn build_client_config_snippets(app: &App) -> Vec<McpClientConfigSnippet> {
        let endpoint_url = Self::resolve_mcp_endpoint_url(&app.mcp_http_server);
        vec![
            McpClientConfigSnippet {
                title: "Shared connection settings",
                content: format!("URL: {endpoint_url}"),
            },
            McpClientConfigSnippet {
                title: "Codex",
                content: format!("[mcp_servers.oatty]\nurl = \"{endpoint_url}\""),
            },
            McpClientConfigSnippet {
                title: "Claude Desktop (claude_desktop_config.json)",
                content: format!(
                    "{{\n  \"mcpServers\": {{\n    \"oatty\": {{\n      \"command\": \"npx\",\n      \"args\": [\n        \"mcp-remote\",\n        \"{endpoint_url}\"\n      ]\n    }}\n  }}\n}}"
                ),
            },
            McpClientConfigSnippet {
                title: "Cursor (.cursor/mcp.json)",
                content: format!("{{\n  \"mcpServers\": {{\n    \"oatty\": {{\n      \"url\": \"{endpoint_url}\"\n    }}\n  }}\n}}"),
            },
            McpClientConfigSnippet {
                title: "Cline / Roo Code (mcp_settings.json)",
                content: format!("{{\n  \"mcpServers\": {{\n    \"oatty\": {{\n      \"url\": \"{endpoint_url}\"\n    }}\n  }}\n}}"),
            },
            McpClientConfigSnippet {
                title: "VS Code MCP config (.vscode/mcp.json)",
                content: format!(
                    "{{\n  \"servers\": {{\n    \"oatty\": {{\n      \"type\": \"http\",\n      \"url\": \"{endpoint_url}\"\n    }}\n  }}\n}}"
                ),
            },
            McpClientConfigSnippet {
                title: "Generic Streamable HTTP MCP client",
                content: format!(
                    "{{\n  \"servers\": {{\n    \"oatty\": {{\n      \"transport\": \"streamable-http\",\n      \"url\": \"{endpoint_url}\"\n    }}\n  }}\n}}"
                ),
            },
        ]
    }

    fn config_item_height(snippet: &McpClientConfigSnippet) -> u16 {
        snippet.content.lines().count() as u16 + 4
    }

    fn config_item_line_ranges(snippets: &[McpClientConfigSnippet]) -> Vec<(usize, u16, u16)> {
        let mut ranges = Vec::with_capacity(snippets.len());
        let mut start = 0u16;
        for (index, snippet) in snippets.iter().enumerate() {
            let height = Self::config_item_height(snippet);
            let end = start.saturating_add(height.saturating_sub(1));
            ranges.push((index, start, end));
            start = end.saturating_add(1);
        }
        ranges
    }

    fn config_total_content_height(snippets: &[McpClientConfigSnippet]) -> u16 {
        Self::config_item_line_ranges(snippets)
            .last()
            .map(|(_, _, end)| end.saturating_add(1))
            .unwrap_or(0)
    }

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

    fn config_count(app: &App) -> usize {
        Self::build_client_config_snippets(app).len()
    }

    fn select_next_config(app: &mut App) {
        let count = Self::config_count(app);
        if count == 0 {
            return;
        }
        let next_index = (app.mcp_http_server.selected_config_index + 1).min(count - 1);
        app.mcp_http_server.set_selected_config_index(next_index);
    }

    fn select_previous_config(app: &mut App) {
        if Self::config_count(app) == 0 {
            return;
        }
        let previous_index = app.mcp_http_server.selected_config_index.saturating_sub(1);
        app.mcp_http_server.set_selected_config_index(previous_index);
    }

    fn scroll_config_list_down(app: &mut App) {
        app.mcp_http_server.scroll_config_lines(1);
    }

    fn scroll_config_list_up(app: &mut App) {
        app.mcp_http_server.scroll_config_lines(-1);
    }

    fn ensure_selected_config_visible(app: &mut App) {
        let snippets = Self::build_client_config_snippets(app);
        let selected_index = app.mcp_http_server.selected_config_index;
        let maybe_selected_range = Self::config_item_line_ranges(&snippets)
            .into_iter()
            .find(|(index, _, _)| *index == selected_index)
            .map(|(_, start, end)| (start, end));
        let Some((selected_start, selected_end)) = maybe_selected_range else {
            return;
        };

        let viewport_height = app.mcp_http_server.config_viewport_height() as usize;
        let current_offset = app.mcp_http_server.config_scroll_offset() as usize;
        if viewport_height == 0 {
            app.mcp_http_server.set_config_scroll_offset(selected_start);
            return;
        }

        if usize::from(selected_start) < current_offset {
            app.mcp_http_server.set_config_scroll_offset(selected_start);
            return;
        }

        let visible_end = current_offset.saturating_add(viewport_height.saturating_sub(1));
        if usize::from(selected_end) > visible_end {
            let desired_offset = usize::from(selected_end).saturating_sub(viewport_height.saturating_sub(1)) as u16;
            app.mcp_http_server.set_config_scroll_offset(desired_offset);
        }
    }

    fn handle_config_copy(&self, app: &mut App, config_index: usize) -> Vec<Effect> {
        let snippets = Self::build_client_config_snippets(app);
        let Some(snippet) = snippets.get(config_index) else {
            return Vec::new();
        };

        app.mcp_http_server.set_selected_config_index(config_index);
        app.mcp_http_server
            .set_success_message(Cow::Owned(format!("Copied {} config", snippet.title)));
        vec![Effect::CopyToClipboardRequested(snippet.content.clone())]
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

    fn render_message(&mut self, frame: &mut Frame, app: &mut App) {
        let theme = &*app.ctx.theme;
        let Some(message) = app.mcp_http_server.message_ref() else {
            return;
        };
        if message.is_expired() {
            app.mcp_http_server.set_message(None);
            return;
        }

        if let Some(message_paragraph) = th::create_status_paragraph(theme, message, self.layout.message_area.width, false) {
            frame.render_widget(message_paragraph, self.layout.message_area);
        }
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

    fn render_config_list(&mut self, frame: &mut Frame, app: &mut App) {
        let theme = &*app.ctx.theme;
        let is_focused = app.mcp_http_server.config_list_focus.get();
        let block = th::block(theme, Some("Client Config Snippets"), is_focused);
        let inner = block.inner(self.layout.config_area);
        frame.render_widget(block, self.layout.config_area);

        self.layout.config_copy_areas.clear();

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let snippets = Self::build_client_config_snippets(app);
        let item_ranges = Self::config_item_line_ranges(&snippets);
        let total_content_height = Self::config_total_content_height(&snippets);
        app.mcp_http_server.update_config_viewport_height(inner.height);
        app.mcp_http_server.update_config_content_height(total_content_height);

        let selected_index = app.mcp_http_server.selected_config_index;
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(total_content_height as usize);
        for (config_index, snippet) in snippets.iter().enumerate() {
            let title_style = if config_index == selected_index {
                theme.status_success()
            } else {
                theme.text_primary_style()
            };
            lines.push(Line::from(Span::styled(snippet.title, title_style)));
            lines.push(Line::from(""));
            for content_line in snippet.content.lines() {
                lines.push(Line::from(Span::styled(content_line.to_string(), theme.syntax_string_style())));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("─".repeat(inner.width as usize), theme.text_muted_style())));
        }

        let scroll_offset = app.mcp_http_server.config_scroll_offset();
        let paragraph = Paragraph::new(lines).scroll((scroll_offset, 0));
        frame.render_widget(paragraph, inner);

        for (config_index, start_line, _) in item_ranges {
            if start_line < scroll_offset {
                continue;
            }
            let relative_y = start_line.saturating_sub(scroll_offset);
            if relative_y >= inner.height {
                continue;
            }
            let copy_area = Rect::new(inner.x + inner.width.saturating_sub(7), inner.y + relative_y, 6, 1);
            let copy_style = if config_index == app.mcp_http_server.selected_config_index {
                theme.status_success()
            } else {
                theme.text_muted_style()
            };
            frame.render_widget(Paragraph::new(Span::styled("⧉ copy", copy_style)), copy_area);
            self.layout.config_copy_areas.push((config_index, copy_area));
        }
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
                if app.mcp_http_server.config_list_focus.get() {
                    return self.handle_config_copy(app, app.mcp_http_server.selected_config_index);
                }
            }
            KeyCode::Down => {
                if app.mcp_http_server.config_list_focus.get() {
                    Self::select_next_config(app);
                    Self::ensure_selected_config_visible(app);
                }
            }
            KeyCode::Up => {
                if app.mcp_http_server.config_list_focus.get() {
                    Self::select_previous_config(app);
                    Self::ensure_selected_config_visible(app);
                }
            }
            KeyCode::PageDown => {
                if app.mcp_http_server.config_list_focus.get() {
                    app.mcp_http_server.scroll_config_pages(1);
                }
            }
            KeyCode::PageUp => {
                if app.mcp_http_server.config_list_focus.get() {
                    app.mcp_http_server.scroll_config_pages(-1);
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
            if let Some((config_index, _)) = self.layout.config_copy_areas.iter().find(|(_, area)| area.contains(pos)) {
                app.focus.focus(&app.mcp_http_server.config_list_focus);
                return self.handle_config_copy(app, *config_index);
            }
            if self.layout.config_area.contains(pos) {
                let relative_row = pos.y.saturating_sub(self.layout.config_area.y + 1);
                let global_row = app.mcp_http_server.config_scroll_offset().saturating_add(relative_row);
                let snippets = Self::build_client_config_snippets(app);
                if let Some((config_index, _, _)) = Self::config_item_line_ranges(&snippets)
                    .into_iter()
                    .find(|(_, start, end)| global_row >= *start && global_row <= *end)
                {
                    app.focus.focus(&app.mcp_http_server.config_list_focus);
                    app.mcp_http_server.set_selected_config_index(config_index);
                    Self::ensure_selected_config_visible(app);
                }
            }
        }

        if self.layout.config_area.contains(pos) {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    app.focus.focus(&app.mcp_http_server.config_list_focus);
                    Self::scroll_config_list_down(app);
                }
                MouseEventKind::ScrollUp => {
                    app.focus.focus(&app.mcp_http_server.config_list_focus);
                    Self::scroll_config_list_up(app);
                }
                _ => {}
            }
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let block = th::block(theme, Some("MCP HTTP Server"), app.mcp_http_server.container_focus.get());
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        self.layout = McpHttpServerLayout::from(self.get_preferred_layout(app, inner));

        self.render_status(frame, app);
        self.render_controls(frame, app);
        self.render_message(frame, app);
        self.render_details(frame, app);
        self.render_config_list(frame, app);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let mut hints = Vec::new();
        if app.mcp_http_server.start_stop_focus.get() {
            hints.push(("Enter/Space", " Start/Stop "));
        }
        if app.mcp_http_server.auto_start_focus.get() {
            hints.push(("Enter/Space", " Toggle auto-start "));
        }
        if app.mcp_http_server.config_list_focus.get() {
            hints.push(("↑/↓", " Select config "));
            hints.push(("PgUp/PgDn", " Scroll configs "));
            hints.push(("Enter/Space", " Copy selected config "));
        }
        th::build_hint_spans(&*app.ctx.theme, &hints)
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        let cols = Layout::horizontal([
            Constraint::Percentage(40), // Left
            Constraint::Percentage(60), // Right
        ])
        .split(area);

        let sections = Layout::vertical([
            Constraint::Length(1), // Status
            Constraint::Length(3), // Controls
            Constraint::Min(1),    // Details
        ])
        .split(cols[0]);

        let controls = Layout::horizontal([
            Constraint::Length(12), // Start/Stop button
            Constraint::Length(2),  // Spacer
            Constraint::Length(20), // Auto-start checkbox
        ])
        .split(sections[1]);

        let config_layout = Layout::vertical([
            Constraint::Length(1), // Message above the config list
            Constraint::Length(1), // Spacer
            Constraint::Min(1),    // Config list
        ])
        .split(cols[1]);

        vec![
            sections[0],      // status
            sections[1],      // controls
            sections[2],      // details
            controls[0],      // Start/Stop button
            controls[2],      // Auto-start Checkbox
            config_layout[0], // Message above the config list
            config_layout[2], // Config list
        ]
    }
}
