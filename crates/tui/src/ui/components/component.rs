//! Component system for the Heroku TUI application.
//!
//! This module defines the Component trait and related abstractions that enable
//! modular UI development. Components are self-contained UI elements that
//! handle their own state, events, and rendering while integrating with the
//! main application through a consistent interface.

use crate::app::App;
use crossterm::event::{KeyEvent, MouseEvent};
use heroku_types::{Effect, Msg};
use ratatui::layout::Position;
use ratatui::{Frame, layout::Rect, text::Span};
use std::fmt::Debug;

/// A trait representing a UI component with its own state and behavior.
///
/// Components handle localized events, update their internal state, and render
/// themselves into a provided `Rect`, reporting any side effects back to the
/// application via `Effect`s and `Msg`s.
///
/// # Design Principles
///
/// - **Separation of concerns**: Components own only local UI behavior and
///   state
/// - **Single responsibility**: Each component handles one specific area (e.g.,
///   palette, browser, help)
/// - **Consistent patterns**: All components expose `init`, event handlers,
///   `update`, and `render`
/// - **Event-driven**: Components respond to application messages and user
///   input
/// - **Side-effect reporting**: Components report effects rather than directly
///   modifying global state
///
/// # Component Lifecycle
///
/// 1. **Initialization**: `init()` is called once when the component is created
/// 2. **Event Handling**: Components receive events through `handle_events()`,
///    `handle_key_events()`, etc.
/// 3. **State Updates**: `update()` processes application messages and updates
///    internal state
/// 4. **Rendering**: `render()` draws the component into the provided frame
///    area
///
/// # Example Implementation
///
/// ```rust,ignore
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
pub trait Component: Debug {
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
    #[allow(dead_code)]
    fn handle_events(&mut self, _app: &mut App, _msg: &Msg) -> Vec<Effect> {
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
    /// ```rust,ignore
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
    fn handle_key_events(&mut self, _app: &mut App, _key: KeyEvent) -> Vec<Effect> {
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
    /// ```rust,ignore
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
    fn handle_mouse_events(&mut self, _app: &mut App, _mouse: MouseEvent) -> Vec<Effect> {
        Vec::new()
    }

    /// Update the internal state based on an application message.
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
    /// ```rust,ignore
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
    fn update(&mut self, _app: &mut App, _msg: &Msg) -> Vec<Effect> {
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
    /// ```rust,ignore
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
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App);

    /// Optionally render the hints bar into the given area.
    ///
    /// This method is responsible for drawing the component's hints
    /// into the provided frame area. Implementations should be side-effect
    /// free. Any state changes should happen in `update` or event handlers.
    ///
    /// If the component contains children, the child's render_hints_bar
    /// should be called by the parent only when the child has focus or
    /// when it is expected to received key events. This prevents a
    /// scenario where the child specifies key combinations in it's hints
    /// but is unable to act on them.
    ///
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `rect` - The rectangular area allocated for this component
    /// * `app` - The application state (read-only during rendering)
    /// * `is_root` - Indicates if this should be rendered as a root hints bar
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn render_hints_bar(&mut self, frame: &mut Frame, rect: Rect, app: &App) {
    /// }
    /// ```
    fn get_hint_spans(&self, _app: &App, _is_root: bool) -> Vec<Span<'_>> {
        Vec::new()
    }

    /// Determines and returns the preferred layout for a given area within the application.
    ///
    /// # Arguments
    /// - `&self`: A reference to the current instance of the object.
    /// - `_app`: A reference to the application (`App`) that this method relates to.
    /// - `area`: A `Rect` structure defining the spatial area within which the layout is to be determined.
    ///
    /// # Returns
    /// - `Self::NamedRegions`: The defined layout regions associated with `Self`, if implemented.
    ///
    /// # Panics
    /// This method currently invokes the `unimplemented!()` macro, meaning it has not been implemented yet.
    /// Any attempt to call this method will result in a runtime panic.
    ///
    /// # Notes
    /// - Ensure this method is implemented before usage to provide the desired functionality.
    /// - This is likely to be a placeholder stub for future layout logic in a UI or rendering system.
    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        vec![area]
    }
}
/// Finds the index of the button from a list of rectangular target areas that contains the given mouse coordinates.
///
/// This function checks if the mouse's x and y coordinates are within the bounds of any rectangular `target`
/// in the `targets` vector. If there is a match, it returns the index of the first matching rectangle. If no
/// target contains the mouse coordinates or if the input `rect` has zero width or height, the function returns `None`.
///
/// # Parameters
/// - `rect`: A reference to a `Rect` object. This serves as a validation check to ensure it has a positive width and height.
/// - `targets`: A vector of `Rect` objects, representing the clickable target areas that can be checked against the mouse coordinates.
/// - `mouse_x`: The x-coordinate of the mouse cursor.
/// - `mouse_y`: The y-coordinate of the mouse cursor.
///
/// # Returns
/// - `Option<usize>`: The index of the first rectangle in `targets` that contains the mouse coordinates, or `None` if no rectangle matches.
///
/// # Examples
/// ```
/// // Assuming `Rect` is defined with fields `x`, `y`, `width`, and `height`.
/// let rect = Rect { x: 0, y: 0, width: 50, height: 50 };
/// let targets = vec![
///     Rect { x: 10, y: 10, width: 30, height: 30 },
///     Rect { x: 50, y: 50, width: 20, height: 20 },
/// ];
/// let mouse_x = 15;
/// let mouse_y = 15;
///
/// let index = find_target_index_by_mouse_position(&rect, &targets, mouse_x, mouse_y);
/// assert_eq!(index, Some(0)); // The mouse is within the first target rectangle.
/// ```
///
/// # Note
/// - If the input `rect` has a width or height of zero, the function immediately returns `None` without performing any checks.
/// - The function uses inclusive bounds for `mouse_x` and `mouse_y` when checking the rectangle edges. This means that
/// if the mouse is exactly on the left or top edges of a rectangle, it will still match.
pub fn find_target_index_by_mouse_position(rect: &Rect, targets: &Vec<Rect>, mouse_x: u16, mouse_y: u16) -> Option<usize> {
    let pos = Position::new(mouse_x, mouse_y);
    if !rect.contains(pos.clone()) {
        return None;
    }
    if let Some((idx, _)) = targets.iter().enumerate().find(|(_, r)| r.contains(pos.clone())) {
        return Some(idx);
    }
    None
}
