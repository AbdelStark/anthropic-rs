//! # Anthropic Rust SDK
//!
//! A production-grade Rust client for the Anthropic API.
//!
//! ## Quickstart
//! ```no_run
//! use anthropic::types::{Message, MessagesRequestBuilder};
//! use anthropic::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::from_env()?;
//!     let request = MessagesRequestBuilder::new(
//!         "claude-3-5-sonnet-20240620",
//!         vec![Message::user("Tell me a haiku about Rust.")],
//!         256,
//!     )
//!     .temperature(0.7)
//!     .build()?;
//!
//!     let response = client.messages(request).await?;
//!     println!("{}", response.text());
//!     Ok(())
//! }
//! ```
//!
//! ## What's included
//!
//! - [`Client`] / [`ClientBuilder`] for the `/v1/messages` and
//!   `/v1/messages/count_tokens` endpoints.
//! - Models API: [`Client::list_models`](client::Client::list_models) and
//!   [`Client::get_model`](client::Client::get_model).
//! - Message Batches API: [`Client::create_batch`](client::Client::create_batch),
//!   [`Client::list_batches`](client::Client::list_batches),
//!   [`Client::get_batch`](client::Client::get_batch),
//!   [`Client::cancel_batch`](client::Client::cancel_batch),
//!   [`Client::delete_batch`](client::Client::delete_batch), and
//!   [`Client::get_batch_results`](client::Client::get_batch_results) (JSONL-aware).
//! - [`StreamAccumulator`] / [`collect_stream`] to fold a live SSE stream
//!   into a fully materialized [`types::MessagesResponse`].
//! - [`run_tool_loop`] to drive a tool-use conversation end-to-end.
//! - Prompt-caching (`CacheControl`), extended thinking (`ThinkingConfig`),
//!   service tier, image / document blocks, and all other modern request
//!   fields are supported on [`types::MessagesRequestBuilder`].

pub mod batches;
pub mod client;
pub mod count_tokens;
pub mod error;
pub mod models;
pub mod stream;
pub mod tool_loop;
pub mod types;

pub use batches::{
    BatchProcessingStatus, BatchRequest, BatchRequestCounts, BatchRequestResult, BatchResultItem, CreateBatchRequest,
    ListBatchesParams, MessageBatch, MessageBatchList,
};
pub use client::{Client, ClientBuilder};
pub use count_tokens::{CountTokensRequest, CountTokensRequestBuilder, CountTokensResponse};
pub use error::{AnthropicError, ApiError};
pub use models::{ListModelsParams, Model, ModelList};
pub use stream::{collect, collect_stream, StreamAccumulator};
pub use tool_loop::{run_tool_loop, ToolLoopConfig, ToolOutput};
