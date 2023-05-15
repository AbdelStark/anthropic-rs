#[macro_use]
extern crate log;
use std::error::Error;

use anthropic::client::{Client, ClientBuilder};
use anthropic::config::AnthropicConfig;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the logger.
    env_logger::init();

    // Load the environment variables from the .env file.
    dotenv().ok();

    // You can use automatic configuration from environment variables or build the client manually.
    // Build manually.
    let client = ClientBuilder::default().api_key("...".to_owned()).default_model("claude-v1".to_owned()).build()?;

    // Build from configuration.
    let cfg = AnthropicConfig::new()?;
    let client = Client::try_from(cfg)?;

    // TODO: use the client and interact with the API.

    Ok(())
}
