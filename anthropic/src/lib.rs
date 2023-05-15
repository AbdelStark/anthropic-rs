//! # Anthropic Rust SDK
//! This is the Rust SDK for Anthropic. It is a work in progress.
//! The goal is to provide a Rust interface to the Anthropic API.
//!
//! ## Usage
//! - TODO: add usage instructions.

#[macro_use]
extern crate derive_builder;

pub mod client;
pub mod config;
pub mod error;
pub mod types;

/// Default model to use.
pub const DEFAULT_MODEL: &str = "claude-v1";
/// Default v1 API base url.
pub const DEFAULT_API_BASE: &str = "https://api.anthropic.com";
/// Auth header key.
pub const AUTHORIZATION_HEADER_KEY: &str = "x-api-key";
