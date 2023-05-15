//! General configuration
use color_eyre::Result;
use config::Config;
use serde_derive::Deserialize;

/// Configuration for the application.
#[derive(Debug, Default, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: Option<String>,
}

impl AnthropicConfig {
    /// Create a new configuration from environment variables.
    pub fn new() -> Result<Self> {
        CONFIG.clone().try_deserialize().map_err(|e| e.into())
    }
}

lazy_static::lazy_static! {
    #[derive(Debug)]
    pub static ref CONFIG: Config = Config::builder()
        .add_source(config::Environment::with_prefix("anthropic"))
        .build()
        .unwrap();
}
