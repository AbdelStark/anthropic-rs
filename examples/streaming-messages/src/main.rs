use std::error::Error;
use std::io::Write;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use anthropic::types::{ContentBlock, ContentBlockDelta, Message, MessagesRequestBuilder, MessagesStreamEvent, Role};
use dotenv::dotenv;
use tokio_stream::StreamExt;

fn extend_messages(messages: &mut Vec<Message>, event: &MessagesStreamEvent) {
    match event {
        MessagesStreamEvent::MessageStart { message } => messages.push(message.clone()),
        MessagesStreamEvent::ContentBlockStart { content_block, .. } => {
            messages.last_mut().unwrap().content.push(content_block.clone());
        }
        MessagesStreamEvent::ContentBlockDelta { index, delta } => {
            match (messages.last_mut().unwrap().content.get_mut(*index), delta) {
                (Some(ContentBlock::Text { text }), ContentBlockDelta::TextDelta { text: delta }) => *text += delta,
                _ => unreachable!("This should currently never happen"),
            }
        }
        _ => (),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the logger.
    env_logger::init();

    // Load the environment variables from the .env file.
    dotenv().ok();

    // Build from configuration.
    let cfg = AnthropicConfig::new()?;
    let client = Client::try_from(cfg)?;
    let mut messages =
        vec![Message { role: Role::User, content: vec![ContentBlock::Text { text: "Say hello world".into() }] }];

    let messages_request = MessagesRequestBuilder::default()
        .messages(messages.clone())
        .model("claude-3-opus-20240229".to_string())
        .max_tokens(256usize)
        .stream(true)
        .build()?;

    println!("\n\nSending messages:\n{:#?}\n", messages);
    let mut stream = client.messages_stream(messages_request).await?;

    while let Some(resp) = stream.next().await {
        match resp {
            Ok(response) => {
                extend_messages(&mut messages, &response);

                // Currently it seems that only MessagesStreamEvent::ContentBlockDelta contains new text
                // snippets so we can ignore other stuff currently and just print token for token.
                // A more correct and complete example is given in `extend_messages`
                if let MessagesStreamEvent::ContentBlockDelta { delta: ContentBlockDelta::TextDelta { text }, .. } =
                    response
                {
                    print!("{}", text);
                }

                std::io::stdout().flush().unwrap();
            }
            Err(e) => {
                println!("\n{e}\n")
            }
        }
    }
    println!("\n\nNew messages:\n{:#?}", messages);

    Ok(())
}
