//! Manual entry widget encapsulating typed value editing for workflow inputs.
//!
//! This module exposes both the state container and the component responsible for
//! handling keyboard/mouse input plus rendering. The collector owns an instance
//! and delegates to it whenever the manual entry modal is open.

pub mod component;
pub mod state;

pub use component::ManualEntryComponent;
#[allow(unused_imports)]
pub use state::{ManualEntryEnumOption, ManualEntryEnumState, ManualEntryFocus, ManualEntryKind, ManualEntryLayoutState, ManualEntryState};
