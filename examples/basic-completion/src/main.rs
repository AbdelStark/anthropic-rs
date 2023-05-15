use std::error::Error;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use anthropic::types::CompleteRequestBuilder;
use anthropic::{AI_PROMPT, HUMAN_PROMPT};
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

    let complete_request = CompleteRequestBuilder::default()
        .prompt(format!("{HUMAN_PROMPT}How many toes do dogs have?{AI_PROMPT}"))
        .model("claude-v1".to_string())
        .stream_response(false)
        .stop_sequences(vec![HUMAN_PROMPT.to_string()])
        .build()?;

    // Send a completion request.
    let complete_response = client.complete(complete_request).await?;

    println!("completion response: {complete_response:?}");

    Ok(())
}
