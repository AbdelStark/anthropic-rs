use crate::config::AnthropicConfig;
use crate::error::AnthropicError;

pub const DEFAULT_MODEL: &str = "claude-v1";

/// The client to interact with the API.
#[derive(Default, Builder, Debug)]
pub struct Client {
    /// The API key.
    pub api_key: String,
    /// The model to use.
    pub default_model: String,
}

impl Client {
    /// Send a completion request.
    /// # TODO: implement this function.
    pub async fn complete(&self) -> Result<(), AnthropicError> {
        Ok(())
    }
}

impl TryFrom<AnthropicConfig> for Client {
    type Error = AnthropicError;

    fn try_from(value: AnthropicConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            api_key: value.api_key,
            default_model: value.default_model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        })
    }
}
