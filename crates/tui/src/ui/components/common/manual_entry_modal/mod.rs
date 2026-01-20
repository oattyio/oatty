//! Manual entry widget encapsulating typed value editing for workflow inputs.
//!
//! This module exposes both the state container and the component responsible for
//! handling keyboard/mouse input plus rendering. The collector owns an instance
//! and delegates to it whenever the manual entry modal is open.

pub mod default_manual_entry_component;
mod manual_entry_view;
pub mod state;

pub use default_manual_entry_component::DefaultManualEntryComponent;
pub use manual_entry_view::ManualEntryView;
