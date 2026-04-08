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
//! - Per-call retry override via [`RetryPolicy`] —
//!   `MessagesRequestBuilder::backoff`, `no_retries`, and `retry_policy`
//!   let individual calls opt out of retries on interactive paths or
//!   stretch them for background workers without rebuilding the client.
//! - Optional `tracing` Cargo feature — enables structured
//!   `anthropic.http` spans around every HTTP call on the transport
//!   critical path, with `method`, `path`, `status`, `attempts`, and
//!   `duration_ms` fields plus per-attempt events. The feature compiles
//!   out entirely when disabled.

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
pub use client::{Client, ClientBuilder, ExponentialBackoff};
pub use count_tokens::{CountTokensRequest, CountTokensRequestBuilder, CountTokensResponse};
pub use error::{AnthropicError, ApiError};
pub use models::{ListModelsParams, Model, ModelList};
pub use stream::{collect, collect_stream, StreamAccumulator};
pub use tool_loop::{run_tool_loop, ToolLoopConfig, ToolOutput};
pub use types::RetryPolicy;

/// Fuzzing entry points for harnesses under `fuzz/`.
///
/// These functions wrap internal parsers that run on attacker-controllable
/// bytes from the network (error bodies and JSON-Lines batch results). They
/// are not part of the stable public API — treat everything under this
/// module as an implementation detail, subject to change at any time — but
/// they need to be reachable from a sibling fuzz crate that can only see
/// `pub` items.
#[doc(hidden)]
pub mod __fuzz {
    /// Feed arbitrary bytes through the internal error-body parser the way
    /// `execute_bytes` does on a non-success response. The function must
    /// never panic and must always produce an `AnthropicError`.
    pub fn parse_error(status: u16, bytes: &[u8]) -> crate::AnthropicError {
        crate::client::parse_error(status, bytes)
    }

    /// Feed arbitrary bytes (as UTF-8) through the batch results JSONL
    /// parser. Invalid UTF-8 is silently replaced before parsing, matching
    /// how the HTTP transport path handles response bodies. The function
    /// must never panic — it can only succeed or return an
    /// `AnthropicError::InvalidRequest`.
    pub fn parse_results_jsonl(bytes: &[u8]) -> Result<Vec<crate::BatchResultItem>, crate::AnthropicError> {
        // JSONL is text-oriented. Mirror the lossy conversion the live
        // transport does so the fuzz harness can drive the parser with
        // arbitrary byte sequences without immediately bailing on non-UTF-8.
        let text = String::from_utf8_lossy(bytes);
        crate::batches::parse_results_jsonl(text.as_ref())
    }

    #[cfg(test)]
    mod regression_tests {
        use super::*;

        /// Regression corpus for `parse_error` — crash inputs found by the
        /// fuzz harness go here as permanent smoke tests.
        #[test]
        fn parse_error_handles_crash_corpus() {
            // Empty body should fall back to UnexpectedResponse.
            let _ = parse_error(0, &[]);
            // Invalid UTF-8.
            let _ = parse_error(500, &[0xff, 0xfe, 0xfd]);
            // Truncated JSON.
            let _ = parse_error(400, b"{\"error\":");
            // Valid structure, unexpected shape.
            let _ = parse_error(503, b"{\"unexpected\":true}");
            // Deeply nested JSON should not blow the stack.
            let mut nested = String::from("{\"a\":");
            for _ in 0..256 {
                nested.push_str("{\"a\":");
            }
            nested.push('1');
            for _ in 0..=256 {
                nested.push('}');
            }
            let _ = parse_error(400, nested.as_bytes());
        }

        /// Regression corpus for `parse_results_jsonl`.
        #[test]
        fn parse_results_jsonl_handles_crash_corpus() {
            // Empty input must succeed with zero items.
            assert_eq!(parse_results_jsonl(&[]).unwrap().len(), 0);
            // Only blank lines.
            assert_eq!(parse_results_jsonl(b"\n\n\n").unwrap().len(), 0);
            // Garbage bytes — should return an error, never panic.
            let _ = parse_results_jsonl(&[0xff, 0xfe, 0xfd, b'\n']);
            // Single malformed line.
            let _ = parse_results_jsonl(b"not json\n");
            // Enormous (by test standards) input with repeated malformed lines.
            let body = "not json\n".repeat(1024);
            let _ = parse_results_jsonl(body.as_bytes());
        }
    }
}
