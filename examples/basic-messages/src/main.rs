use anthropic::types::{Message, MessagesRequestBuilder};
use anthropic::Client;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let client = Client::from_env()?;

    let request = MessagesRequestBuilder::new(
        "claude-3-5-sonnet-20240620",
        vec![Message::user("Say hello in one sentence.")],
        128,
    )
    .build()?;
    let response = client.messages(request).await?;

    println!("messages response:\n{response:#?}");

    Ok(())
}
