//! Integration tests for `run_tool_loop` against a wiremock-backed server.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anthropic::tool_loop::{run_tool_loop, ToolLoopConfig, ToolOutput};
use anthropic::types::{Message, MessagesRequestBuilder, Tool, ToolChoice};
use anthropic::{AnthropicError, Client};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> Client {
    Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap()
}

fn tool_request() -> anthropic::types::MessagesRequest {
    MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("What's the weather in Paris?")], 256)
        .tools(vec![Tool::new(
            "get_weather",
            "Fetch the current weather for a city",
            json!({
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            }),
        )])
        .tool_choice(ToolChoice::Auto)
        .build()
        .unwrap()
}

fn tool_use_response(id: &str, city: &str) -> serde_json::Value {
    json!({
        "id": "msg_tool",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "Let me check."},
            {
                "type": "tool_use",
                "id": id,
                "name": "get_weather",
                "input": {"city": city}
            }
        ],
        "model": "claude-3-5-sonnet-20240620",
        "stop_reason": "tool_use",
        "stop_sequence": null,
        "usage": {"input_tokens": 20, "output_tokens": 15}
    })
}

fn text_response(text: &str) -> serde_json::Value {
    json!({
        "id": "msg_final",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": text}],
        "model": "claude-3-5-sonnet-20240620",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 40, "output_tokens": 12}
    })
}

#[tokio::test]
async fn tool_loop_runs_single_tool_then_returns_final_response() {
    let server = MockServer::start().await;

    // First call: model asks for a tool. Second call: model returns plain text.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_use_response("tu_1", "Paris")))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(text_response("It's sunny in Paris.")))
        .mount(&server)
        .await;

    let client = client(&server);
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);

    let response = run_tool_loop(
        &client,
        tool_request(),
        move |name, input| {
            let call_count = Arc::clone(&call_count_clone);
            async move {
                call_count.fetch_add(1, Ordering::SeqCst);
                assert_eq!(name, "get_weather");
                assert_eq!(input["city"], "Paris");
                Ok(ToolOutput::ok("sunny, 22C"))
            }
        },
        ToolLoopConfig::default(),
    )
    .await
    .unwrap();

    assert_eq!(response.text(), "It's sunny in Paris.");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn tool_loop_handles_multiple_tool_calls_in_one_turn() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_tool",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Checking both cities."},
                {"type": "tool_use", "id": "tu_1", "name": "get_weather", "input": {"city": "Paris"}},
                {"type": "tool_use", "id": "tu_2", "name": "get_weather", "input": {"city": "Rome"}}
            ],
            "model": "claude-3-5-sonnet-20240620",
            "stop_reason": "tool_use",
            "stop_sequence": null,
            "usage": {"input_tokens": 20, "output_tokens": 15}
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(text_response("Paris is sunny, Rome is rainy.")))
        .mount(&server)
        .await;

    let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let calls_clone = Arc::clone(&calls);

    let client = client(&server);
    let response = run_tool_loop(
        &client,
        tool_request(),
        move |_name, input| {
            let calls = Arc::clone(&calls_clone);
            async move {
                let city = input["city"].as_str().unwrap().to_string();
                calls.lock().unwrap().push(city.clone());
                let result = if city == "Paris" { "sunny" } else { "rainy" };
                Ok(ToolOutput::ok(format!("{city}: {result}")))
            }
        },
        ToolLoopConfig::default(),
    )
    .await
    .unwrap();

    assert_eq!(response.text(), "Paris is sunny, Rome is rainy.");
    let locked = calls.lock().unwrap();
    assert_eq!(locked.len(), 2);
    assert!(locked.contains(&"Paris".to_string()));
    assert!(locked.contains(&"Rome".to_string()));
}

#[tokio::test]
async fn tool_loop_propagates_tool_errors_to_model() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_use_response("tu_1", "Unknown")))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(text_response("That city is unknown.")))
        .mount(&server)
        .await;

    let client = client(&server);
    let response = run_tool_loop(
        &client,
        tool_request(),
        |_name, _input| async move { Ok(ToolOutput::error("city not found")) },
        ToolLoopConfig::default(),
    )
    .await
    .unwrap();

    assert_eq!(response.text(), "That city is unknown.");
}

#[tokio::test]
async fn tool_loop_bails_out_on_iteration_limit() {
    let server = MockServer::start().await;

    // The mock always returns a tool_use response, so the loop never terminates.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_use_response("tu_1", "Paris")))
        .mount(&server)
        .await;

    let client = client(&server);
    let err =
        run_tool_loop(&client, tool_request(), |_n, _i| async { Ok(ToolOutput::ok("sunny")) }, ToolLoopConfig::new(2))
            .await
            .unwrap_err();

    match err {
        AnthropicError::InvalidRequest(msg) => {
            assert!(msg.contains("2 iterations"));
        }
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn tool_loop_propagates_executor_errors() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_use_response("tu_1", "Paris")))
        .mount(&server)
        .await;

    let client = client(&server);
    let err = run_tool_loop(
        &client,
        tool_request(),
        |_n, _i| async move { Err(AnthropicError::InvalidRequest("tool blew up".into())) },
        ToolLoopConfig::default(),
    )
    .await
    .unwrap_err();

    match err {
        AnthropicError::InvalidRequest(msg) => assert_eq!(msg, "tool blew up"),
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn tool_loop_returns_directly_when_no_tool_use() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(text_response("Just chatting.")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);
    let response = run_tool_loop(
        &client,
        tool_request(),
        move |_n, _i| {
            let c = Arc::clone(&call_count_clone);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(ToolOutput::ok("never called"))
            }
        },
        ToolLoopConfig::default(),
    )
    .await
    .unwrap();

    assert_eq!(response.text(), "Just chatting.");
    assert_eq!(call_count.load(Ordering::SeqCst), 0);
}
