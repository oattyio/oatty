//! Async runtime helpers for blocking callers.
//!
//! This module provides a single entry point for executing async futures from
//! synchronous code paths, reusing the current Tokio runtime when available.

use anyhow::anyhow;
use std::future::Future;
use tokio::{runtime::Handle, task};

/// Execute an async future from synchronous code.
///
/// # Arguments
/// - `future`: The future to run to completion.
///
/// # Returns
/// Returns the future's output or an error if a Tokio runtime cannot be created.
///
/// # Notes
/// - Reuses the current runtime when available.
/// - Falls back to a single-threaded runtime for call sites outside Tokio.
pub fn block_on_future<F, T>(future: F) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    if let Ok(handle) = Handle::try_current() {
        task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| anyhow!(error))?
            .block_on(future)
    }
}
