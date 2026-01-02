//! Vertical navigation bar component.
//!
//! This module provides a reusable vertical navigation bar widget composed of
//! icon buttons. It supports:
//! - Arbitrary number of items (N icons)
//! - rat-focus integration via `FocusFlag`s per item
//! - Keyboard navigation (Up/Down/Home/End/Enter)
//! - Theming via `ui::theme::helpers`
//!
//! The component is self-contained and not wired to the broader application
//! layout. Consumers can instantiate it, feed it input events, and render it in
//! any layout slot. To integrate with the app, map selection/activation to an
//! application `Msg`/`Effect` in the caller.
//!
//! # Usage
//!
//! ```ignore
//! use oatty_types::Route;
//! use oatty_tui::ui::components::nav_bar::{
//!     NavItem, NavItemAction, VerticalNavBarComponent, VerticalNavBarState,
//! };
//!
//! let state = VerticalNavBarState::new(vec![
//!     NavItem::new("$", "Command", NavItemAction::Route(Route::Palette)),
//!     NavItem::new("⌕", "Browser", NavItemAction::Route(Route::Browser)),
//!     NavItem::new("{}", "Plugins", NavItemAction::Route(Route::Plugins)),
//! ]);
//! let mut component = VerticalNavBarComponent::new(state);
//! // In your event loop, route key events to component.handle_key_events(...)
//! // In your render pass, call component.render(...)
//! ```

mod nav_bar_component;
mod state;

pub use nav_bar_component::VerticalNavBarComponent;
pub use state::{NavItemAction, VerticalNavBarState};
