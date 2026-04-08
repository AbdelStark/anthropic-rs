//! Integration tests for `Client::count_tokens`.

use anthropic::count_tokens::CountTokensRequestBuilder;
use anthropic::types::Message;
use anthropic::Client;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn count_tokens_returns_input_token_count() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages/count_tokens"))
        .and(header("x-api-key", "test-key"))
        .and(body_json(json!({
            "model": "claude-3-5-sonnet-20240620",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi there"}]}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"input_tokens": 17})))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap();
    let request =
        CountTokensRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("hi there")]).build().unwrap();

    let response = client.count_tokens(request).await.expect("count_tokens");
    assert_eq!(response.input_tokens, 17);
}

#[tokio::test]
async fn count_tokens_surfaces_api_errors() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages/count_tokens"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "type": "error",
            "error": {
                "type": "authentication_error",
                "message": "invalid api key"
            }
        })))
        .mount(&server)
        .await;

    let client = Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap();
    let request = CountTokensRequestBuilder::new("m", vec![Message::user("hi")]).build().unwrap();
    let err = client.count_tokens(request).await.unwrap_err();
    match err {
        anthropic::AnthropicError::Api(api) => {
            assert_eq!(api.error_type, "authentication_error");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}
