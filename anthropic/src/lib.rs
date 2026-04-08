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
