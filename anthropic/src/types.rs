//! Module for types used in the API.
use std::pin::Pin;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;

use crate::error::AnthropicError;
use crate::DEFAULT_MODEL;

#[derive(Clone, Serialize, Default, Debug, Builder, PartialEq)]
#[builder(pattern = "mutable")]
#[builder(setter(into, strip_option), default)]
#[builder(derive(Debug))]
#[builder(build_fn(error = "AnthropicError"))]
pub struct CompleteRequest {
    /// The prompt to complete.
    pub prompt: String,
    /// The model to use.
    #[builder(default = "DEFAULT_MODEL.to_string()")]
    pub model: String,
    /// The number of tokens to sample.
    pub max_tokens_to_sample: usize,
    /// The stop sequences to use.
    pub stop_sequences: Option<Vec<String>>,
    /// Whether to incrementally stream the response.
    #[builder(default = "false")]
    pub stream: bool,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct CompleteResponse {
    pub completion: String,
    pub stop_reason: Option<StopReason>,
}

/// Parsed server side events stream until a [StopReason::StopSequence] is received from server.
pub type CompleteResponseStream = Pin<Box<dyn Stream<Item = Result<CompleteResponse, AnthropicError>> + Send>>;

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    MaxTokens,
    StopSequence,
}
