# Anthropic Rust SDK 🦀

[![Rust docs](https://docs.rs/anthropic/badge.svg)](https://docs.rs/anthropic)
[![Rust crate](https://img.shields.io/crates/v/anthropic.svg)](https://crates.io/crates/anthropic)

`anthropic` is a production-grade Rust SDK for the Anthropic Messages API, with streaming support and
first-class typed requests.

## Features

- ✅ Messages API (`/v1/messages`)
- ✅ Streaming responses (Server-Sent Events)
- ✅ Tool use / tool results
- ✅ Typed builders and ergonomic helpers

## Installation

```bash
cargo add anthropic
```

## Usage

```rust
use anthropic::types::{ContentBlock, Message, MessagesRequestBuilder, Role};
use anthropic::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;

    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text("Write a short limerick about Rust.")],
    }];

    let request = MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", messages, 256)
        .temperature(0.7)
        .build()?;

    let response = client.messages(request).await?;
    println!("{response:#?}");

    Ok(())
}
```

### Streaming

```rust
use anthropic::types::{ContentBlock, Message, MessagesRequestBuilder, Role};
use anthropic::Client;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;

    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text("Stream a haiku about Claude.")],
    }];

    let request = MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", messages, 128).build()?;
    let mut stream = client.messages_stream(request).await?;

    while let Some(event) = stream.next().await {
        println!("{event:?}");
    }

    Ok(())
}
```

## Configuration

The client reads configuration from environment variables:

- `ANTHROPIC_API_KEY` (required)
- `ANTHROPIC_API_BASE` (optional, defaults to `https://api.anthropic.com`)
- `ANTHROPIC_API_VERSION` (optional, defaults to `2023-06-01`)
- `ANTHROPIC_BETA` (optional, for beta headers like `tools-2024-04-04`)
- `ANTHROPIC_TIMEOUT_SECS` (optional, defaults to 60 seconds)

You can also build a client manually with `ClientBuilder`.

## License

MIT
