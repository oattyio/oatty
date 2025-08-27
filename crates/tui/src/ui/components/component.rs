//! Component system for the Heroku TUI application.
//!
//! This module defines the Component trait and related abstractions that enable
//! modular UI development. Components are self-contained UI elements that handle
//! their own state, events, and rendering while integrating with the main
//! application through a consistent interface.

use anyhow::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;

use crate::app::{Effect, Msg};
use ratatui::Frame;

/// A trait representing a UI component with its own state and behavior.
///
/// Components handle localized events, update their internal state, and render
/// themselves into a provided `Rect`, reporting any side effects back to the
/// application via `Effect`s and `Msg`s.
///
/// # Design Principles
///
/// - **Separation of concerns**: Components own only local UI behavior and state
/// - **Single responsibility**: Each component handles one specific area (e.g., palette, builder, help)
/// - **Consistent patterns**: All components expose `init`, event handlers, `update`, and `render`
/// - **Event-driven**: Components respond to application messages and user input
/// - **Side-effect reporting**: Components report effects rather than directly modifying global state
///
/// # Component Lifecycle
///
/// 1. **Initialization**: `init()` is called once when the component is created
/// 2. **Event Handling**: Components receive events through `handle_events()`, `handle_key_events()`, etc.
/// 3. **State Updates**: `update()` processes application messages and updates internal state
/// 4. **Rendering**: `render()` draws the component into the provided frame area
///
/// # Example Implementation
///
/// ```rust
/// use heroku_tui::component::Component;
/// use ratatui::{Frame, layout::Rect};
/// use crossterm::event::KeyEvent;
/// use anyhow::Result;
///
/// #[derive(Default)]
/// struct MyComponent {
///     internal_state: String,
/// }
///
/// impl Component for MyComponent {
///     fn init(&mut self) -> Result<()> {
///         self.internal_state = "initialized".to_string();
///         Ok(())
///     }
///
///     fn handle_key_events(
///         &mut self,
///         app: &mut heroku_tui::app::App,
///         key: KeyEvent,
///     ) -> Vec<heroku_tui::app::Effect> {
///         // Handle key events specific to this component
///         vec![]
///     }
///
///     fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut heroku_tui::app::App) {
///         // Draw the component's UI elements
///         use ratatui::widgets::Paragraph;
///         let widget = Paragraph::new(&self.internal_state);
///         frame.render_widget(widget, rect);
///     }
/// }
/// ```
pub(crate) trait Component {
    /// Initialize any internal state.
    ///
    /// This method is called once when the component is created, allowing
    /// it to set up any internal state, load resources, or perform other
    /// initialization tasks.
    ///
    /// # Returns
    ///
    /// `Result<()>` indicating success or failure of initialization
    ///
    /// # Examples
    ///
    /// ```rust
    /// fn init(&mut self) -> Result<()> {
    ///     self.load_configuration()?;
    ///     self.initialize_widgets()?;
    ///     Ok(())
    /// }
    /// ```
    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    /// Handle a generic application-level message the component cares about.
    ///
    /// This method allows components to respond to application-wide messages
    /// that may affect their state or behavior. Components should only handle
    /// messages that are relevant to their functionality.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state
    /// * `msg` - The application message to handle
    ///
    /// # Returns
    ///
    /// Vector of effects that the application should process
    ///
    /// # Examples
    ///
    /// ```rust
    /// fn handle_events(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
    ///     match msg {
    ///         Msg::ToggleHelp => {
    ///             self.show_help = !self.show_help;
    ///             vec![]
    ///         }
    ///         _ => vec![]
    ///     }
    /// }
    /// ```
    #[allow(dead_code)]
    fn handle_events(&mut self, _app: &mut crate::app::App, _msg: &Msg) -> Vec<Effect> {
        Vec::new()
    }

    /// Handle key events when this component has focus.
    ///
    /// This method processes keyboard input specific to the component.
    /// Components should only handle keys that are meaningful to their
    /// functionality and return `true` if the key was consumed.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state
    /// * `key` - The key event to handle
    ///
    /// # Returns
    ///
    /// Vector of effects that the application should process
    ///
    /// # Examples
    ///
    /// ```rust
    /// fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
    ///     match key.code {
    ///         KeyCode::Char('q') => {
    ///             self.close();
    ///             vec![Effect::CloseModal]
    ///         }
    ///         _ => vec![]
    ///     }
    /// }
    /// ```
    #[allow(dead_code)]
    fn handle_key_events(&mut self, _app: &mut crate::app::App, _key: KeyEvent) -> Vec<Effect> {
        Vec::new()
    }

    /// Handle mouse events when this component has focus.
    ///
    /// This method processes mouse input specific to the component.
    /// Components should handle clicks, scrolls, and other mouse interactions
    /// that are relevant to their functionality.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state
    /// * `mouse` - The mouse event to handle
    ///
    /// # Returns
    ///
    /// Vector of effects that the application should process
    ///
    /// # Examples
    ///
    /// ```rust
    /// fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
    ///     match mouse.kind {
    ///         MouseEventKind::Down(MouseButton::Left) => {
    ///             self.handle_click(mouse.column, mouse.row);
    ///             vec![]
    ///         }
    ///         _ => vec![]
    ///     }
    /// }
    /// ```
    #[allow(dead_code)]
    fn handle_mouse_events(&mut self, _app: &mut crate::app::App, _mouse: MouseEvent) -> Vec<Effect> {
        Vec::new()
    }

    /// Update internal state based on an application message.
    ///
    /// This method allows components to update their internal state in response
    /// to application messages. Unlike `handle_events`, this method is called
    /// for all messages and is intended for state synchronization rather than
    /// event handling.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state
    /// * `msg` - The application message to process
    ///
    /// # Returns
    ///
    /// Vector of effects that the application should process
    ///
    /// # Examples
    ///
    /// ```rust
    /// fn update(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
    ///     match msg {
    ///         Msg::DataChanged => {
    ///             self.refresh_data(app);
    ///             vec![]
    ///         }
    ///         _ => vec![]
    ///     }
    /// }
    /// ```
    #[allow(dead_code)]
    fn update(&mut self, _app: &mut crate::app::App, _msg: &Msg) -> Vec<Effect> {
        Vec::new()
    }

    /// Render the component into the given area.
    ///
    /// This method is responsible for drawing the component's UI elements
    /// into the provided frame area. Implementations should be side-effect
    /// free except for frame drawing and cursor placement. Any state changes
    /// should happen in `update` or event handlers.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `rect` - The rectangular area allocated for this component
    /// * `app` - The application state (read-only during rendering)
    ///
    /// # Examples
    ///
    /// ```rust
    /// fn render(&mut self, frame: &mut Frame, rect: Rect, app: &App) {
    ///     use ratatui::widgets::{Block, Borders, Paragraph};
    ///     
    ///     let block = Block::default()
    ///         .title("My Component")
    ///         .borders(Borders::ALL);
    ///     
    ///     let widget = Paragraph::new(&self.content)
    ///         .block(block);
    ///     
    ///     frame.render_widget(widget, rect);
    /// }
    /// ```
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut crate::app::App);
}
