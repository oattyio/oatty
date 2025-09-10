//! Plugins environment editor component for editing plugin environment variables.
//!
//! Renders a two-column table of KEY and VALUE, supports inline editing of the
//! selected row, and saving the updated environment via an effect. Secret values
//! are redacted during rendering.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Row, Table},
};

use crate::app::Effect;
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::state::PluginEnvEditorState;

/// Component for rendering the plugin environment editor overlay.
#[derive(Debug, Default)]
pub struct PluginsEnvComponent;

impl PluginsEnvComponent {
    /// Handle key events specific to the environment editor.
    pub fn handle_key_events(&self, env: &mut PluginEnvEditorState, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Up => { Self::move_up(env); Vec::new() }
            KeyCode::Down => { Self::move_down(env); Vec::new() }
            KeyCode::Enter => { Self::toggle_or_commit(env); Vec::new() }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => Self::save(env),
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => { Self::cancel_edit(env); Vec::new() }
            KeyCode::Backspace if env.editing => { env.input.pop(); Vec::new() }
            KeyCode::Char(c) if env.editing && !key.modifiers.contains(KeyModifiers::CONTROL) => { env.input.push(c); Vec::new() }
            _ => Vec::new(),
        }
    }

    fn move_up(env: &mut PluginEnvEditorState) {
        if env.selected > 0 { env.selected -= 1; }
    }

    fn move_down(env: &mut PluginEnvEditorState) {
        if env.selected + 1 < env.rows.len() { env.selected += 1; }
    }

    fn toggle_or_commit(env: &mut PluginEnvEditorState) {
        if env.editing {
            if let Some(row) = env.rows.get_mut(env.selected) { row.value = env.input.clone(); }
            env.input.clear(); env.editing = false;
        } else if let Some(row) = env.rows.get(env.selected) {
            env.input = row.value.clone(); env.editing = true;
        }
    }

    fn save(env: &PluginEnvEditorState) -> Vec<Effect> {
        let name = env.name.clone();
        let rows = env.rows.iter().map(|r| (r.key.clone(), r.value.clone())).collect();
        vec![Effect::PluginsSaveEnv { name, rows }]
    }

    fn cancel_edit(env: &mut PluginEnvEditorState) { if env.editing { env.input.clear(); env.editing = false; } }
}

impl Component for PluginsEnvComponent {
    fn handle_key_events(&mut self, _app: &mut crate::app::App, _key: KeyEvent) -> Vec<Effect> {
        Vec::new()
    }

    fn update(&mut self, _app: &mut crate::app::App, _msg: &crate::app::Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        if let Some(env) = &app.plugins.env {
            let theme = &*app.ctx.theme;
            self.render_env_editor(frame, area, theme, env);
        }
    }
}

impl PluginsEnvComponent {
    /// Render the environment editor table and title.
    fn render_env_editor(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, env: &PluginEnvEditorState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .title(Span::styled(
                format!("Edit Env — {}  [Enter] edit  [Ctrl-S] save  [b] back", env.name),
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(block.clone(), area);
        let inner = block.inner(area);

        // Build rows: mask secrets with bullets
        let rows = env.rows.iter().enumerate().map(|(i, r)| {
            let val = if r.is_secret {
                "••••••••••••••••".to_string()
            } else {
                r.value.clone()
            };
            let name = if i == env.selected {
                format!("› {}", r.key)
            } else {
                r.key.clone()
            };
            Row::new(vec![name, val]).style(theme.text_primary_style())
        });

        let table = Table::new(rows, [Constraint::Percentage(30), Constraint::Percentage(70)])
            .header(
                Row::new(vec![
                    Span::styled("KEY", th::table_header_style(theme)),
                    Span::styled("VALUE", th::table_header_style(theme)),
                ])
                .style(th::table_header_row_style(theme)),
            )
            .block(Block::default().style(th::panel_style(theme)));
        frame.render_widget(table, inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_env_component_constructs() {
        let _c = PluginsEnvComponent::default();
        assert!(true);
    }
}
