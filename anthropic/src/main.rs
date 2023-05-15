use std::error::Error;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the logger.
    env_logger::init();

    // Load the environment variables from the .env file.
    dotenv().ok();

    // Build from configuration.
    let cfg = AnthropicConfig::new()?;
    let client = Client::try_from(cfg)?;

    // Send a completion request.
    let _completion_result = client.complete().await?;

    Ok(())
}
