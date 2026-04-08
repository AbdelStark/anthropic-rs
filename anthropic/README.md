<div align="center">

# anthropic-rs

Typed async Rust client for Anthropic's Messages API with structured requests, SSE streaming, and tool-use payloads.

<p>
  <a href="https://crates.io/crates/anthropic"><img src="https://img.shields.io/crates/v/anthropic?logo=rust&label=crate" alt="Crates.io"></a>
  <a href="https://docs.rs/anthropic"><img src="https://docs.rs/anthropic/badge.svg" alt="Docs.rs"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-2b3137" alt="License"></a>
  <img src="https://img.shields.io/badge/runtime-tokio-0f172a" alt="Tokio runtime">
  <img src="https://img.shields.io/badge/transport-reqwest%20%2B%20SSE-115e59" alt="Reqwest and SSE">
  <img src="https://img.shields.io/badge/all_contributors-6-orange.svg" alt="All contributors">
</p>

![Terminal preview of anthropic-rs](../docs/screenshot.svg)

</div>

## How It Works

```text
Prompt / image blocks / tool schema
                |
                v
+-------------------------------------------+
| MessagesRequestBuilder                    |
| model | max_tokens | temperature | tools  |
+---------------------------+---------------+
                            | build()
                            v
+-------------------------------------------+
| Client / ClientBuilder                    |
| env | headers | timeout | backoff         |
+---------------+-------------------+-------+
                |                   |
                | messages()        | messages_stream()
                v                   v
     POST /v1/messages      POST /v1/messages + stream=true
                |                   |
                v                   v
+-------------------------+  +-----------------------------+
| MessagesResponse        |  | MessagesStreamEvent         |
| content | usage | stop  |  | start | delta | stop        |
+-------------------------+  +-----------------------------+

429 responses are retried with exponential backoff before surfacing an error.
```

## 3-Step Quick Start

1. Clone the repo.

```bash
git clone https://github.com/AbdelStark/anthropic-rs.git
cd anthropic-rs
```

2. Export your API key.

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

3. Run the basic example.

```bash
cargo run --manifest-path examples/basic-messages/Cargo.toml
```

Expected output:

```text
messages response:
MessagesResponse {
    id: "msg_...",
    message_type: "message",
    role: Assistant,
    content: [
        Text {
            text: "Hello ...",
        },
    ],
    usage: Usage {
        input_tokens: ...,
        output_tokens: ...,
    },
}
```

## The Good Stuff

### 1. Send one typed message

```rust
use anthropic::types::{Message, MessagesRequestBuilder};
use anthropic::Client;

let client = Client::from_env()?;
let request = MessagesRequestBuilder::new(
    "claude-3-5-sonnet-20240620",
    vec![Message::user("Summarize this diff in one sentence.")],
    256,
)
.temperature(0.2)
.build()?;

let response = client.messages(request).await?;
println!("{}", response.text());
```

- `Message::user` / `Message::assistant` wrap a single text block — no enum literals required.
- `MessagesResponse::text()`, `first_text()`, and `tool_uses()` pull structured
  data back out without matching on `ContentBlock` by hand.
- `temperature(0.2)` keeps output tighter and less varied; the builder also
  validates non-empty `model` / `messages` and non-zero `max_tokens`.

### 2. Stream text and materialize the final response

```rust
use anthropic::stream::StreamAccumulator;
use anthropic::types::{ContentBlockDelta, Message, MessagesRequestBuilder, MessagesStreamEvent};
use anthropic::Client;
use tokio_stream::StreamExt;

let client = Client::from_env()?;
let request = MessagesRequestBuilder::new(
    "claude-3-5-sonnet-20240620",
    vec![Message::user("Stream a short release note.")],
    128,
)
.build()?;

let mut stream = client.messages_stream(request).await?;
let mut accumulator = StreamAccumulator::new();

while let Some(event) = stream.next().await {
    let event = event?;
    if let MessagesStreamEvent::ContentBlockDelta {
        delta: ContentBlockDelta::TextDelta { text },
        ..
    } = &event {
        print!("{text}");
    }
    accumulator.push(event)?;
}

let response = accumulator.finish()?;
```

- `StreamAccumulator` folds every `MessagesStreamEvent` into a
  `MessagesResponse`, handling text, tool-use `input_json_delta` chunks,
  extended-thinking `thinking_delta` / `signature_delta`, and usage /
  stop-reason updates.
- Prefer `anthropic::stream::collect(stream).await` when you just want the
  final response without any per-event processing.

### 3. Drive a full tool-use loop with one helper

```rust
use anthropic::tool_loop::{run_tool_loop, ToolLoopConfig, ToolOutput};
use anthropic::types::{Message, MessagesRequestBuilder, Tool, ToolChoice};
use anthropic::Client;
use serde_json::json;

let client = Client::from_env()?;
let request = MessagesRequestBuilder::new(
    "claude-3-5-sonnet-20240620",
    vec![Message::user("What's the weather in Paris?")],
    512,
)
.tools(vec![Tool::new(
    "get_weather",
    "Fetch current weather for a city",
    json!({
        "type": "object",
        "properties": { "city": { "type": "string" } },
        "required": ["city"]
    }),
)])
.tool_choice(ToolChoice::Auto)
.build()?;

let response = run_tool_loop(
    &client,
    request,
    |name, input| async move {
        assert_eq!(name, "get_weather");
        let city = input["city"].as_str().unwrap_or("");
        Ok(ToolOutput::ok(format!("{city}: 22C and sunny")))
    },
    ToolLoopConfig::default(),
)
.await?;

println!("{}", response.text());
```

- `run_tool_loop` handles the entire call-execute-reply cycle — it clones
  the original request each iteration (keeping `tools` / `tool_choice` /
  `system` intact), collects every `tool_use` block in parallel, runs your
  executor, appends `tool_result` blocks, and stops once the model returns
  a tool-free response or `max_iterations` is hit.
- Return `ToolOutput::error("...")` to surface a tool-level failure to the
  model; return `Err(AnthropicError)` to abort the loop instead.

### 4. Prompt caching, extended thinking, and image / document blocks

```rust
use anthropic::types::{CacheControl, ContentBlock, Message, MessagesRequestBuilder, Role, ServiceTier, ThinkingConfig};

let request = MessagesRequestBuilder::new(
    "claude-3-5-sonnet-20240620",
    vec![
        Message::new(
            Role::User,
            vec![
                ContentBlock::image_url("https://example.com/chart.png"),
                ContentBlock::document_url("https://example.com/handbook.pdf"),
                ContentBlock::text("Summarize both attachments."),
            ],
        ),
    ],
    1024,
)
.system("You are a careful analyst.")
.thinking(ThinkingConfig::enabled(2048))
.service_tier(ServiceTier::Auto)
.tools(vec![]) // add tool schemas as needed
.build()?;

// Tag any cacheable block or tool with a CacheControl marker:
let cached_prompt = ContentBlock::text("...long system context...")
    .with_cache_control(CacheControl::ephemeral());
```

- `CacheControl::ephemeral()` / `::ephemeral_ttl("1h")` attach a cache
  marker to any `Text` / `Image` / `Document` / `ToolUse` / `ToolResult`
  block or tool definition.
- `ContentBlock` constructors cover base64 / URL images, base64 / URL /
  inline text documents, tool-use + tool-result (ok and error), thinking
  (with optional signature), and plain text.
- `ThinkingConfig::enabled(budget)` turns on extended thinking;
  `ServiceTier::StandardOnly` opts out of priority routing.

### 5. count_tokens, list_models, get_model

```rust
use anthropic::count_tokens::CountTokensRequestBuilder;
use anthropic::models::ListModelsParams;
use anthropic::types::Message;

let count = client
    .count_tokens(
        CountTokensRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("hi")]).build()?,
    )
    .await?;
println!("this request would cost {} input tokens", count.input_tokens);

let models = client.list_models(&ListModelsParams::new().limit(20)).await?;
for m in &models.data {
    println!("{} - {}", m.id, m.display_name);
}
let detail = client.get_model("claude-3-5-sonnet-20240620").await?;
```

### 6. Message Batches

```rust
use anthropic::batches::{BatchRequest, CreateBatchRequest, ListBatchesParams};
use anthropic::types::{Message, MessagesRequestBuilder};

let batch = client
    .create_batch(CreateBatchRequest::new(vec![
        BatchRequest::new(
            "req_1",
            MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("hi")], 64).build()?,
        ),
        BatchRequest::new(
            "req_2",
            MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("bye")], 64).build()?,
        ),
    ]))
    .await?;

// Poll until the batch finishes...
let batch = client.get_batch(&batch.id).await?;
if batch.is_complete() {
    for item in client.get_batch_results(&batch.id).await? {
        println!("{}: {:?}", item.custom_id, item.result);
    }
}

// Or page through every batch on the workspace:
let _list = client.list_batches(&ListBatchesParams::new().limit(10)).await?;
// ...cancel_batch / delete_batch round out the CRUD surface.
```

## Configuration

### Environment

| Variable | Default | Description |
| --- | --- | --- |
| `ANTHROPIC_API_KEY` | none | Required API key used for the `x-api-key` header. |
| `ANTHROPIC_API_BASE` | `https://api.anthropic.com` | Override the API base URL. |
| `ANTHROPIC_API_VERSION` | `2023-06-01` | Sets the `anthropic-version` header. |
| `ANTHROPIC_BETA` | none | Optional `anthropic-beta` header for beta features. |
| `ANTHROPIC_TIMEOUT_SECS` | `60` | Request timeout in seconds when building from env. |

### Core API

| Call | Returns | Notes |
| --- | --- | --- |
| `Client::new(api_key)` / `Client::builder()` | `Result<Client, AnthropicError>` | Manual setup when you do not want env-based config. |
| `Client::from_env()` | `Result<Client, AnthropicError>` | Reads the environment variables above. |
| `client.messages(request)` | `Result<MessagesResponse, AnthropicError>` | Rejects `stream=true` requests. |
| `client.messages_stream(request)` | `Result<MessagesResponseStream, AnthropicError>` | Opens an SSE stream and yields typed events. |
| `client.count_tokens(request)` | `Result<CountTokensResponse, AnthropicError>` | `POST /v1/messages/count_tokens`. |
| `client.list_models(&params)` / `client.get_model(id)` | `Result<ModelList / Model, AnthropicError>` | `GET /v1/models` with pagination. |
| `client.create_batch(request)` | `Result<MessageBatch, AnthropicError>` | `POST /v1/messages/batches` with local non-empty validation. |
| `client.list_batches(&params)` / `client.get_batch(id)` | `Result<MessageBatchList / MessageBatch, AnthropicError>` | List and poll batches. |
| `client.cancel_batch(id)` / `client.delete_batch(id)` | `Result<.., AnthropicError>` | Batch lifecycle management. |
| `client.get_batch_results(id)` | `Result<Vec<BatchResultItem>, AnthropicError>` | Download + parse the JSONL results file. |
| `StreamAccumulator` / `anthropic::stream::collect` | `Result<MessagesResponse, AnthropicError>` | Folds a live SSE stream into a full response. |
| `run_tool_loop(&client, request, executor, config)` | `Result<MessagesResponse, AnthropicError>` | Agentic call/execute/reply loop with iteration budget. |
| `ClientBuilder::backoff(...)` | `ClientBuilder` | Customizes retry behavior for cloneable requests. |

## Deployment / Integration

Use the example crate as a CI smoke test inside GitHub Actions:

```yaml
name: anthropic-smoke

on:
  workflow_dispatch:

jobs:
  basic-messages:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo run --manifest-path examples/basic-messages/Cargo.toml
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

That exercises `Client::from_env()`, request building, and `/v1/messages` end to end.

## Contributors ✨

Thanks goes to these wonderful people ([emoji key](https://allcontributors.org/docs/en/emoji-key)):

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="16.66%"><a href="https://github.com/AbdelStark"><img src="https://github.com/AbdelStark.png?size=100" width="100px;" alt="A₿del ∞/21M 🐺 - 🐱"/><br /><sub><b>A₿del ∞/21M 🐺 - 🐱</b></sub></a><br /><a href="https://github.com/AbdelStark/anthropic-rs/commits?author=AbdelStark" title="Code">💻</a></td>
      <td align="center" valign="top" width="16.66%"><a href="https://github.com/ofalvai"><img src="https://github.com/ofalvai.png?size=100" width="100px;" alt="ofalvai"/><br /><sub><b>ofalvai</b></sub></a><br /><a href="https://github.com/AbdelStark/anthropic-rs/commits?author=ofalvai" title="Code">💻</a></td>
      <td align="center" valign="top" width="16.66%"><a href="https://github.com/JohnAllen"><img src="https://github.com/JohnAllen.png?size=100" width="100px;" alt="JohnAllen"/><br /><sub><b>JohnAllen</b></sub></a><br /><a href="https://github.com/AbdelStark/anthropic-rs/commits?author=JohnAllen" title="Code">💻</a></td>
      <td align="center" valign="top" width="16.66%"><a href="https://github.com/Philipp-M"><img src="https://github.com/Philipp-M.png?size=100" width="100px;" alt="Philipp-M"/><br /><sub><b>Philipp-M</b></sub></a><br /><a href="https://github.com/AbdelStark/anthropic-rs/commits?author=Philipp-M" title="Code">💻</a></td>
      <td align="center" valign="top" width="16.66%"><a href="https://github.com/wyatt-avilla"><img src="https://github.com/wyatt-avilla.png?size=100" width="100px;" alt="wyatt-avilla"/><br /><sub><b>wyatt-avilla</b></sub></a><br /><a href="https://github.com/AbdelStark/anthropic-rs/commits?author=wyatt-avilla" title="Code">💻</a></td>
      <td align="center" valign="top" width="16.66%"><a href="https://github.com/aoikurokawa"><img src="https://github.com/aoikurokawa.png?size=100" width="100px;" alt="aoikurokawa"/><br /><sub><b>aoikurokawa</b></sub></a><br /><a href="https://github.com/AbdelStark/anthropic-rs/commits?author=aoikurokawa" title="Code">💻</a></td>
    </tr>
  </tbody>
</table>
<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

This project follows the [all-contributors](https://allcontributors.org) specification. Contributions of any kind welcome!
