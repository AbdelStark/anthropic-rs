use std::pin::Pin;
use std::time::Duration;

use backoff::ExponentialBackoff;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio_stream::Stream;

use crate::batches::{
    parse_results_jsonl, BatchResultItem, CreateBatchRequest, ListBatchesParams, MessageBatch, MessageBatchList,
};
use crate::count_tokens::{CountTokensRequest, CountTokensResponse};
use crate::error::{AnthropicError, ErrorResponse};
use crate::models::{ListModelsParams, Model, ModelList};
use crate::types::{MessagesRequest, MessagesResponse, MessagesStreamEvent};

const DEFAULT_API_BASE: &str = "https://api.anthropic.com";
const DEFAULT_API_VERSION: &str = "2023-06-01";
const API_KEY_HEADER: &str = "x-api-key";
const VERSION_HEADER: &str = "anthropic-version";
const BETA_HEADER: &str = "anthropic-beta";

/// Configure and build an Anthropic API client.
#[derive(Debug, Default)]
pub struct ClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
    api_version: Option<String>,
    beta: Option<String>,
    timeout: Option<Duration>,
    backoff: Option<ExponentialBackoff>,
    http_client: Option<reqwest::Client>,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn api_version(mut self, api_version: impl Into<String>) -> Self {
        self.api_version = Some(api_version.into());
        self
    }

    pub fn beta(mut self, beta: impl Into<String>) -> Self {
        self.beta = Some(beta.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn backoff(mut self, backoff: ExponentialBackoff) -> Self {
        self.backoff = Some(backoff);
        self
    }

    pub fn http_client(mut self, http_client: reqwest::Client) -> Self {
        self.http_client = Some(http_client);
        self
    }

    pub fn build(self) -> Result<Client, AnthropicError> {
        let api_key = self.api_key.ok_or_else(|| AnthropicError::InvalidRequest("api_key is required".into()))?;
        if api_key.trim().is_empty() {
            return Err(AnthropicError::InvalidRequest("api_key must not be empty".into()));
        }
        let api_base = self.api_base.unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        if api_base.trim().is_empty() {
            return Err(AnthropicError::InvalidRequest("api_base must not be empty".into()));
        }
        let api_version = self.api_version.unwrap_or_else(|| DEFAULT_API_VERSION.to_string());
        if api_version.trim().is_empty() {
            return Err(AnthropicError::InvalidRequest("api_version must not be empty".into()));
        }
        let timeout = self.timeout.unwrap_or_else(|| Duration::from_secs(60));
        let http_client = match self.http_client {
            Some(client) => client,
            None => reqwest::Client::builder().timeout(timeout).build()?,
        };

        Ok(Client {
            api_key,
            api_base,
            api_version,
            beta: self.beta,
            http_client,
            backoff: self.backoff.unwrap_or_default(),
        })
    }
}

/// The client to interact with the Anthropic API.
///
/// `Client` is cheap to clone — the underlying `reqwest::Client` is reference
/// counted internally — so most applications will build one client at startup
/// and clone it into request handlers as needed.
#[derive(Clone)]
pub struct Client {
    api_key: String,
    api_base: String,
    api_version: String,
    beta: Option<String>,
    http_client: reqwest::Client,
    backoff: ExponentialBackoff,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the raw API key — debug-printing a client is a common
        // way to leak credentials into logs.
        f.debug_struct("Client")
            .field("api_key", &"<redacted>")
            .field("api_base", &self.api_base)
            .field("api_version", &self.api_version)
            .field("beta", &self.beta)
            .finish()
    }
}

impl Client {
    pub fn new(api_key: impl Into<String>) -> Result<Self, AnthropicError> {
        ClientBuilder::new().api_key(api_key).build()
    }

    /// Shortcut for [`ClientBuilder::new`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Build a client from `ANTHROPIC_*` environment variables.
    ///
    /// Errors:
    /// - [`AnthropicError::MissingEnvironment`] if `ANTHROPIC_API_KEY` is unset
    ///   or empty.
    /// - [`AnthropicError::InvalidRequest`] if `ANTHROPIC_TIMEOUT_SECS` is set
    ///   but cannot be parsed as a positive `u64`.
    pub fn from_env() -> Result<Self, AnthropicError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AnthropicError::MissingEnvironment("ANTHROPIC_API_KEY".into()))?;

        let mut builder = ClientBuilder::new().api_key(api_key);

        if let Ok(api_base) = std::env::var("ANTHROPIC_API_BASE") {
            builder = builder.api_base(api_base);
        }

        if let Ok(api_version) = std::env::var("ANTHROPIC_API_VERSION") {
            builder = builder.api_version(api_version);
        }

        if let Ok(beta) = std::env::var("ANTHROPIC_BETA") {
            builder = builder.beta(beta);
        }

        if let Ok(timeout) = std::env::var("ANTHROPIC_TIMEOUT_SECS") {
            let timeout_secs = timeout.parse::<u64>().map_err(|_| {
                AnthropicError::InvalidRequest(format!(
                    "ANTHROPIC_TIMEOUT_SECS must be a positive integer (got {timeout:?})"
                ))
            })?;
            builder = builder.timeout(Duration::from_secs(timeout_secs));
        }

        builder.build()
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub fn api_version(&self) -> &str {
        &self.api_version
    }

    pub fn beta(&self) -> Option<&str> {
        self.beta.as_deref()
    }

    pub async fn messages(&self, mut request: MessagesRequest) -> Result<MessagesResponse, AnthropicError> {
        if matches!(request.stream, Some(true)) {
            return Err(AnthropicError::InvalidRequest("stream=true requests must use messages_stream".into()));
        }
        request.stream = None;
        self.post("/v1/messages", &request).await
    }

    pub async fn messages_stream(
        &self,
        mut request: MessagesRequest,
    ) -> Result<MessagesResponseStream, AnthropicError> {
        request.stream = Some(true);
        self.post_stream("/v1/messages", &request).await
    }

    /// `POST /v1/messages/count_tokens` — compute the input-token cost of a
    /// Messages request without actually generating a response.
    pub async fn count_tokens(&self, request: CountTokensRequest) -> Result<CountTokensResponse, AnthropicError> {
        self.post("/v1/messages/count_tokens", &request).await
    }

    /// `GET /v1/models` — list every model available to the authenticated key.
    pub async fn list_models(&self, params: &ListModelsParams) -> Result<ModelList, AnthropicError> {
        self.get("/v1/models", &params.as_query()).await
    }

    /// `GET /v1/models/{model_id}` — fetch metadata about a single model.
    pub async fn get_model(&self, model_id: &str) -> Result<Model, AnthropicError> {
        let path = format!("/v1/models/{}", model_id);
        self.get::<Model>(&path, &[]).await
    }

    /// `POST /v1/messages/batches` — submit a new batch of Messages requests.
    pub async fn create_batch(&self, request: CreateBatchRequest) -> Result<MessageBatch, AnthropicError> {
        request.validate()?;
        self.post("/v1/messages/batches", &request).await
    }

    /// `GET /v1/messages/batches` — list batches submitted by this workspace.
    pub async fn list_batches(&self, params: &ListBatchesParams) -> Result<MessageBatchList, AnthropicError> {
        self.get("/v1/messages/batches", &params.as_query()).await
    }

    /// `GET /v1/messages/batches/{id}` — fetch current metadata for a batch.
    pub async fn get_batch(&self, batch_id: &str) -> Result<MessageBatch, AnthropicError> {
        let path = format!("/v1/messages/batches/{}", batch_id);
        self.get::<MessageBatch>(&path, &[]).await
    }

    /// `POST /v1/messages/batches/{id}/cancel` — request cancellation of a
    /// batch. Already-completed requests remain available in the results.
    pub async fn cancel_batch(&self, batch_id: &str) -> Result<MessageBatch, AnthropicError> {
        let path = format!("/v1/messages/batches/{}/cancel", batch_id);
        self.post_empty::<MessageBatch>(&path).await
    }

    /// `DELETE /v1/messages/batches/{id}` — permanently delete a batch.
    pub async fn delete_batch(&self, batch_id: &str) -> Result<serde_json::Value, AnthropicError> {
        let path = format!("/v1/messages/batches/{}", batch_id);
        self.delete::<serde_json::Value>(&path).await
    }

    /// `GET /v1/messages/batches/{id}/results` — download and parse the
    /// JSON-Lines results file for a completed batch.
    pub async fn get_batch_results(&self, batch_id: &str) -> Result<Vec<BatchResultItem>, AnthropicError> {
        let path = format!("/v1/messages/batches/{}/results", batch_id);
        let body = self.get_raw(&path).await?;
        parse_results_jsonl(&body)
    }

    fn headers(&self) -> Result<HeaderMap, AnthropicError> {
        let mut headers = HeaderMap::new();
        headers.insert(API_KEY_HEADER, HeaderValue::from_str(&self.api_key)?);
        headers.insert(VERSION_HEADER, HeaderValue::from_str(&self.api_version)?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_str(&format!("anthropic-rs/{}", env!("CARGO_PKG_VERSION")))?);
        if let Some(beta) = &self.beta {
            headers.insert(BETA_HEADER, HeaderValue::from_str(beta)?);
        }
        Ok(headers)
    }

    async fn post<I, O>(&self, path: &str, request: &I) -> Result<O, AnthropicError>
    where
        I: Serialize + ?Sized,
        O: DeserializeOwned,
    {
        let request =
            self.http_client.post(format!("{}{path}", self.api_base)).headers(self.headers()?).json(request).build()?;

        self.execute(request).await
    }

    async fn get<O>(&self, path: &str, query: &[(&str, String)]) -> Result<O, AnthropicError>
    where
        O: DeserializeOwned,
    {
        let request =
            self.http_client.get(format!("{}{path}", self.api_base)).headers(self.headers()?).query(query).build()?;

        self.execute(request).await
    }

    async fn get_raw(&self, path: &str) -> Result<String, AnthropicError> {
        let request = self.http_client.get(format!("{}{path}", self.api_base)).headers(self.headers()?).build()?;
        self.execute_raw(request).await
    }

    async fn post_empty<O>(&self, path: &str) -> Result<O, AnthropicError>
    where
        O: DeserializeOwned,
    {
        let request = self.http_client.post(format!("{}{path}", self.api_base)).headers(self.headers()?).build()?;
        self.execute(request).await
    }

    async fn delete<O>(&self, path: &str) -> Result<O, AnthropicError>
    where
        O: DeserializeOwned,
    {
        let request = self.http_client.delete(format!("{}{path}", self.api_base)).headers(self.headers()?).build()?;
        self.execute(request).await
    }

    async fn post_stream<I>(
        &self,
        path: &str,
        request: &I,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<MessagesStreamEvent, AnthropicError>> + Send>>, AnthropicError>
    where
        I: Serialize + ?Sized,
    {
        let event_source = self
            .http_client
            .post(format!("{}{path}", self.api_base))
            .headers(self.headers()?)
            .json(request)
            .eventsource()
            .map_err(|err| AnthropicError::EventSourceCannotClone(err.into()))?;

        Ok(stream(event_source).await)
    }

    async fn execute<O>(&self, request: reqwest::Request) -> Result<O, AnthropicError>
    where
        O: DeserializeOwned,
    {
        let bytes = self.execute_bytes(request).await?;
        serde_json::from_slice::<O>(&bytes).map_err(AnthropicError::Deserialize)
    }

    async fn execute_raw(&self, request: reqwest::Request) -> Result<String, AnthropicError> {
        let bytes = self.execute_bytes(request).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Send a request with retry-on-429, returning the raw success body.
    ///
    /// All response parsing happens in callers; this method only deals with
    /// transport, retries, and HTTP-level error mapping.
    async fn execute_bytes(&self, request: reqwest::Request) -> Result<Vec<u8>, AnthropicError> {
        let client = self.http_client.clone();

        // `reqwest::Request` cannot be cloned when its body is a stream. In
        // that case we cannot safely retry — fall back to a single attempt.
        let Some(retryable) = request.try_clone() else {
            let response = client.execute(request).await?;
            return process_bytes(response).await;
        };

        backoff::future::retry(self.backoff.clone(), || {
            let request = retryable.try_clone().ok_or_else(|| {
                backoff::Error::Permanent(AnthropicError::InvalidRequest("request could not be cloned".into()))
            });
            let client = client.clone();
            async move {
                let request = request?;
                let response =
                    client.execute(request).await.map_err(AnthropicError::Http).map_err(backoff::Error::Permanent)?;

                let status = response.status();
                let retry_after = parse_retry_after(response.headers().get(reqwest::header::RETRY_AFTER));
                let bytes = response.bytes().await.map_err(AnthropicError::Http).map_err(backoff::Error::Permanent)?;

                if !status.is_success() {
                    let error = parse_error(status.as_u16(), bytes.as_ref());
                    if status.as_u16() == 429 {
                        return Err(backoff::Error::Transient { err: error, retry_after });
                    }
                    return Err(backoff::Error::Permanent(error));
                }

                Ok(bytes.to_vec())
            }
        })
        .await
    }
}

/// Parse a `Retry-After` header value into a [`Duration`].
///
/// Honors both forms supported by RFC 7231:
/// - integer seconds (e.g. `Retry-After: 30`)
/// - HTTP-date (currently ignored — exotic in practice for `429` responses)
///
/// Returns `None` if the header is missing, malformed, or contains zero.
fn parse_retry_after(header: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    let value = header?.to_str().ok()?.trim();
    let seconds = value.parse::<u64>().ok()?;
    if seconds == 0 {
        return None;
    }
    Some(Duration::from_secs(seconds))
}

async fn process_bytes(response: reqwest::Response) -> Result<Vec<u8>, AnthropicError> {
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        return Err(parse_error(status.as_u16(), bytes.as_ref()));
    }
    Ok(bytes.to_vec())
}

pub type MessagesResponseStream = Pin<Box<dyn Stream<Item = Result<MessagesStreamEvent, AnthropicError>> + Send>>;

fn parse_error(status: u16, bytes: &[u8]) -> AnthropicError {
    if let Ok(error) = serde_json::from_slice::<ErrorResponse>(bytes) {
        return AnthropicError::Api(error.error);
    }

    let body = String::from_utf8_lossy(bytes).to_string();
    AnthropicError::UnexpectedResponse { status, body }
}

async fn stream(
    mut event_source: EventSource,
) -> Pin<Box<dyn Stream<Item = Result<MessagesStreamEvent, AnthropicError>> + Send>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        while let Some(ev) = event_source.next().await {
            match ev {
                Ok(Event::Open) => continue,
                Ok(Event::Message(message)) => {
                    let event = message.event.as_str();
                    if event == "ping" {
                        continue;
                    }

                    let response = if event == "error" {
                        match serde_json::from_str::<ErrorResponse>(&message.data) {
                            Ok(error) => Err(AnthropicError::Api(error.error)),
                            Err(err) => Err(AnthropicError::Deserialize(err)),
                        }
                    } else {
                        match serde_json::from_str::<MessagesStreamEvent>(&message.data) {
                            Ok(output) => Ok(output),
                            Err(err) => Err(AnthropicError::Deserialize(err)),
                        }
                    };

                    let cancel = response.is_err();
                    if tx.send(response).is_err() || cancel {
                        break;
                    }
                }
                Err(e) => {
                    if let reqwest_eventsource::Error::StreamEnded = e {
                        break;
                    }

                    // Surface transport errors with their typed variant so
                    // callers can match on `AnthropicError::EventSource`
                    // instead of string-sniffing an `InvalidRequest`.
                    let error = AnthropicError::EventSource(Box::new(e));
                    if tx.send(Err(error)).is_err() {
                        break;
                    }
                }
            }
        }

        event_source.close();
    });

    Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn builder_rejects_missing_api_key() {
        let err = ClientBuilder::new().build().unwrap_err();
        assert!(matches!(err, AnthropicError::InvalidRequest(_)));
    }

    #[test]
    fn builder_rejects_empty_api_key() {
        let err = ClientBuilder::new().api_key("   ").build().unwrap_err();
        assert!(matches!(err, AnthropicError::InvalidRequest(_)));
        assert!(format!("{err}").contains("api_key"));
    }

    #[test]
    fn builder_rejects_empty_api_base() {
        let err = ClientBuilder::new().api_key("k").api_base("").build().unwrap_err();
        assert!(format!("{err}").contains("api_base"));
    }

    #[test]
    fn debug_redacts_api_key() {
        let client = Client::builder().api_key("super-secret-key").build().unwrap();
        let rendered = format!("{client:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("super-secret-key"));
    }

    #[test]
    fn client_is_clone() {
        let client = Client::builder().api_key("k").build().unwrap();
        let _cloned = client.clone();
    }

    #[test]
    fn parse_retry_after_handles_seconds() {
        let header = HeaderValue::from_static("30");
        assert_eq!(parse_retry_after(Some(&header)), Some(Duration::from_secs(30)));
    }

    #[test]
    fn parse_retry_after_ignores_zero_and_invalid() {
        let zero = HeaderValue::from_static("0");
        assert_eq!(parse_retry_after(Some(&zero)), None);
        let date = HeaderValue::from_static("Wed, 21 Oct 2015 07:28:00 GMT");
        assert_eq!(parse_retry_after(Some(&date)), None);
        assert_eq!(parse_retry_after(None), None);
    }

    #[test]
    fn parse_error_falls_back_to_unexpected_response() {
        let err = parse_error(500, b"not json");
        match err {
            AnthropicError::UnexpectedResponse { status, body } => {
                assert_eq!(status, 500);
                assert_eq!(body, "not json");
            }
            other => panic!("expected UnexpectedResponse, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_decodes_api_payload() {
        let body = br#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
        let err = parse_error(429, body);
        match err {
            AnthropicError::Api(api) => {
                assert_eq!(api.error_type, "rate_limit_error");
                assert_eq!(api.message, "slow down");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }
}
