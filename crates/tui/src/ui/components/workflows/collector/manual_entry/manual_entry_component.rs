use crate::app::App;
use crate::ui::components::Component;
use crate::ui::components::common::ManualEntryView;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::Effect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

/// Handles rendering and interaction for the manual entry modal.
#[derive(Debug, Default)]
pub struct ManualEntryComponent {
    view: ManualEntryView,
}

impl Component for ManualEntryComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc {
            app.workflows.manual_entry = None;
            return vec![Effect::CloseModal];
        }

        let Some(state) = app.workflows.manual_entry_state_mut() else {
            return Vec::new();
        };

        match self.view.handle_key_events(state, key) {
            Ok(Some(candidate)) => {
                let input_name = app.workflows.active_input_name();
                if let Some(run_state_rc) = app.workflows.active_run_state.clone()
                    && let Some(name) = input_name
                {
                    let mut run_state = run_state_rc.borrow_mut();
                    run_state.run_context_mut().inputs.insert(name, candidate);
                    let _ = run_state.evaluate_input_providers();
                }

                app.workflows.manual_entry = None;
                vec![Effect::CloseModal]
            }
            Err(err) => {
                state.set_error(format!("{}", err));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let Some(state) = app.workflows.manual_entry_state_mut() else {
            return Vec::new();
        };
        self.view.handle_mouse_events(state, mouse)
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        if app.workflows.manual_entry.is_none() {
            return;
        }

        let Some(state) = app.workflows.manual_entry_state_mut() else {
            return;
        };
        self.view.render_with_state(frame, rect, &*app.ctx.theme, state);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let Some(state) = app.workflows.manual_entry_state() else {
            return Vec::new();
        };
        self.view.get_hint_spans(&*app.ctx.theme, state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use indexmap::IndexMap;
    use oatty_engine::WorkflowRunState;
    use oatty_mcp::{PluginEngine, config::McpConfig};
    use oatty_registry::CommandRegistry;
    use oatty_types::workflow::{RuntimeWorkflow, WorkflowInputDefinition};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn build_app() -> App<'static> {
        let registry = Arc::new(Mutex::new(CommandRegistry::default()));
        let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::clone(&registry)).expect("engine"));
        App::new(registry, engine)
    }

    fn run_state_with_single_input() -> WorkflowRunState {
        let mut inputs: IndexMap<String, WorkflowInputDefinition> = IndexMap::new();
        inputs.insert("username".into(), WorkflowInputDefinition::default());
        let workflow = RuntimeWorkflow {
            identifier: "sample".into(),
            title: None,
            description: None,
            inputs,
            steps: Vec::new(),
            requires: None,
        };
        WorkflowRunState::new(workflow)
    }

    fn prepare_manual_entry(app: &mut App<'_>) {
        app.workflows.begin_inputs_session(run_state_with_single_input());
        {
            let view = app.workflows.input_view_state_mut().expect("input view available");
            view.build_input_rows();
            view.input_list_state.select(Some(0));
        }
        app.workflows.open_manual_for_active_input().expect("manual entry opens");
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn esc_cancels_manual_entry_modal() {
        let mut app = build_app();
        prepare_manual_entry(&mut app);
        assert!(app.workflows.manual_entry_state().is_some());

        let mut component = ManualEntryComponent::default();
        let effects = component.handle_key_events(&mut app, key_event(KeyCode::Esc));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::CloseModal));
        assert!(app.workflows.manual_entry_state().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn enter_submits_value_and_updates_run_state() {
        let mut app = build_app();
        prepare_manual_entry(&mut app);
        if let Some(buffer) = app
            .workflows
            .manual_entry_state_mut()
            .expect("manual entry state present")
            .value
            .text_buffer_mut()
        {
            buffer.set_input("demo-user");
            buffer.set_cursor("demo-user".len());
        }

        let mut component = ManualEntryComponent::default();
        let effects = component.handle_key_events(&mut app, key_event(KeyCode::Enter));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::CloseModal));
        assert!(app.workflows.manual_entry_state().is_none());

        let run_state_rc = app.workflows.active_run_state.clone().expect("run state");
        let run_state = run_state_rc.borrow();
        assert_eq!(run_state.run_context.inputs.get("username"), Some(&json!("demo-user")));
    }
}
