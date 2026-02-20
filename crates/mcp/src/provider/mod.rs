//! MCP provider integration for the engine.

mod adapter;
mod mcp_provider;
mod registry;

pub use adapter::{AdapterError, McpProviderAdapter};
pub use mcp_provider::McpProvider;
pub use registry::{McpProviderError, McpProviderOps, McpProviderRegistry};
