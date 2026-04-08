//! End-to-end demo of [`anthropic::tool_loop::run_tool_loop`].
//!
//! This example defines two "weather" tools, registers them on a Messages
//! request, and lets the helper drive the whole call/execute/reply cycle.

use anthropic::tool_loop::{run_tool_loop, ToolLoopConfig, ToolOutput};
use anthropic::types::{Message, MessagesRequestBuilder, Tool, ToolChoice};
use anthropic::Client;
use dotenvy::dotenv;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let client = Client::from_env()?;

    let request = MessagesRequestBuilder::new(
        "claude-3-5-sonnet-20240620",
        vec![Message::user("Compare today's weather in Paris and Rome in one sentence, using the get_weather tool.")],
        512,
    )
    .tools(vec![Tool::new(
        "get_weather",
        "Fetch the current weather for a city by name",
        json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "City name" }
            },
            "required": ["city"]
        }),
    )])
    .tool_choice(ToolChoice::Auto)
    .build()?;

    let response = run_tool_loop(
        &client,
        request,
        |name, input| async move {
            println!("-> executing tool `{name}` with input {input}");
            if name != "get_weather" {
                return Ok(ToolOutput::error(format!("unknown tool: {name}")));
            }
            let city = input.get("city").and_then(|v| v.as_str()).unwrap_or("");
            let result = match city.to_lowercase().as_str() {
                "paris" => "Paris: 22C and sunny",
                "rome" => "Rome: 27C and partly cloudy",
                other => return Ok(ToolOutput::error(format!("unknown city: {other}"))),
            };
            Ok(ToolOutput::ok(result))
        },
        ToolLoopConfig::default(),
    )
    .await?;

    println!("\nFinal answer:\n{}", response.text());
    Ok(())
}
