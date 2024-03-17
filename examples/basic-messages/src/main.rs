use std::error::Error;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use anthropic::types::{ContentBlock, Message, MessagesRequestBuilder, Role};
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

    let messages =
        vec![Message { role: Role::User, content: vec![ContentBlock::Text { text: "Say hello world".into() }] }];

    let messages_request = MessagesRequestBuilder::default()
        .messages(messages.clone())
        .model("claude-3-opus-20240229".to_string())
        .max_tokens(256usize)
        .build()?;

    // Send a completion request.
    let messages_response = client.messages(messages_request).await?;

    println!("messages response:\n\n{messages_response:#?}");

    Ok(())
}
