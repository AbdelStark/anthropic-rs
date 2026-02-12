use anthropic::types::{ContentBlock, Message, MessagesRequestBuilder, Role};
use anthropic::Client;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let client = Client::from_env()?;

    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text("Say hello in one sentence.")],
    }];

    let request = MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", messages, 128).build()?;
    let response = client.messages(request).await?;

    println!("messages response:\n{response:#?}");

    Ok(())
}
