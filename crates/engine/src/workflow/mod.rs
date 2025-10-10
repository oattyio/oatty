//! Workflow-specific runtime helpers.
//!
//! This module groups the pieces that interpret declarative workflow definitions at runtime.
//! Work Unit 3 focuses on dependent provider resolution, execution orchestration, and
//! telemetry hooks. The submodules introduced here will gradually grow to cover those
//! responsibilities without bloating the core executor or resolver modules.

pub mod bindings;
pub mod document;
pub mod runtime;
pub mod state;
