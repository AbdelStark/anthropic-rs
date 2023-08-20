use std::error::Error;
use std::io::Write;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use anthropic::types::{CompleteRequestBuilder, StopReason};
use anthropic::{AI_PROMPT, HUMAN_PROMPT};
use dotenv::dotenv;
use tokio_stream::StreamExt;

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
        .model("claude-2".to_string())
        .max_tokens_to_sample(256usize)
        .stream(true)
        .stop_sequences(vec![HUMAN_PROMPT.to_string()])
        .build()?;

    // Send a completion request.
    let mut stream = client.complete_stream(complete_request).await?;

    while let Some(resp) = stream.next().await {
        match resp {
            Ok(response) => {
                if let Some(StopReason::StopSequence) = response.stop_reason {
                    break;
                }

                print!("{}", response.completion);
                std::io::stdout().flush().unwrap();
            },
            Err(e) => {
                println!("\n{e}\n")
            },
        }
    }

    Ok(())
}
