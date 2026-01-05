use oatty_util::line_clamp;
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Wrap},
};

use crate::{
    app::App,
    ui::components::{Component, common::key_value_editor::KeyValueEditorView},
};

#[derive(Default, Debug)]
pub struct DetailsEditorComponent {
    kv_view: KeyValueEditorView,
}

impl Component for DetailsEditorComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let Some(projection) = app.library.selected_projection() else {
            frame.render_widget(Paragraph::new("Select an item to configure"), rect);
            return;
        };
        let description = line_clamp(projection.description.as_ref(), 3, rect.width.saturating_sub(2) as usize);
        let theme = &*app.ctx.theme;
        let (title_style, enabled_text) = if projection.is_enabled {
            (theme.status_success(), "enabled")
        } else {
            (theme.text_muted_style(), "disabled")
        };
        let summary_lines = vec![
            Line::from(vec![
                Span::styled(projection.title.clone(), title_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({})", enabled_text), theme.text_muted_style()),
            ]),
            Line::from(vec![
                Span::styled("Command Prefix: ", theme.text_primary_style()),
                Span::styled(projection.vendor.clone(), theme.syntax_type_style()),
            ]),
            Line::from(vec![
                Span::styled("Endpoints: ", theme.text_primary_style()),
                Span::styled(projection.command_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled("Workflows: ", theme.text_primary_style()),
                Span::styled(projection.workflow_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled("Value providers: ", theme.text_primary_style()),
                Span::styled(projection.provider_contract_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(description, theme.syntax_type_style())]),
            Line::from(""),
            Line::from(vec![
                Span::styled("HTTP headers ", theme.text_primary_style()),
                Span::styled("(requests for these endpoints will ", theme.text_primary_style()),
            ]),
        ];
        let block = Block::new().padding(Padding::horizontal(1));
        let inner = block.inner(rect);
        let summary = Paragraph::new(summary_lines).wrap(Wrap { trim: true }).block(block);
        let line_ct = summary.line_count(inner.width) as u16;
        frame.render_widget(summary, rect);

        let mut kv_area = rect.clone();
        kv_area.y = rect.y + line_ct;
        kv_area.height = rect.height.saturating_sub(line_ct);

        let kv_state = app.library.kv_state_mut();
        self.kv_view.render_with_state(frame, kv_area, theme, kv_state);
    }
}

impl DetailsEditorComponent {}
