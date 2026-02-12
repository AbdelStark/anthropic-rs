use std::pin::Pin;
use std::time::Duration;

use backoff::ExponentialBackoff;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio_stream::Stream;

use crate::error::{AnthropicError, ErrorResponse};
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
        let api_base = self.api_base.unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        let api_version = self.api_version.unwrap_or_else(|| DEFAULT_API_VERSION.to_string());
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
pub struct Client {
    api_key: String,
    api_base: String,
    api_version: String,
    beta: Option<String>,
    http_client: reqwest::Client,
    backoff: ExponentialBackoff,
}

impl Client {
    pub fn new(api_key: impl Into<String>) -> Result<Self, AnthropicError> {
        ClientBuilder::new().api_key(api_key).build()
    }

    pub fn from_env() -> Result<Self, AnthropicError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| AnthropicError::MissingEnvironment("ANTHROPIC_API_KEY".into()))?;

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
            if let Ok(timeout_secs) = timeout.parse::<u64>() {
                builder = builder.timeout(Duration::from_secs(timeout_secs));
            }
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
        let client = self.http_client.clone();

        match request.try_clone() {
            Some(request) => {
                backoff::future::retry(self.backoff.clone(), || {
                    let request = request.try_clone().ok_or_else(|| {
                        backoff::Error::Permanent(AnthropicError::InvalidRequest("request could not be cloned".into()))
                    });
                    let client = client.clone();
                    async move {
                        let request = request?;
                        let response = client
                            .execute(request)
                            .await
                            .map_err(AnthropicError::Http)
                            .map_err(backoff::Error::Permanent)?;

                        let status = response.status();
                        let bytes =
                            response.bytes().await.map_err(AnthropicError::Http).map_err(backoff::Error::Permanent)?;

                        if !status.is_success() {
                            let error = parse_error(status.as_u16(), bytes.as_ref());
                            if status.as_u16() == 429 {
                                return Err(backoff::Error::Transient { err: error, retry_after: None });
                            }
                            return Err(backoff::Error::Permanent(error));
                        }

                        let response = serde_json::from_slice::<O>(bytes.as_ref())
                            .map_err(AnthropicError::Deserialize)
                            .map_err(backoff::Error::Permanent)?;
                        Ok(response)
                    }
                })
                .await
            }
            None => {
                let response = client.execute(request).await?;
                process_response(response).await
            }
        }
    }
}

pub type MessagesResponseStream = Pin<Box<dyn Stream<Item = Result<MessagesStreamEvent, AnthropicError>> + Send>>;

async fn process_response<O>(response: reqwest::Response) -> Result<O, AnthropicError>
where
    O: DeserializeOwned,
{
    let status = response.status();
    let bytes = response.bytes().await?;

    if !status.is_success() {
        return Err(parse_error(status.as_u16(), bytes.as_ref()));
    }

    let response = serde_json::from_slice::<O>(bytes.as_ref())?;
    Ok(response)
}

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

                    let error = AnthropicError::InvalidRequest(format!("stream error: {e}"));
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
