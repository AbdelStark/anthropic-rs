//! Integration tests for `Client::messages` using a wiremock-backed server.

use anthropic::types::{Message, MessagesRequestBuilder, Role, StopReason};
use anthropic::{AnthropicError, Client};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn build_client(server: &MockServer) -> Client {
    Client::builder().api_key("test-key").api_base(server.uri()).build().expect("client")
}

fn sample_request() -> anthropic::types::MessagesRequest {
    MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("hi")], 128).build().unwrap()
}

#[tokio::test]
async fn messages_sends_correct_headers_and_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "model": "claude-3-5-sonnet-20240620",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi"}]}
            ],
            "max_tokens": 128
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "hello"}],
            "model": "claude-3-5-sonnet-20240620",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {"input_tokens": 5, "output_tokens": 2}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(&server);
    let response = client.messages(sample_request()).await.expect("ok");

    assert_eq!(response.id, "msg_01");
    assert_eq!(response.role, Role::Assistant);
    assert_eq!(response.text(), "hello");
    assert_eq!(response.stop_reason, Some(StopReason::EndTurn));
    assert_eq!(response.usage.input_tokens, 5);
    assert_eq!(response.usage.output_tokens, 2);
}

#[tokio::test]
async fn messages_beta_header_is_forwarded() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("anthropic-beta", "prompt-caching-2024-07-31"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_02",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "claude",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {"input_tokens": 1, "output_tokens": 0}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        Client::builder().api_key("test-key").api_base(server.uri()).beta("prompt-caching-2024-07-31").build().unwrap();

    client.messages(sample_request()).await.expect("ok");
}

#[tokio::test]
async fn messages_surfaces_api_errors() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "messages.0.content.0: missing field"
            }
        })))
        .mount(&server)
        .await;

    let client = build_client(&server);
    let err = client.messages(sample_request()).await.unwrap_err();
    match err {
        AnthropicError::Api(api) => {
            assert_eq!(api.error_type, "invalid_request_error");
            assert!(api.message.contains("missing"));
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn messages_surfaces_unexpected_response_bodies() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("gateway down"))
        .mount(&server)
        .await;

    let client = build_client(&server);
    let err = client.messages(sample_request()).await.unwrap_err();
    match err {
        AnthropicError::UnexpectedResponse { status, body } => {
            assert_eq!(status, 500);
            assert_eq!(body, "gateway down");
        }
        other => panic!("expected UnexpectedResponse, got {other:?}"),
    }
}

#[tokio::test]
async fn messages_rejects_stream_true_requests() {
    let server = MockServer::start().await;
    let client = build_client(&server);

    let request = MessagesRequestBuilder::new("claude", vec![Message::user("hi")], 10).stream(true).build().unwrap();
    let err = client.messages(request).await.unwrap_err();
    assert!(matches!(err, AnthropicError::InvalidRequest(_)));
}

#[tokio::test]
async fn messages_retries_429_then_succeeds() {
    let server = MockServer::start().await;

    // First two attempts return 429 with a Retry-After header. The third
    // attempt returns a normal response. The default ExponentialBackoff has
    // an initial interval of 500ms, and Retry-After takes precedence — the
    // test runs in well under a second per attempt.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0").set_body_json(json!({
            "type": "error",
            "error": {"type": "rate_limit_error", "message": "slow down"}
        })))
        .up_to_n_times(2)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_ok",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "ok"}],
            "model": "claude",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(&server);
    let response = client.messages(sample_request()).await.expect("retried success");
    assert_eq!(response.text(), "ok");
}

#[tokio::test]
async fn messages_preserves_conversation_roundtrip() {
    let server = MockServer::start().await;

    // Echo back a tool_use response so we can verify extractor helpers.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_03",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Calling tool. "},
                {
                    "type": "tool_use",
                    "id": "tu_1",
                    "name": "get_weather",
                    "input": {"city": "Paris"}
                }
            ],
            "model": "claude",
            "stop_reason": "tool_use",
            "stop_sequence": null,
            "usage": {"input_tokens": 8, "output_tokens": 12}
        })))
        .mount(&server)
        .await;

    let client = build_client(&server);
    let response = client.messages(sample_request()).await.unwrap();

    assert!(response.has_tool_use());
    assert_eq!(response.first_text(), Some("Calling tool. "));
    let tool_uses: Vec<_> = response.tool_uses().collect();
    assert_eq!(tool_uses.len(), 1);
    let (id, name, input) = tool_uses[0];
    assert_eq!(id, "tu_1");
    assert_eq!(name, "get_weather");
    assert_eq!(input["city"], "Paris");
}
