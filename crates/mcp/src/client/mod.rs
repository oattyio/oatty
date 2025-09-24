//! rmcp-backed MCP client subsystem facade.
//!
//! This module exposes a focused, public surface for managing MCP plugins via `rmcp`.
//! Implementation is split across submodules for clarity and maintainability.

mod client;
mod health;
mod http;
mod manager;
mod stdio;

pub use client::McpClient;
pub use health::HealthCheckResult;
pub use manager::{ClientManagerError, ClientManagerEvent, McpClientManager};
