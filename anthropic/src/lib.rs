//! # Anthropic Rust SDK
//! This is the Rust SDK for Anthropic. It is a work in progress.
//! The goal is to provide a Rust interface to the Anthropic API.
//!
//! ## Usage
//! - TODO: add usage instructions.

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
pub const AUTHORIZATION_HEADER_KEY: &str = "x-api-key";
/// Client id header key.
pub const CLIENT_ID_HEADER_KEY: &str = "Client";

/// Get the client id.
pub fn client_id() -> String {
    // Get the Rust version used to build SDK at compile time.
    let rust_version = version().unwrap();
    let crate_name = env!("CARGO_PKG_NAME");
    let crate_version = env!("CARGO_PKG_VERSION");
    format!("rustv{rust_version}/{crate_name}/{crate_version}")
}
