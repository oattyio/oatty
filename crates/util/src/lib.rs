//! # Oatty Utility Library
//!
//! This crate provides utility functions for the Oatty CLI, including HTTP utilities,
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
//! use oatty_util::{redact_sensitive, fuzzy_score, lex_shell_like};
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
pub mod command_vec_utils;
pub mod date_handling;
pub mod history_store;
pub mod http;
pub mod http_exec;
pub mod http_path_resolution;
pub mod path_processing;
pub mod preferences;
pub mod schema;
pub mod shell_lexing;
pub mod text_processing;

// Re-export commonly used items for convenience
pub use command_vec_utils::*;
pub use date_handling::*;
pub use history_store::*;
pub use http::*;
pub use http_exec::*;
pub use http_path_resolution::*;
pub use path_processing::*;
pub use preferences::*;
pub use schema::*;
pub use shell_lexing::*;
pub use text_processing::*;

// Generated date fields from build-time schema processing
pub mod generated_date_fields {
    include!(concat!(env!("OUT_DIR"), "/date_fields.rs"));
}
