//! Execution engine facade.
//!
//! This module intentionally exposes the executor surface only. Implementation
//! details live in focused sibling modules (`prepare`, `planning`, `step_once`,
//! `repeat`, and `execute_plan`).

mod execute_plan;
mod planning;
mod prepare;
mod repeat;
pub mod runner;
mod step_once;
mod types;

pub use execute_plan::{execute_workflow, execute_workflow_with_runner};
pub use planning::order_steps_for_execution;
pub use prepare::{collect_unresolved_step_templates, prepare_step};
pub(crate) use repeat::{run_step_repeating_with, run_step_repeating_with_observer};
pub use runner::{CommandRunner, NoopRunner, RegistryCommandRunner};
pub use step_once::run_step_with;
pub use types::{PreparedStep, StepResult, StepStatus};
