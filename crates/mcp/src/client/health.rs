//! Health-related types for MCP clients.

/// Minimal health check payload consumed by the TUI.
///
/// Captures whether the client is healthy, optional latency,
/// and a short error message if unhealthy.
#[derive(Debug, Clone, Default)]
pub struct HealthCheckResult {
    /// Whether the client is considered healthy (connected/running).
    pub healthy: bool,
    /// Optional handshake latency in milliseconds.
    pub latency_ms: Option<u64>,
    /// Optional error context if unhealthy.
    pub error: Option<String>,
}
