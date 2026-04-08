use std::io::Write;

use anthropic::stream::StreamAccumulator;
use anthropic::types::{ContentBlockDelta, Message, MessagesRequestBuilder, MessagesStreamEvent};
use anthropic::Client;
use dotenvy::dotenv;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let client = Client::from_env()?;
    let messages = vec![Message::user("Stream a short greeting.")];

    let request = MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", messages.clone(), 128).build()?;

    println!("\n\nSending messages:\n{messages:#?}\n");
    let mut stream = client.messages_stream(request).await?;

    // Tee every event into a StreamAccumulator while also printing text
    // deltas as they arrive. When the stream ends we materialize the final
    // MessagesResponse — no manual book-keeping required.
    let mut accumulator = StreamAccumulator::new();

    while let Some(event) = stream.next().await {
        let event = event?;

        if let MessagesStreamEvent::ContentBlockDelta { delta: ContentBlockDelta::TextDelta { text }, .. } = &event {
            print!("{text}");
            std::io::stdout().flush().ok();
        }

        accumulator.push(event)?;
    }

    let response = accumulator.finish()?;
    println!("\n\nFinal response:\n{response:#?}");

    Ok(())
}
