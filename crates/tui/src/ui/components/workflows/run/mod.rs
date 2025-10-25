//! Workflow run view module.
//!
//! This module houses the state and component responsible for rendering the
//! workflow execution view. The run view presents step progress, surfaced
//! outputs, and footer controls while reusing shared table rendering utilities.

pub mod run;
pub mod state;

pub use run::RunViewComponent;
pub use state::{RunViewState, StepFinishedData, WorkflowRunControlHandle};
