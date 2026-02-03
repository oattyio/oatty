//! Workflow run view module.
//!
//! This module houses the state and component responsible for rendering the
//! workflow execution view. The run view presents step progress, surfaced
//! outputs, and footer controls while reusing shared results rendering utilities.

mod run_component;
pub mod state;

pub use run_component::RunViewComponent;
pub use state::{RunViewState, StepFinishedData, WorkflowRunControlHandle};
