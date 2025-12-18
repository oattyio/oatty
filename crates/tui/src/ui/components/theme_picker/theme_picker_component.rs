use crate::app::App;
use crate::ui::components::Component;
use crate::ui::theme::catalog::ThemeDefinition;
use crate::ui::theme::theme_helpers as th;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oatty_types::{Effect, Modal};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};

/// Theme picker modal controller.
#[derive(Debug, Default)]
pub struct ThemePickerComponent;

impl ThemePickerComponent {
    fn apply_selection(&self, app: &mut App) {
        if let Some(option) = app.theme_picker.selected_option() {
            app.apply_theme_selection(option.id);
        }
    }

    fn build_option_lines<'a>(
        definition: &ThemeDefinition,
        app: &App,
        is_selected: bool,
        theme: &dyn crate::ui::theme::Theme,
    ) -> ListItem<'a> {
        let mut line_spans: Vec<Span> = Vec::new();
        let prefix = if is_selected { "> " } else { "  " };
        line_spans.push(Span::styled(prefix.to_owned(), theme.text_secondary_style()));

        let mut label_style = theme.text_primary_style();
        if is_selected {
            label_style = label_style.add_modifier(Modifier::BOLD);
        }
        line_spans.push(Span::styled(format!("{:<25}", definition.label.to_string()), label_style));
        let mut padding = 25;
        if definition.is_high_contrast {
            line_spans.push(Span::styled(format!("{:<8}", "[HC]"), theme.text_secondary_style()));
            padding -= 8;
        }
        if definition.is_ansi_fallback {
            line_spans.push(Span::styled(format!("{:<8}", "[ANSI]"), theme.text_secondary_style()));
            padding -= 8;
        }
        if definition.id.eq_ignore_ascii_case(&app.ctx.active_theme_id) {
            line_spans.push(Span::styled(format!("{:<9}", "● Active"), theme.status_success()));
            padding -= 9;
        }

        line_spans.push(Span::raw(format!("{:>padding$}", " ")));
        line_spans.extend(Self::swatch_spans(definition));

        ListItem::new(Line::from(line_spans))
    }

    fn swatch_spans(definition: &ThemeDefinition) -> Vec<Span<'static>> {
        let colors = [
            ("   ".to_string(), Style::default().bg(definition.swatch.background)),
            ("   ".to_string(), Style::default().bg(definition.swatch.accent)),
            ("   ".to_string(), Style::default().bg(definition.swatch.selection)),
        ];
        let mut spans: Vec<Span> = Vec::with_capacity(colors.len());
        for (text, style) in colors {
            spans.push(Span::styled(text, style));
            spans.push(Span::raw(" "));
        }
        spans
    }
}

impl Component for ThemePickerComponent {
    fn handle_message(&mut self, app: &mut App, msg: &oatty_types::Msg) -> Vec<Effect> {
        match (msg, app.open_modal_kind.as_ref()) {
            (oatty_types::Msg::Resize(_, _), Some(Modal::ThemePicker)) => Vec::new(),
            _ => Vec::new(),
        }
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => return vec![Effect::CloseModal],
            KeyCode::Enter => {
                self.apply_selection(app);
                return vec![Effect::CloseModal];
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.theme_picker.select_previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.theme_picker.select_next();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return vec![Effect::CloseModal];
            }
            _ => {}
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let block = th::block(theme, Some("Theme Picker"), true);
        frame.render_widget(block.clone(), area);
        let inner = block.inner(area);

        let segments = Layout::vertical([Constraint::Length(4), Constraint::Min(5)]).split(inner);

        let intro_lines = vec![
            Line::from(Span::styled(
                "Select a palette to immediately update the interface.",
                theme.text_secondary_style(),
            )),
            Line::from(Span::styled(
                "Press Enter to apply, Esc to close. Selection persists to ~/.config/oatty/preferences.json.",
                theme.text_muted_style(),
            )),
        ];
        frame.render_widget(Paragraph::new(intro_lines).wrap(Wrap { trim: true }), segments[0]);

        let options: Vec<ListItem> = app
            .theme_picker
            .options()
            .iter()
            .enumerate()
            .map(|(idx, definition)| Self::build_option_lines(definition, app, idx == app.theme_picker.selected_index, theme))
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(app.theme_picker.selected_index));
        let list = List::new(options)
            .highlight_style(Style::default())
            .highlight_symbol("")
            .block(Block::default());
        frame.render_stateful_widget(list, segments[1], &mut list_state);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        th::build_hint_spans(
            &*app.ctx.theme,
            &[(" ↑/↓", " Navigate  "), (" Enter", " Apply  "), (" Esc", " Close ")],
        )
    }
}
