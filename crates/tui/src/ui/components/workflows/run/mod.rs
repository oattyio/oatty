//! Workflow run view module.
//!
//! This module houses the state and component responsible for rendering the
//! workflow execution view. The run view presents step progress, surfaced
//! outputs, and footer controls while reusing shared table rendering utilities.

pub mod component;
pub mod state;

pub use component::RunViewComponent;
pub use state::RunViewState;
