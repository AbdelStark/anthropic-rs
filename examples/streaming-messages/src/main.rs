use std::io::Write;

use anthropic::types::{ContentBlock, ContentBlockDelta, Message, MessagesRequestBuilder, MessagesStreamEvent, Role};
use anthropic::Client;
use dotenvy::dotenv;
use tokio_stream::StreamExt;

fn extend_messages(messages: &mut Vec<Message>, event: &MessagesStreamEvent) {
    match event {
        MessagesStreamEvent::MessageStart { message } => messages.push(message.clone()),
        MessagesStreamEvent::ContentBlockStart { content_block, .. } => {
            if let Some(last) = messages.last_mut() {
                last.content.push(content_block.clone());
            }
        }
        MessagesStreamEvent::ContentBlockDelta { index, delta } => {
            if let Some(last) = messages.last_mut() {
                match (last.content.get_mut(*index), delta) {
                    (Some(ContentBlock::Text { text }), ContentBlockDelta::TextDelta { text: delta }) => {
                        *text += delta;
                    }
                    _ => (),
                }
            }
        }
        _ => (),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let client = Client::from_env()?;
    let mut messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text("Stream a short greeting.")],
    }];

    let request = MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", messages.clone(), 128).build()?;

    println!("\n\nSending messages:\n{messages:#?}\n");
    let mut stream = client.messages_stream(request).await?;

    while let Some(resp) = stream.next().await {
        match resp {
            Ok(response) => {
                extend_messages(&mut messages, &response);

                if let MessagesStreamEvent::ContentBlockDelta {
                    delta: ContentBlockDelta::TextDelta { text },
                    ..
                } = response
                {
                    print!("{text}");
                    std::io::stdout().flush().ok();
                }
            }
            Err(e) => {
                println!("\n{e}\n");
            }
        }
    }

    println!("\n\nNew messages:\n{messages:#?}");

    Ok(())
}
