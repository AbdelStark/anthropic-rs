//! General configuration
use config::Config;
use serde_derive::Deserialize;

use crate::error::AnthropicError;

/// Configuration for the application.
#[derive(Debug, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub api_base: Option<String>,
    pub default_model: Option<String>,
}

impl AnthropicConfig {
    /// Create a new configuration from environment variables.
    pub fn new() -> Result<Self, AnthropicError> {
        CONFIG.clone().try_deserialize().map_err(|e| e.into())
    }
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

lazy_static::lazy_static! {
    #[derive(Debug)]
    pub static ref CONFIG: Config = Config::builder()
        .add_source(config::Environment::with_prefix("anthropic"))
        .build()
        .unwrap();
}
