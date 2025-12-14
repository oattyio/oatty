//! rmcp-backed MCP client subsystem facade.
//!
//! This module exposes a focused, public surface for managing MCP plugins via `rmcp`.
//! Implementation is split across submodules for clarity and maintainability.

mod core;
mod gateway;
mod health;
mod http;
mod stdio;

pub use core::McpClient;
pub use gateway::{ClientGatewayError, ClientGatewayEvent, McpClientGateway};
pub use health::HealthCheckResult;
