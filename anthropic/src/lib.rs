//! # Anthropic Rust SDK
//!
//! A production-grade Rust client for the Anthropic API.
//!
//! ## Quickstart
//! ```no_run
//! use anthropic::types::{ContentBlock, Message, MessagesRequestBuilder, Role};
//! use anthropic::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::from_env()?;
//!     let messages = vec![Message {
//!         role: Role::User,
//!         content: vec![ContentBlock::text("Tell me a haiku about Rust.")],
//!     }];
//!
//!     let request = MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", messages, 256)
//!         .temperature(0.7)
//!         .build()?;
//!
//!     let response = client.messages(request).await?;
//!     println!("{response:#?}");
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod types;

pub use client::{Client, ClientBuilder};
pub use error::{AnthropicError, ApiError};
