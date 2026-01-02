use std::sync::Arc;

use super::components::logs::LogDetailsComponent;
use super::components::nav_bar::VerticalNavBarComponent;
use super::components::plugins::PluginsDetailsComponent;
use super::components::workflows::WorkflowCollectorComponent;
use super::components::{Component, HelpComponent, LogsComponent, TableComponent};
use super::theme::theme_helpers as th;
use super::utils::centered_rect;
use crate::app::App;
use crate::ui::components::common::ConfirmationModal;
use crate::ui::components::palette::PaletteComponent;
use crate::ui::components::theme_picker::ThemePickerComponent;
use crate::ui::components::workflows::{RunViewComponent, WorkflowInputsComponent};
use crate::ui::components::{BrowserComponent, FilePickerModal, FilePickerState, LibraryComponent, PluginsComponent, WorkflowsComponent};
use crate::ui::utils::centered_min_max;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use oatty_types::{Effect, Modal, Msg, Route};
use rat_focus::{FocusBuilder, HasFocus};
use ratatui::widgets::Clear;
use ratatui::{
    prelude::*,
    style::Style,
    widgets::{Block, Paragraph},
};

pub struct ModalLayout(Box<dyn Fn(Rect) -> Rect>);

impl std::fmt::Debug for ModalLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ModalLayout")
    }
}

type ModalView = (Box<dyn Component>, ModalLayout);

#[derive(Default, Debug)]
pub struct MainView {
    /// Current main view component
    pub content_view: Option<Box<dyn Component>>,
    /// Main view for the nav bar
    pub nav_bar_view: VerticalNavBarComponent,
    /// Currently open modal component
    pub modal_view: Option<ModalView>,
    /// Currently open logs view component
    pub logs_view: LogsComponent,

    /// the widget_id of the focus just before a modal is opened
    transient_focus_id: Option<usize>,
}

impl MainView {
    pub fn new(content_view: Option<Box<dyn Component>>) -> Self {
        Self {
            content_view,
            modal_view: None,
            nav_bar_view: VerticalNavBarComponent::new(),
            logs_view: LogsComponent,
            transient_focus_id: None,
        }
    }
    /// Updates the current route of the application and performs necessary state transitions.
    /// Note that this method is not intended to be called directly. Instead, use Effect::SwitchTo.
    ///
    /// # Arguments
    /// * `route` - A `Route` enum variant representing the new route to be set.
    ///
    /// # Behavior
    /// 1. Based on the provided `Route`, determines the corresponding components and their states.
    /// 2. For specific routes:
    ///     * **`Route::WorkflowInputs`**: Attempts to open workflow inputs and log any errors encountered.
    ///     * **`Route::WorkflowRun`**: Validates run view state is available; falls back to the workflow list if missing.
    ///     * **`Route::Workflows`**: Ensures workflows are loaded via the registry and logs any errors encountered.
    /// 3. Updates the navigation bar to reflect the new route.
    /// 4. Changes the main view to the component corresponding to the new route.
    /// 5. Updates the focus behavior using a `FocusBuilder` and sets the focus to the respective state.
    ///
    /// # Errors
    /// - Logs errors related to loading workflows or opening workflow inputs if the operations fail.
    ///
    /// # Side Effects
    /// - Updates internal state fields:
    ///   * `current_route` - Tracks the currently active route.
    ///   * `main_view` - Holds the new route's component as a boxed trait object.
    ///   * `focus` - Responsible for managing the focus and is updated dynamically based on the route.
    ///
    /// # Example
    /// ```rust
    /// let mut app = MyApp::new();
    /// app.set_current_route(Route::Palette);
    /// ```
    pub fn set_current_route(&mut self, app: &mut App, route: Route) {
        if matches!(route, Route::WorkflowRun) && app.workflows.run_view_state().is_none() {
            app.append_log_message("Workflow run view unavailable; falling back to workflow list.");
            return self.set_current_route(app, Route::Workflows);
        }

        let (view, state): (Box<dyn Component>, Box<&dyn HasFocus>) = match route {
            Route::Browser => (Box::new(BrowserComponent::default()), Box::new(&app.browser)),
            Route::Palette => (Box::new(PaletteComponent::default()), Box::new(&app.palette)),
            Route::Plugins => (Box::new(PluginsComponent::default()), Box::new(&app.plugins)),
            Route::WorkflowInputs => (Box::new(WorkflowInputsComponent::default()), Box::new(&app.workflows)),
            Route::Workflows => (Box::new(WorkflowsComponent::default()), Box::new(&app.workflows)),
            Route::WorkflowRun => (Box::new(RunViewComponent::default()), Box::new(&app.workflows)),
            Route::Library => (
                Box::new(LibraryComponent::new(Arc::clone(&app.ctx.command_registry))),
                Box::new(&app.library),
            ),
        };

        app.current_route = app.nav_bar.set_route(route);
        self.content_view = Some(view);

        app.focus = FocusBuilder::build_for(app);
        app.focus.focus(*state);
    }

    /// Update the open modal kind (use None to clear).
    pub fn set_open_modal_kind(&mut self, app: &mut App, modal: Option<Modal>) {
        if let Some(modal_kind) = modal.as_ref() {
            let modal_view: ModalView = match modal_kind {
                Modal::Help => (
                    Box::new(HelpComponent::default()),
                    ModalLayout(Box::new(|rect| centered_rect(80, 70, rect))),
                ),
                Modal::Results(exec_outcome) => {
                    let mut table = TableComponent::default();
                    table.handle_message(app, Msg::ExecCompleted(exec_outcome.clone()));
                    (Box::new(table), ModalLayout(Box::new(|rect| centered_rect(96, 90, rect))))
                }
                Modal::LogDetails => (
                    Box::new(LogDetailsComponent::default()),
                    ModalLayout(Box::new(|rect| centered_rect(80, 70, rect))),
                ),
                Modal::PluginDetails => (
                    Box::new(PluginsDetailsComponent),
                    ModalLayout(Box::new(|rect| centered_rect(90, 80, rect))),
                ),
                Modal::ThemePicker => {
                    app.theme_picker.set_active_theme(&app.ctx.active_theme_id);
                    (
                        Box::new(ThemePickerComponent),
                        ModalLayout(Box::new(|rect| centered_rect(70, 80, rect))),
                    )
                }
                Modal::WorkflowCollector => {
                    let component: Box<dyn Component> = Box::new(WorkflowCollectorComponent::default());
                    let layout = if app.workflows.manual_entry_state().is_some() {
                        ModalLayout(Box::new(|rect| centered_rect(45, 35, rect)))
                    } else {
                        ModalLayout(Box::new(|rect| centered_rect(96, 90, rect)))
                    };
                    (component, layout)
                }
                Modal::Confirmation => (
                    Box::new(ConfirmationModal::default()),
                    ModalLayout(Box::new(|rect| {
                        centered_min_max(45, 35, Rect::new(0, 0, 80, 10), Rect::new(0, 0, 160, 16), rect)
                    })),
                ),
                Modal::FilePicker(extensions) => {
                    let state = FilePickerState::new(extensions.to_owned());
                    app.file_picker = Some(state);
                    (
                        Box::new(FilePickerModal::default()),
                        ModalLayout(Box::new(|rect| {
                            centered_min_max(75, 95, Rect::new(0, 0, 80, 15), Rect::new(0, 0, 160, 150), rect)
                        })),
                    )
                }
            };
            self.modal_view = Some(modal_view);
            // save the current focus to restore when the modal is closed
            self.transient_focus_id = app.focus.focused().map(|focus| focus.widget_id());
        } else {
            self.modal_view = None;
        }
        app.open_modal_kind = modal;
    }

    pub fn restore_focus(&mut self, app: &mut App) {
        if let Some(id) = self.transient_focus_id
            && app.open_modal_kind.is_none()
        {
            app.focus.by_widget_id(id);
            self.transient_focus_id = None;
        } else {
            app.focus.first();
        }
    }
}

impl Component for MainView {
    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        let mut effects = app.update(&msg);
        if let Msg::ExecCompleted(outcome) = &msg {
            app.logs.process_general_execution_result(&outcome)
        }

        // Since messages are consumed, the recipient is assumed to be
        // the first component that is not None. If multiple components
        // require messages, cloning is required to avoid borrowing issues
        // but may lead to performance issues.
        match () {
            _ if self.modal_view.is_some() => {
                effects.append(&mut self.modal_view.as_mut().map(|c| c.0.handle_message(app, msg)).unwrap_or_default());
            }
            _ => {
                if self.content_view.is_some() {
                    effects.append(&mut self.content_view.as_mut().map(|c| c.handle_message(app, msg)).unwrap_or_default());
                }
            }
        };

        effects
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if let Some(target) = self.modal_view.as_mut() {
            return target.0.handle_key_events(app, key);
        }

        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            app.logs.toggle_visible();
            return Vec::new();
        }

        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if app.ctx.theme_picker_available {
                return vec![Effect::ShowModal(Modal::ThemePicker)];
            }
            return Vec::new();
        }

        if app.nav_bar.container_focus.get() {
            return self.nav_bar_view.handle_key_events(app, key);
        }

        if app.logs.is_visible && app.logs.container_focus.get() {
            return self.logs_view.handle_key_events(app, key);
        }

        if let Some(content) = self.content_view.as_mut() {
            return content.handle_key_events(app, key);
        }

        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some(target) = self.modal_view.as_mut() {
            return target.0.handle_mouse_events(app, mouse);
        }

        effects.extend(self.nav_bar_view.handle_mouse_events(app, mouse));
        effects.extend(
            self.content_view
                .as_mut()
                .map(|c| c.handle_mouse_events(app, mouse))
                .unwrap_or_default(),
        );
        effects.extend(self.logs_view.handle_mouse_events(app, mouse));

        effects
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        // Fill the entire background with the theme's background color for consistency
        let bg_fill = Paragraph::new("").style(Style::default().bg(app.ctx.theme.roles().background));
        frame.render_widget(bg_fill, area);

        let layout = self.get_preferred_layout(app, area);
        // Handle main view rendering
        if let Some(current) = self.content_view.as_mut() {
            // Render an active view on the right
            current.render(frame, layout[2], app);
            // Render nav bar on the left
            self.nav_bar_view.render(frame, layout[0], app);
        }

        if app.logs.is_visible {
            let logs_area = layout[3];
            self.logs_view.render(frame, logs_area, app);
        }

        let hint_spans: Vec<Span> = self.get_hint_spans(app);
        let hints_widget = Paragraph::new(Line::from(hint_spans)).style(app.ctx.theme.text_muted_style());
        frame.render_widget(hints_widget, layout[1]);

        if let Some((modal, position)) = self.modal_view.as_mut() {
            render_overlay(frame, app);
            let modal_area = position.0(area);
            frame.render_widget(Clear, modal_area);

            let modal_hints = modal.get_hint_spans(app);
            if !modal_hints.is_empty() {
                let splits = Layout::vertical([
                    Constraint::Percentage(100), // Modal width
                    Constraint::Length(1),       // Modal hints bar
                ])
                .split(modal_area);
                let hints_widget = Paragraph::new(Line::from(modal_hints))
                    .style(app.ctx.theme.text_muted_style())
                    .bg(app.ctx.theme.roles().background);
                frame.render_widget(hints_widget, splits[1]);
                modal.render(frame, splits[0], app);
            } else {
                modal.render(frame, modal_area, app);
            }
        }
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let mut hint_spans: Vec<Span> = vec![Span::styled("Hints: ", app.ctx.theme.text_muted_style())];

        if app.nav_bar.container_focus.get() {
            hint_spans.extend(self.nav_bar_view.get_hint_spans(app));
            return hint_spans;
        }

        if app.logs.is_visible && app.logs.container_focus.get() {
            hint_spans.extend(self.logs_view.get_hint_spans(app));
            return hint_spans;
        }

        if let Some(content) = self.content_view.as_ref() {
            hint_spans.extend(content.get_hint_spans(app));
        }

        hint_spans.extend(th::build_hint_spans(
            &*app.ctx.theme,
            &[(" Ctrl+L", " Toggle logs "), ("Ctrl+T", " Theme picker ")],
        ));

        hint_spans
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        // a wider area displays 2 columns with the leftmost
        // column split into 2 rows totaling 3 rendering areas.
        let outer_areas = Layout::horizontal([
            Constraint::Length(9), // Nav bar width
            Constraint::Min(1),    // Wrapper
        ])
        .split(area);
        // Split the wrapper area into 2 areas for the main view
        // and hints stacked vertically.
        let content_areas = Layout::vertical([
            Constraint::Percentage(100), // Main view width
            Constraint::Min(1),          // Hints area
        ])
        .split(outer_areas[1]);

        let main_view_areas = if content_areas[0].width >= 141 {
            let constraints = if app.logs.is_visible {
                [
                    Constraint::Percentage(75), // Main view
                    Constraint::Fill(1),        // Logs
                ]
            } else {
                [
                    Constraint::Percentage(100), // Main view
                    Constraint::Length(0),       // No logs shown
                ]
            };

            Layout::horizontal(constraints).split(content_areas[0])
        } else {
            // Smaller screens display 3 stacked rows.
            let constraints = if app.logs.is_visible {
                [
                    Constraint::Percentage(80), // Command palette area (+ suggestions)
                    Constraint::Fill(1),        // logs / output content
                ]
            } else {
                [
                    Constraint::Percentage(100), // Command palette area (+ suggestions)
                    Constraint::Length(0),       // logs / output content
                ]
            };

            Layout::vertical(constraints).split(content_areas[0])
        };

        vec![
            outer_areas[0],     // navigation
            content_areas[1],   // Hints bar
            main_view_areas[0], // Main view
            main_view_areas[1], // Logs / output content (if open)
        ]
    }
}

/// Renders modal overlays based on the application state.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state
fn render_overlay(frame: &mut Frame, app: &mut App) {
    frame.render_widget(Block::default().style(app.ctx.theme.modal_background_style()).dim(), frame.area());
}
