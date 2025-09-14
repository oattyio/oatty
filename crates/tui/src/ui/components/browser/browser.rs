//! Command browser component for interactive command discovery.
//!
//! This module renders a modal for browsing Heroku commands with a search bar,
//! a command list, and an inline help panel. Selecting a command updates the
//! help content. Press Enter to send the command to the palette without
//! executing it.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::*,
};
use ratatui::style::Modifier;

use crate::{
    app,
    ui::{
        components::{browser::layout::BrowserLayout, component::Component, HelpComponent},
        theme::helpers as th,
    },
};

#[derive(Debug, Default)]
pub struct BrowserComponent;

impl BrowserComponent {
    fn handle_global_shortcuts(&self, key: KeyEvent) -> Option<app::Msg> {
        match key.code {
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::ToggleBuilder),
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::ToggleHelp),
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::ToggleTable),
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::CopyCommand),
            _ => None,
        }
    }

    fn handle_search_keys(&self, app: &mut app::App, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                app.browser.search_input_push(c);
            }
            KeyCode::Backspace => app.browser.search_input_pop(),
            KeyCode::Esc => app.browser.search_input_clear(),
            KeyCode::Tab | KeyCode::BackTab => {
                let focus_ring = app.browser.focus_ring();
                if key.code == KeyCode::Tab { focus_ring.next(); } else { focus_ring.prev(); };
            }
            KeyCode::Down => app.browser.move_selection(1),
            KeyCode::Up => app.browser.move_selection(-1),
            KeyCode::Enter => {
                app.browser.apply_enter();
                app.browser.commands_flag.set(true);
                app.browser.search_flag.set(false);
                app.browser.inputs_flag.set(false);
            }
            _ => {}
        }
    }

    fn handle_commands_keys(&self, app: &mut app::App, key: KeyEvent) {
        match key.code {
            KeyCode::Down => app.browser.move_selection(1),
            KeyCode::Up => app.browser.move_selection(-1),
            KeyCode::Enter => {
                app.browser.apply_enter();
                app.browser.commands_flag.set(true);
                app.browser.search_flag.set(false);
                app.browser.inputs_flag.set(false);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let f = app.browser.focus_ring();
                if key.code == KeyCode::Tab { let _ = f.next(); } else { let _ = f.prev(); }
            }
            _ => {}
        }
    }
}

impl Component for BrowserComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        use crate::ui::utils::centered_rect;

        let area = centered_rect(96, 90, rect);
        self.render_modal_frame(f, area, app);

        let inner = self.create_modal_layout(area, app);
        let chunks = BrowserLayout::vertical_layout(inner);

        self.render_search_panel(f, app, chunks[0]);

        let main = self.create_main_layout(chunks[1]);
        self.render_commands_panel(f, app, main[0]);
        self.render_inline_help_panel(f, app, main[1]);

        self.render_footer(f, app, chunks[2]);
    }

    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<app::Effect> {
        let mut effects: Vec<app::Effect> = Vec::new();
        if let Some(effect) = self.handle_global_shortcuts(key) { effects.extend(app.update(effect)); return effects; }
        if app.browser.search_flag.get() {
            self.handle_search_keys(app, key);
        } else if app.browser.commands_flag.get() {
            self.handle_commands_keys(app, key);
        }
        effects
    }
}

impl BrowserComponent {
    fn render_modal_frame(&self, f: &mut Frame, area: Rect, app: &app::App) {
        let block = th::block(&*app.ctx.theme, Some("Command Browser  [Esc] Close"), true);
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);
    }

    fn create_modal_layout(&self, area: Rect, app: &app::App) -> Rect {
        let block = th::block(&*app.ctx.theme, Some("Command Browser  [Esc] Close"), true);
        block.inner(area)
    }

    fn create_main_layout(&self, area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Commands
                Constraint::Percentage(70), // Inline Help
            ])
            .split(area)
            .to_vec()
    }

    fn render_footer(&self, frame: &mut Frame, app: &app::App, area: Rect) {
        let theme = &*app.ctx.theme;
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", theme.text_muted_style()),
            Span::styled("Ctrl+F", theme.accent_emphasis_style()),
            Span::styled(" close  ", theme.text_muted_style()),
            Span::styled("Enter", theme.accent_emphasis_style()),
            Span::styled(" send to palette  ", theme.text_muted_style()),
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" cancel", theme.text_muted_style()),
        ])).style(theme.text_muted_style());
        frame.render_widget(footer, area);
    }

    fn render_search_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = self.create_search_title(app);
        let focused = app.browser.search_flag.get();
        let mut block = th::block(&*app.ctx.theme, None, focused);
        block = block.title(title);
        let inner = block.inner(area);
        let p = Paragraph::new(app.browser.search_input().as_str())
            .style(app.ctx.theme.text_primary_style())
            .block(block);
        f.render_widget(p, area);
        self.set_search_cursor(f, app, inner);
    }

    fn create_search_title(&self, app: &app::App) -> Line<'_> {
        if app.ctx.debug_enabled {
            let t = &*app.ctx.theme;
            Line::from(vec![
                Span::styled("Search Commands", t.text_secondary_style().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled("[DEBUG]", t.accent_emphasis_style()),
            ])
        } else {
            let t = &*app.ctx.theme;
            Line::from(Span::styled("Search Commands", t.text_secondary_style().add_modifier(Modifier::BOLD)))
        }
    }

    fn set_search_cursor(&self, f: &mut Frame, app: &app::App, inner: Rect) {
        if app.browser.search_flag.get() {
            let x = inner.x.saturating_add(app.browser.search_input().chars().count() as u16);
            let y = inner.y;
            f.set_cursor_position((x, y));
        }
    }

    fn render_commands_panel(&self, frame: &mut Frame, app: &mut app::App, area: Rect) {
        let title = format!("Commands ({})", app.browser.filtered().len());
        let focused = app.browser.commands_flag.get();
        let block = th::block(&*app.ctx.theme, Some(&title), focused);
        let items: Vec<ListItem> = app
            .browser
            .filtered()
            .iter()
            .map(|idx| {
                let all = app.browser.all_commands();
                let group = &all[*idx].group;
                let name = &all[*idx].name;
                let display = if name.is_empty() { group.to_string() } else { format!("{} {}", group, name) };
                ListItem::new(display).style(app.ctx.theme.text_primary_style())
            })
            .collect();
        let list = List::new(items)
            .block(block)
            .highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");
        let list_state = &mut app.browser.list_state();
        frame.render_stateful_widget(list, area, list_state);
    }

    fn render_inline_help_panel(&self, frame: &mut Frame, app: &mut app::App, area: Rect) {
        let mut title = "Help".to_string();
        let mut text = ratatui::text::Text::from(Line::from(Span::styled(
            "Select a command to view detailed help.",
            app.ctx.theme.text_secondary_style().add_modifier(Modifier::BOLD),
        )));
        if let Some(spec) = app.browser.selected_command() {
            let mut split = spec.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let cmd = if rest.is_empty() { group.to_string() } else { format!("{} {}", group, rest) };
            title = format!("Help — {}", cmd);
            text = HelpComponent::build_command_help(&*app.ctx.theme, spec);
        }
        let block = th::block(&*app.ctx.theme, Some(&title), false);
        let inner = block.inner(area);
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        frame.render_widget(block, area);
        let p = Paragraph::new(text).style(app.ctx.theme.text_primary_style()).wrap(Wrap { trim: false });
        frame.render_widget(p, splits[0]);
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", app.ctx.theme.text_muted_style()),
            Span::styled("Ctrl+Y", app.ctx.theme.accent_emphasis_style()),
            Span::styled(" copy", app.ctx.theme.text_muted_style()),
        ])).style(app.ctx.theme.text_muted_style());
        frame.render_widget(footer, splits[1]);
    }
}

