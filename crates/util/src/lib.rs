//! # Heroku Utility Library
//!
//! This crate provides utility functions for the Heroku CLI, including HTTP utilities,
//! text processing, fuzzy matching, shell-like lexing, and date handling.
//!
//! ## Modules
//!
//! - **HTTP Utilities**: Functions for parsing HTTP headers, building requests, and handling responses
//! - **Text Processing**: Sensitive data redaction and fuzzy string matching
//! - **Shell Lexing**: Tokenization of shell-like input with position tracking
//! - **Date Handling**: Date field detection and formatting utilities
//!
//! ## Usage
//!
//! ```rust
//! use heroku_util::{redact_sensitive, fuzzy_score, lex_shell_like};
//!
//! // Redact sensitive information
//! let redacted = redact_sensitive("API_KEY=abc123");
//!
//! // Fuzzy string matching
//! let score = fuzzy_score("applications", "app");
//!
//! // Shell-like tokenization
//! let tokens = lex_shell_like("cmd --flag 'value'");
//! ```

// Internal modules
pub mod date_handling;
pub mod http;
pub mod http_exec;
pub mod shell_lexing;
pub mod text_processing;

// Re-export commonly used items for convenience
pub use date_handling::*;
pub use http::*;
pub use http_exec::*;
pub use shell_lexing::*;
pub use text_processing::*;

// Generated date fields from build-time schema processing
pub mod generated_date_fields {
    include!(concat!(env!("OUT_DIR"), "/date_fields.rs"));
}
