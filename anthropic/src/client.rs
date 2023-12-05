use std::pin::Pin;

use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE};
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio_stream::{Stream, StreamExt};

use crate::config::AnthropicConfig;
use crate::error::{map_deserialization_error, AnthropicError, WrappedError};
use crate::types::{CompleteRequest, CompleteResponse, CompleteResponseStream};
use crate::{
    API_VERSION, API_VERSION_HEADER_KEY, AUTHORIZATION_HEADER_KEY, CLIENT_ID, CLIENT_ID_HEADER_KEY, DEFAULT_API_BASE,
    DEFAULT_MODEL,
};

/// The client to interact with the API.
#[derive(Builder, Debug)]
pub struct Client {
    /// The API key.
    pub api_key: String,
    /// The API base url.
    #[builder(default = "DEFAULT_API_BASE.to_string()")]
    pub api_base: String,
    /// The model to use.
    #[builder(default = "DEFAULT_MODEL.to_string()")]
    pub default_model: String,
    /// The HTTP client.
    /// Don't allow the user to set this through the builder.
    #[builder(setter(skip))]
    pub http_client: reqwest::Client,
    /// The exponential backoff strategy, defaulted to `Default::default()`.
    #[builder(default = "Default::default()")]
    pub backoff: backoff::ExponentialBackoff,
}

impl Client {
    /// Send a completion request.
    /// # Arguments
    /// * `request` - The completion request.
    /// # Returns
    /// The completion response.
    /// # Errors
    /// * `AnthropicError` - If the request fails.
    pub async fn complete(&self, request: CompleteRequest) -> Result<CompleteResponse, AnthropicError> {
        if request.stream {
            return Err(AnthropicError::InvalidArgument("When stream is true, use complete_stream() instead".into()));
        }
        self.post("/v1/complete", request).await
    }

    pub async fn complete_stream(&self, request: CompleteRequest) -> Result<CompleteResponseStream, AnthropicError> {
        if !request.stream {
            return Err(AnthropicError::InvalidArgument("When stream is false, use complete() instead".into()));
        }
        Ok(self.post_stream("/v1/complete", request).await)
    }

    /// Get the API key.
    pub fn api_key(&self) -> &str {
        self.api_key.as_str()
    }

    /// Get the API base url.
    pub fn api_base(&self) -> &str {
        self.api_base.as_str()
    }

    /// Generate the headers for the request.
    pub fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER_KEY, self.api_key().parse().unwrap());
        headers.insert(CLIENT_ID_HEADER_KEY, CLIENT_ID.as_str().parse().unwrap());
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        headers.insert(ACCEPT, "application/json".parse().unwrap());
        headers.insert(API_VERSION_HEADER_KEY, API_VERSION.parse().unwrap());
        headers
    }

    /// Make a POST request to {path} and deserialize the response body.
    /// # Arguments
    /// * `path` - The path to POST to.
    /// * `request` - The request body.
    /// # Returns
    /// The response body.
    /// # Errors
    /// * `AnthropicError` - If the request fails.
    pub(crate) async fn post<I, O>(&self, path: &str, request: I) -> Result<O, AnthropicError>
    where
        I: Serialize,
        O: DeserializeOwned,
    {
        let request = self
            .http_client
            .post(format!("{}{path}", self.api_base()))
            .bearer_auth(self.api_key())
            .headers(self.headers())
            .json(&request)
            .build()?;

        self.execute(request).await
    }

    /// Make a streaming POST request to {path} and create a Stream of the retuned Server-Sent
    /// Events # Arguments
    /// * `path` - The path to POST to.
    /// * `request` - The request body.
    /// # Returns
    /// A Stream of Server-Sent Events
    /// # Errors
    /// * `AnthropicError` - If the request fails.
    pub(crate) async fn post_stream<I, O>(
        &self,
        path: &str,
        request: I,
    ) -> Pin<Box<dyn Stream<Item = Result<O, AnthropicError>> + Send>>
    where
        I: Serialize,
        O: DeserializeOwned + Send + 'static,
    {
        let event_source = self
            .http_client
            .post(format!("{}{path}", self.api_base()))
            .bearer_auth(self.api_key())
            .headers(self.headers())
            .json(&request)
            .eventsource()
            .unwrap();

        stream(event_source).await
    }

    /// Deserialize response body from either error object or actual response object.
    /// # Arguments
    /// * `response` - The response to process.
    /// # Returns
    /// The response body.
    /// # Errors
    /// * `AnthropicError` - If the request fails.
    async fn process_response<O>(&self, response: reqwest::Response) -> Result<O, AnthropicError>
    where
        O: DeserializeOwned,
    {
        let status = response.status();
        let bytes = response.bytes().await?;

        if !status.is_success() {
            let wrapped_error: WrappedError =
                serde_json::from_slice(bytes.as_ref()).map_err(|e| map_deserialization_error(e, bytes.as_ref()))?;

            return Err(AnthropicError::ApiError(wrapped_error.error));
        }

        let response: O =
            serde_json::from_slice(bytes.as_ref()).map_err(|e| map_deserialization_error(e, bytes.as_ref()))?;
        Ok(response)
    }

    /// Execute any HTTP requests and retry on rate limit, except streaming ones as they cannot be
    /// cloned for retrying.
    /// # Arguments
    /// * `request` - The request to execute.
    /// # Returns
    /// The response body.
    /// # Errors
    /// * `AnthropicError` - If the request fails.
    async fn execute<O>(&self, request: reqwest::Request) -> Result<O, AnthropicError>
    where
        O: DeserializeOwned,
    {
        let client = self.http_client.clone();

        match request.try_clone() {
            // Only clone-able requests can be retried
            Some(request) => {
                backoff::future::retry(self.backoff.clone(), || async {
                    let response = client
                        .execute(request.try_clone().unwrap())
                        .await
                        .map_err(AnthropicError::Reqwest)
                        .map_err(backoff::Error::Permanent)?;

                    let status = response.status();
                    let bytes =
                        response.bytes().await.map_err(AnthropicError::Reqwest).map_err(backoff::Error::Permanent)?;

                    // Deserialize response body from either error object or actual response object
                    if !status.is_success() {
                        let wrapped_error: WrappedError = serde_json::from_slice(bytes.as_ref())
                            .map_err(|e| map_deserialization_error(e, bytes.as_ref()))
                            .map_err(backoff::Error::Permanent)?;

                        if status.as_u16() == 429
                            // API returns 429 also when:
                            // "You exceeded your current quota, please check your plan and billing details."
                            && wrapped_error.error.r#type != "insufficient_quota"
                        {
                            // Rate limited retry...
                            return Err(backoff::Error::Transient {
                                err: AnthropicError::ApiError(wrapped_error.error),
                                retry_after: None,
                            });
                        } else {
                            return Err(backoff::Error::Permanent(AnthropicError::ApiError(wrapped_error.error)));
                        }
                    }

                    let response: O = serde_json::from_slice(bytes.as_ref())
                        .map_err(|e| map_deserialization_error(e, bytes.as_ref()))
                        .map_err(backoff::Error::Permanent)?;
                    Ok(response)
                })
                .await
            }
            None => {
                let response = client.execute(request).await?;
                self.process_response(response).await
            }
        }
    }
}

async fn stream<O>(mut event_source: EventSource) -> Pin<Box<dyn Stream<Item = Result<O, AnthropicError>> + Send>>
where
    O: DeserializeOwned + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        while let Some(ev) = event_source.next().await {
            match ev {
                Ok(event) => match event {
                    Event::Open => continue,
                    Event::Message(message) => {
                        match message.event.as_ref() {
                            "ping" => continue,
                            "completion" => {
                                let response = match serde_json::from_str::<O>(&message.data) {
                                    Ok(output) => Ok(output),
                                    Err(e) => Err(map_deserialization_error(e, message.data.as_bytes())),
                                };

                                if let Err(_e) = tx.send(response) {
                                    // rx dropped
                                    break;
                                }
                            }
                            _ => continue,
                        }
                    }
                },
                Err(e) => {
                    if let Err(_e) = tx.send(Err(AnthropicError::StreamError(e.to_string()))) {
                        // rx dropped
                        break;
                    }
                }
            }
        }

        event_source.close();
    });

    Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx))
}

impl TryFrom<AnthropicConfig> for Client {
    type Error = AnthropicError;

    /// Try to build a client from the configuration.
    /// # Arguments
    /// * `value` - The configuration.
    fn try_from(value: AnthropicConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            api_key: value.api_key,
            api_base: value.api_base.unwrap_or_else(|| DEFAULT_API_BASE.to_string()),
            default_model: value.default_model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            http_client: reqwest::Client::new(),
            backoff: Default::default(),
        })
    }
}

impl Default for Client {
    /// Create a new client from the default configuration.
    fn default() -> Self {
        Self::try_from(AnthropicConfig::default()).unwrap()
    }
}
