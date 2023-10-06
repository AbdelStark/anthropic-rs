//! # Anthropic Rust SDK
//! This is the Rust SDK for Anthropic. It is a work in progress.
//! The goal is to provide a Rust interface to the Anthropic API.
//!
//! ## Usage
//! ```rust
//! use std::error::Error;
//! use anthropic::client::ClientBuilder;
//! use anthropic::config::AnthropicConfig;
//! use anthropic::types::CompleteRequestBuilder;
//! use anthropic::{AI_PROMPT, HUMAN_PROMPT};
//! use dotenv::dotenv;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn Error>> {
//! // Load the environment variables from the .env file.
//! dotenv().ok();
//!
//! // Build with manual configuration.
//! let client = ClientBuilder::default().api_key("my-api-key".to_string()).build()?;
//!
//! let complete_request = CompleteRequestBuilder::default()
//!     .prompt(format!("{HUMAN_PROMPT}How many toes do dogs have?{AI_PROMPT}"))
//!     .model("claude-instant-1".to_string())
//!     .stream(false)
//!     .stop_sequences(vec![HUMAN_PROMPT.to_string()])
//!     .build()?;
//!
//!  // Send a completion request.
//! let _complete_response_result = client.complete(complete_request).await;
//! // Do something with the response.
//!
//! Ok(())
//! }

use lazy_static::lazy_static;
use rustc_version::version;

#[macro_use]
extern crate derive_builder;

pub mod client;
pub mod config;
pub mod error;
pub mod types;

lazy_static! {
    /// A value to represent the client id of this SDK.
    pub static ref CLIENT_ID: String = client_id();
}

/// A constant to represent the human prompt.
pub const HUMAN_PROMPT: &str = "\n\nHuman:";
/// A constant to represent the assistant prompt.
pub const AI_PROMPT: &str = "\n\nAssistant:";

/// Default model to use.
pub const DEFAULT_MODEL: &str = "claude-v1";
/// Default v1 API base url.
pub const DEFAULT_API_BASE: &str = "https://api.anthropic.com";
/// Auth header key.
const AUTHORIZATION_HEADER_KEY: &str = "x-api-key";
/// Client id header key.
const CLIENT_ID_HEADER_KEY: &str = "Client";
/// API version header key.
/// Ref: https://docs.anthropic.com/claude/reference/versioning
const API_VERSION_HEADER_KEY: &str = "anthropic-version";

/// Ref: https://docs.anthropic.com/claude/reference/versioning
const API_VERSION: &str = "2023-06-01";

/// Get the client id.
pub fn client_id() -> String {
    // Get the Rust version used to build SDK at compile time.
    let rust_version = match version() {
        Ok(v) => v.to_string(),
        Err(_) => "unknown".to_string(),
    };
    let crate_name = env!("CARGO_PKG_NAME");
    let crate_version = env!("CARGO_PKG_VERSION");
    format!("rustv{rust_version}/{crate_name}/{crate_version}")
}
