use std::fmt;

use reqwest::header::InvalidHeaderValue;
use reqwest_eventsource::{CannotCloneRequestError, Error as EventSourceError};
use serde::{Deserialize, Serialize};

/// Errors returned by the Anthropic SDK.
#[derive(Debug, thiserror::Error)]
pub enum AnthropicError {
    /// Underlying HTTP error from reqwest.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// Anthropic API returned an error payload.
    #[error("api error: {0}")]
    Api(ApiError),
    /// Error when a response cannot be deserialized into a Rust type.
    #[error("failed to deserialize api response: {0}")]
    Deserialize(#[from] serde_json::Error),
    /// Invalid request arguments.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// Missing required environment variable.
    #[error("missing environment variable: {0}")]
    MissingEnvironment(String),
    /// Invalid header value provided for request headers.
    #[error("invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    /// Eventsource setup failure.
    #[error("eventsource error: {0}")]
    EventSource(#[from] Box<EventSourceError>),
    /// Eventsource request could not be cloned.
    #[error("eventsource request could not be cloned: {0}")]
    EventSourceCannotClone(#[from] Box<CannotCloneRequestError>),
    /// Unexpected response payload.
    #[error("unexpected response (status {status}): {body}")]
    UnexpectedResponse { status: u16, body: String },
}

/// Anthropic API error payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: Option<serde_json::Value>,
    pub code: Option<serde_json::Value>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_type, self.message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ErrorResponse {
    pub error: ApiError,
}
