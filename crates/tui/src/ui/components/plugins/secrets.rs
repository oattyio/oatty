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

use super::state::PluginSecretsEditorState;

/// Component for rendering the plugin environment editor overlay.
#[derive(Debug, Default)]
pub struct PluginsSecretsComponent;

impl PluginsSecretsComponent {
    fn move_up(&mut self, env: &mut PluginSecretsEditorState) {
        if env.selected > 0 {
            env.selected -= 1;
        }
    }

    fn move_down(&mut self, env: &mut PluginSecretsEditorState) {
        if env.selected + 1 < env.rows.len() {
            env.selected += 1;
        }
    }

    fn toggle_or_commit(&mut self, env: &mut PluginSecretsEditorState) {
        if env.editing {
            if let Some(row) = env.rows.get_mut(env.selected) {
                row.value = env.input.clone();
            }
            env.input.clear();
            env.editing = false;
        } else if let Some(row) = env.rows.get(env.selected) {
            env.input = row.value.clone();
            env.editing = true;
        }
    }

    fn save(&mut self, env: &PluginSecretsEditorState) -> Vec<Effect> {
        let name = env.name.clone();
        let rows = env.rows.iter().map(|r| (r.key.clone(), r.value.clone())).collect();
        vec![Effect::PluginsSaveEnv { name, rows }]
    }

    fn cancel_edit(&mut self, env: &mut PluginSecretsEditorState) {
        if env.editing {
            env.input.clear();
            env.editing = false;
        }
    }
}

impl Component for PluginsSecretsComponent {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Vec<Effect> {
        let env = app
            .plugins
            .env
            .as_mut()
            .expect("key event on uninitialized PluginsEnvComponent");
        match key.code {
            KeyCode::Up => {
                self.move_up(env);
                Vec::new()
            }
            KeyCode::Down => {
                self.move_down(env);
                Vec::new()
            }
            KeyCode::Enter => {
                self.toggle_or_commit(env);
                Vec::new()
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self.save(env),
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cancel_edit(env);
                Vec::new()
            }
            KeyCode::Backspace if env.editing => {
                env.input.pop();
                Vec::new()
            }
            KeyCode::Char(c) if env.editing && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                env.input.push(c);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        if let Some(env) = &app.plugins.env {
            let theme = &*app.ctx.theme;
            self.render_env_editor(frame, area, theme, env);
        }
    }
}

impl PluginsSecretsComponent {
    /// Render the environment editor table and title.
    fn render_env_editor(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, env: &PluginSecretsEditorState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .title(Span::styled(
                format!("Edit Env — {}  [Enter] edit  [Ctrl-s] save  [b] back", env.name),
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
        let _c = PluginsSecretsComponent::default();
        assert!(true);
    }
}
