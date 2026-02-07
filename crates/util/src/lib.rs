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
pub mod async_runtime;
pub mod command_vec_utils;
pub mod date_handling;
pub mod history_store;
pub mod http;
pub mod keystore;
pub mod openapi_validation;
pub mod path_processing;
pub mod preferences;
pub mod schema;
pub mod shell_lexing;
pub mod text_processing;
// Re-export commonly used items for convenience
pub use async_runtime::*;
pub use command_vec_utils::*;
pub use date_handling::*;
pub use history_store::*;
pub use http::*;
pub use keystore::*;
pub use openapi_validation::*;
pub use path_processing::*;
pub use preferences::*;
pub use schema::*;
pub use shell_lexing::*;
pub use text_processing::*;
