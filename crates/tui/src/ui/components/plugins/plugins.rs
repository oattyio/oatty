//! Top-level Plugins view: orchestrates search, table, add view, details,
//! logs, and env editor. Handles focus routing, shortcuts, and responsive
//! layout whether shown fullscreen or as a centered overlay.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Clear,
};

use crate::app::{Effect, Msg};
use crate::ui::components::component::Component;
use crate::ui::theme::helpers as th;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect as _Rect;

use super::add_plugin::state::{AddTransport, PluginAddViewState};
use super::{
    PluginHintsBar, PluginsAddComponent, PluginsDetailsComponent, PluginsLogsComponent, PluginsSearchComponent,
    PluginsSecretsComponent, PluginsTableComponent,
};

/// Top-level Plugins view component. For M1, renders a minimal panel shell
/// with Nord theme styles and a hint bar.
#[derive(Debug, Default)]
pub struct PluginsComponent<'a> {
    details_open: bool,
    logs_open: bool,
    // Child components
    table_component: PluginsTableComponent,
    search_component: PluginsSearchComponent,
    details_component: PluginsDetailsComponent,
    logs_component: PluginsLogsComponent,
    env_component: PluginsSecretsComponent,
    add_component: PluginsAddComponent,
    footer: PluginHintsBar<'a>,
}

impl PluginsComponent<'_> {
    /// Whether the overlay is visible (delegates to app state).
    #[allow(dead_code)]
    pub fn is_visible(&self, app: &crate::app::App) -> bool {
        app.plugins.is_visible()
    }
}

impl Component for PluginsComponent<'_> {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Vec<Effect> {
        if let Some(effects) = self.delegate_open_overlays_keys(app, key) {
            return effects;
        }

        if self.handle_focus_cycle(app, key) {
            return Vec::new();
        }

        if let Some(effects) = self.handle_ctrl_shortcuts(app, key) {
            return effects;
        }

        self.delegate_child_component_keys(app, key)
    }

    fn update(&mut self, _app: &mut crate::app::App, _msg: &Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let Some(inner_area) = self.render_shell(frame, area, app) else {
            return;
        };
        let layout = self.layout_main(inner_area);
        let header_area = layout.get(0).expect("header area not found");
        let body_area = layout.get(1).expect("body area not found");
        let footer_area = layout.get(2).expect("footer area not found");
        self.render_header(frame, *header_area, app);
        self.render_body(frame, *body_area, app);
        self.footer.render(frame, *footer_area, app);
        self.render_overlays(frame, area, app);
    }
}

/// Create a centered rectangle relative to the given area using percentage.
fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let w = area.width.saturating_mul(percent_x).saturating_div(100);
    let h = area.height.saturating_mul(percent_y).saturating_div(100);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

impl PluginsComponent<'_> {
    /// If an overlay (env, logs, add view) is open, delegate keys to it.
    fn delegate_open_overlays_keys(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Option<Vec<Effect>> {
        // Let global focus manager handle Tab/BackTab across the whole tree
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return None;
        }
        if app.plugins.env.is_some() {
            let effects = self.env_component.handle_key_events(app, key);
            return Some(if effects.is_empty() {
                app.mark_dirty();
                Vec::new()
            } else {
                effects
            });
        }
        if let Some(logs) = &mut app.plugins.logs {
            let effects = self.logs_component.handle_key_events(logs, key);
            return Some(if effects.is_empty() {
                app.mark_dirty();
                Vec::new()
            } else {
                effects
            });
        }
        if app.plugins.add.is_some() {
            let effects = self.add_component.handle_key_events(app, key);
            return Some(if effects.is_empty() {
                app.mark_dirty();
                Vec::new()
            } else {
                effects
            });
        }
        None
    }

    /// Cycle focus using rat_focus when Tab/BackTab.
    /// If an overlay or the add wizard is open, restrict cycling to that context.
    fn handle_focus_cycle(&self, app: &mut crate::app::App, key: KeyEvent) -> bool {
        if !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return false;
        }
        let mut builder = rat_focus::FocusBuilder::new(None);
        if let Some(env) = &app.plugins.env {
            builder.widget(env);
        } else if let Some(logs) = &app.plugins.logs {
            builder.widget(logs);
        } else if let Some(add) = &app.plugins.add {
            // Build a manual ring over Add children to ensure cycling works
            struct Leaf(FocusFlag);
            impl HasFocus for Leaf {
                fn build(&self, b: &mut FocusBuilder) {
                    b.leaf_widget(self);
                }
                fn focus(&self) -> FocusFlag {
                    self.0.clone()
                }
                fn area(&self) -> _Rect {
                    _Rect::default()
                }
            }
            builder.widget(&Leaf(add.f_transport.clone()));
            builder.widget(&Leaf(add.f_name.clone()));
            match add.transport {
                AddTransport::Local => {
                    builder.widget(&Leaf(add.f_command.clone()));
                    builder.widget(&Leaf(add.f_args.clone()));
                }
                AddTransport::Remote => {
                    builder.widget(&Leaf(add.f_base_url.clone()));
                }
            }
            builder.widget(&Leaf(add.f_key_value_pairs.clone()));
            // Buttons: include only enabled
            let name_present = !add.name.trim().is_empty();
            match add.transport {
                AddTransport::Local => {
                    let command_present = !add.command.trim().is_empty();
                    if command_present {
                        builder.widget(&Leaf(add.f_btn_validate.clone()));
                    }
                    if name_present && command_present {
                        builder.widget(&Leaf(add.f_btn_save.clone()));
                    }
                }
                AddTransport::Remote => {
                    let base_url_present = !add.base_url.trim().is_empty();
                    if base_url_present {
                        builder.widget(&Leaf(add.f_btn_validate.clone()));
                    }
                    if name_present && base_url_present {
                        builder.widget(&Leaf(add.f_btn_save.clone()));
                    }
                }
            }
            builder.widget(&Leaf(add.f_btn_cancel.clone()));
        } else {
            builder.widget(&app.plugins);
        }
        let f = builder.build();
        if key.code == KeyCode::Tab {
            let _ = f.next();
        } else {
            let _ = f.prev();
        }
        // When scoping focus to Add, ensure top-level search/grid flags are off
        if app.plugins.add.is_some() {
            app.plugins.search_flag.set(false);
            app.plugins.grid_flag.set(false);
        }
        app.mark_dirty();
        true
    }

    /// Handle top-level Ctrl-based shortcuts and return any effects.
    fn handle_ctrl_shortcuts(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Option<Vec<Effect>> {
        let mut effects: Vec<Effect> = Vec::with_capacity(1);
        let ctrl: bool = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('b') if ctrl => {
                if self.logs_open {
                    self.logs_open = false;
                    app.plugins.close_logs();
                } else if app.plugins.add.is_some() {
                    app.plugins.add = None;
                } else if self.details_open {
                    self.details_open = false;
                } else {
                    app.plugins.set_visible(false);
                }
                app.mark_dirty();
            }
            KeyCode::Enter | KeyCode::Char('d') if ctrl => {
                self.details_open = true;
                app.mark_dirty();
            }
            KeyCode::Char('f') if ctrl => {
                if self.logs_open {
                    if let Some(logs) = &mut app.plugins.logs {
                        logs.search_active = true;
                    }
                } else {
                    app.plugins.search_flag.set(true);
                    app.plugins.grid_flag.set(false);
                }
                app.mark_dirty();
            }
            KeyCode::Char('k') if ctrl => {
                app.plugins.filter.clear();
                app.plugins.selected = Some(0);
                app.mark_dirty();
            }
            KeyCode::Char('s') if ctrl => {
                if let Some(item) = app.plugins.selected_item() {
                    effects.push(Effect::PluginsStart(item.name.clone()));
                }
            }
            KeyCode::Char('t') if ctrl => {
                if let Some(item) = app.plugins.selected_item() {
                    effects.push(Effect::PluginsStop(item.name.clone()));
                }
            }
            KeyCode::Char('r') if ctrl => {
                if let Some(item) = app.plugins.selected_item() {
                    effects.push(Effect::PluginsRestart(item.name.clone()));
                }
            }
            KeyCode::Char('a')
                if ctrl && !self.details_open && app.plugins.env.is_none() && app.plugins.logs.is_none() =>
            {
                app.plugins.add = Some(PluginAddViewState::new());
                // Focus the Add wizard's Name field by default
                if let Some(add) = &app.plugins.add {
                    let mut builder = rat_focus::FocusBuilder::new(None);
                    builder.widget(&app.plugins);
                    let f = builder.build();
                    f.focus(&add.f_name);
                }
                if let Some(add) = &mut app.plugins.add {
                    add.sync_selected_from_focus();
                }
                app.mark_dirty();
            }
            KeyCode::Char('l') if ctrl => {
                if let Some(item) = app.plugins.selected_item() {
                    let name = item.name.clone();
                    app.plugins.open_logs(name.clone());
                    self.logs_open = true;
                    effects.push(Effect::PluginsOpenLogs(name));
                }
            }
            KeyCode::Char('e') if ctrl => {
                if let Some(item) = app.plugins.selected_item() {
                    let name = item.name.clone();
                    app.plugins.open_secrets(name.clone());
                    effects.push(Effect::PluginsOpenSecrets(name));
                }
            }
            KeyCode::Char('v') if ctrl && app.plugins.add.is_some() => effects.push(Effect::PluginsValidateAdd),
            KeyCode::Char('a') if ctrl && app.plugins.add.is_some() => effects.push(Effect::PluginsApplyAdd),
            KeyCode::Char('l') if ctrl && self.logs_open => {
                if let Some(logs) = &mut app.plugins.logs {
                    logs.toggle_follow();
                }
                app.mark_dirty();
            }
            KeyCode::Char('y') if ctrl && self.logs_open => {
                if let Some(logs) = &app.plugins.logs {
                    let last = logs.lines.last().cloned().unwrap_or_default();
                    effects.push(Effect::CopyLogsRequested(last));
                }
            }
            KeyCode::Char('u') if ctrl && self.logs_open => {
                if let Some(logs) = &app.plugins.logs {
                    let body = logs.lines.join("\n");
                    effects.push(Effect::CopyLogsRequested(body));
                }
            }
            KeyCode::Char('o') if ctrl && self.logs_open => {
                if let Some(logs) = &app.plugins.logs {
                    effects.push(Effect::PluginsExportLogsDefault(logs.name.clone()));
                }
            }
            _ => {}
        }
        return if effects.is_empty() { Some(effects) } else { None };
    }

    /// Delegate keys to search or table when those areas are focused.
    fn delegate_child_component_keys(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Backspace | KeyCode::Char(_) if app.plugins.search_flag.get() => {
                let effects = self.search_component.handle_key_events(app, key);
                if !effects.is_empty() {
                    return effects;
                }
                app.mark_dirty();
            }
            KeyCode::Up | KeyCode::Down if app.plugins.grid_flag.get() => {
                let effects = self.table_component.handle_key_events(app, key);
                if !effects.is_empty() {
                    return effects;
                }
                app.mark_dirty();
            }
            _ => {}
        }
        Vec::new()
    }

    /// Render the outer shell depending on fullscreen vs overlay; returns inner area.
    fn render_shell(&self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) -> Option<Rect> {
        let fullscreen = app.plugins_fullscreen;
        if !fullscreen && !app.plugins.is_visible() {
            return None;
        }
        let outer = area;
        let inner = if fullscreen {
            let block = th::block(&*app.ctx.theme, Some("Plugins — MCP"), true);
            frame.render_widget(block.clone(), outer);
            block.inner(outer)
        } else {
            let panel = centered_rect(outer, 90, 80);
            let block = th::block(&*app.ctx.theme, Some("Plugins — MCP"), true);
            frame.render_widget(Clear, panel);
            frame.render_widget(block.clone(), panel);
            block.inner(panel)
        };
        Some(inner)
    }

    /// Compute the main 3-rows layout: header, body, footer.
    fn layout_main(&self, inner: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(6), Constraint::Length(1)])
            .split(inner)
            .to_vec()
    }

    /// Render header area (search bar).
    fn render_header(&mut self, frame: &mut Frame, header_area: Rect, app: &mut crate::app::App) {
        self.search_component.render(frame, header_area, app);
    }

    /// Render body area (table or add view side-by-side depending on width).
    fn render_body(&mut self, frame: &mut Frame, body_area: Rect, app: &mut crate::app::App) {
        let add_open = app.plugins.add.as_ref().map(|w| w.visible).unwrap_or(false);
        if add_open && body_area.width >= 120 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(body_area);
            self.add_component.render(frame, cols[0], app);
            self.table_component.render(frame, cols[1], app);
        } else if add_open {
            self.add_component.render(frame, body_area, app);
        } else {
            self.table_component.render(frame, body_area, app);
        }
    }

    /// Render details/logs/env overlays on top of the shell.
    fn render_overlays(&mut self, frame: &mut Frame, outer: Rect, app: &mut crate::app::App) {
        if self.details_open {
            let details_area = centered_rect(outer, 70, 60);
            frame.render_widget(Clear, details_area);
            self.details_component.render(frame, details_area, app);
        }
        if self.logs_open {
            if let Some(_logs) = &app.plugins.logs {
                let logs_area = centered_rect(outer, 90, 60);
                frame.render_widget(Clear, logs_area);
                self.logs_component.render(frame, logs_area, app);
            } else {
                self.logs_open = false;
            }
        }
        if app.plugins.env.is_some() {
            let env_area = centered_rect(outer, 90, 70);
            frame.render_widget(Clear, env_area);
            self.env_component.render(frame, env_area, app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_component_constructs() {
        let _c = PluginsComponent::default();
        assert!(true);
    }
}
