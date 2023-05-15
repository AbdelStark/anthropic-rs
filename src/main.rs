#[macro_use]
extern crate log;
use anthropic::config::AnthropicConfig;
use color_eyre::eyre::Result;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger.
    env_logger::init();

    // Initialize the error handler.
    color_eyre::install()?;

    // Load the environment variables from the .env file.
    dotenv().ok();

    // Retrieve the application configuration.
    let _cfg = AnthropicConfig::new()?;

    // TODO: use the configuration and interact with the API.

    // Say hello.
    info!("hello from anthropic-rs 🤖 !");

    Ok(())
}
