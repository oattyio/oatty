//! Ring buffer implementation for plugin logs.

use crate::types::{LogError, McpLogEntry};
use std::collections::VecDeque;

/// A ring buffer for storing log entries with a maximum capacity.
#[derive(Debug)]
pub struct LogRingBuffer {
    /// The underlying buffer.
    buffer: VecDeque<McpLogEntry>,

    /// Maximum number of entries to store.
    max_size: usize,

    /// Whether to follow new entries (for real-time display).
    follow_mode: bool,
}

impl LogRingBuffer {
    /// Create a new ring buffer with the specified maximum size.
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
            follow_mode: false,
        }
    }

    /// Add a log entry to the buffer.
    pub fn add_entry(&mut self, entry: McpLogEntry) -> Result<(), LogError> {
        // If the buffer is full, remove the oldest entry
        if self.buffer.len() >= self.max_size {
            self.buffer.pop_front();
        }

        self.buffer.push_back(entry);
        Ok(())
    }

    /// Get the most recent log entries.
    pub fn get_recent(&self, count: usize) -> Vec<McpLogEntry> {
        let start = if count >= self.buffer.len() { 0 } else { self.buffer.len() - count };

        self.buffer.iter().skip(start).cloned().collect()
    }

    /// Get all log entries.
    pub fn get_all(&self) -> Vec<McpLogEntry> {
        self.buffer.iter().cloned().collect()
    }

    /// Get log entries within a time range.
    pub fn get_in_range(&self, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Vec<McpLogEntry> {
        self.buffer
            .iter()
            .filter(|entry| entry.timestamp >= start && entry.timestamp <= end)
            .cloned()
            .collect()
    }

    /// Get log entries by level.
    pub fn get_by_level(&self, level: crate::types::LogLevel) -> Vec<McpLogEntry> {
        self.buffer.iter().filter(|entry| entry.level == level).cloned().collect()
    }

    /// Get log entries by source.
    pub fn get_by_source(&self, source: crate::types::LogSource) -> Vec<McpLogEntry> {
        self.buffer.iter().filter(|entry| entry.source == source).cloned().collect()
    }

    /// Clear all log entries.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get the number of entries in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Check if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.max_size
    }

    /// Set follow mode.
    pub fn set_follow_mode(&mut self, follow: bool) {
        self.follow_mode = follow;
    }

    /// Check if follow mode is enabled.
    pub fn follow_mode(&self) -> bool {
        self.follow_mode
    }

    /// Get the maximum size of the buffer.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Set the maximum size of the buffer.
    pub fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;

        // If the new size is smaller, remove excess entries
        while self.buffer.len() > max_size {
            self.buffer.pop_front();
        }
    }

    /// Get an iterator over all log entries.
    pub fn iter(&self) -> impl Iterator<Item = &McpLogEntry> {
        self.buffer.iter()
    }

    /// Get a reverse iterator over all log entries.
    pub fn iter_rev(&self) -> impl Iterator<Item = &McpLogEntry> {
        self.buffer.iter().rev()
    }
}

impl Default for LogRingBuffer {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Errors that can occur with the log ring buffer.
#[derive(Debug, thiserror::Error)]
pub enum LogBufferError {
    #[error("Buffer is full and cannot accept more entries")]
    BufferFull,

    #[error("Invalid buffer size: {size}")]
    InvalidSize { size: usize },

    #[error("Buffer operation failed: {reason}")]
    OperationFailed { reason: String },
}

#[cfg(test)]
mod tests {
    use crate::types::{LogLevel, LogSource};

    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buffer = LogRingBuffer::new(3);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);

        let entry = McpLogEntry::new(LogLevel::Info, "Test message".to_string(), LogSource::System, "test".to_string());

        buffer.add_entry(entry).unwrap();
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buffer = LogRingBuffer::new(2);

        for i in 0..5 {
            let entry = McpLogEntry::new(LogLevel::Info, format!("Message {}", i), LogSource::System, "test".to_string());
            buffer.add_entry(entry).unwrap();
        }

        assert_eq!(buffer.len(), 2);
        assert!(buffer.is_full());

        let recent = buffer.get_recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "Message 3");
        assert_eq!(recent[1].message, "Message 4");
    }

    #[test]
    fn test_ring_buffer_filtering() {
        let mut buffer = LogRingBuffer::new(10);

        let info_entry = McpLogEntry::new(LogLevel::Info, "Info message".to_string(), LogSource::System, "test".to_string());

        let error_entry = McpLogEntry::new(LogLevel::Error, "Error message".to_string(), LogSource::Stderr, "test".to_string());

        buffer.add_entry(info_entry).unwrap();
        buffer.add_entry(error_entry).unwrap();

        let info_logs = buffer.get_by_level(LogLevel::Info);
        assert_eq!(info_logs.len(), 1);
        assert_eq!(info_logs[0].message, "Info message");

        let error_logs = buffer.get_by_level(LogLevel::Error);
        assert_eq!(error_logs.len(), 1);
        assert_eq!(error_logs[0].message, "Error message");

        let stderr_logs = buffer.get_by_source(LogSource::Stderr);
        assert_eq!(stderr_logs.len(), 1);
        assert_eq!(stderr_logs[0].message, "Error message");
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut buffer = LogRingBuffer::new(10);

        let entry = McpLogEntry::new(LogLevel::Info, "Test message".to_string(), LogSource::System, "test".to_string());

        buffer.add_entry(entry).unwrap();
        assert_eq!(buffer.len(), 1);

        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }
}
