//! Integration tests for `Client::list_models` and `Client::get_model`.

use anthropic::models::ListModelsParams;
use anthropic::Client;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_models_returns_all_models() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "claude-3-5-sonnet-20240620",
                    "type": "model",
                    "display_name": "Claude 3.5 Sonnet",
                    "created_at": "2024-06-20T00:00:00Z"
                },
                {
                    "id": "claude-3-opus-20240229",
                    "type": "model",
                    "display_name": "Claude 3 Opus",
                    "created_at": "2024-02-29T00:00:00Z"
                }
            ],
            "has_more": false,
            "first_id": "claude-3-5-sonnet-20240620",
            "last_id": "claude-3-opus-20240229"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap();
    let list = client.list_models(&ListModelsParams::new()).await.unwrap();
    assert_eq!(list.data.len(), 2);
    assert_eq!(list.data[0].id, "claude-3-5-sonnet-20240620");
    assert!(!list.has_more);
}

#[tokio::test]
async fn list_models_forwards_pagination_parameters() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("limit", "5"))
        .and(query_param("after_id", "claude-3-opus-20240229"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap();
    let params = ListModelsParams::new().limit(5).after_id("claude-3-opus-20240229");
    client.list_models(&params).await.unwrap();
}

#[tokio::test]
async fn get_model_returns_single_model() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models/claude-3-5-sonnet-20240620"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "claude-3-5-sonnet-20240620",
            "type": "model",
            "display_name": "Claude 3.5 Sonnet",
            "created_at": "2024-06-20T00:00:00Z"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap();
    let model = client.get_model("claude-3-5-sonnet-20240620").await.unwrap();
    assert_eq!(model.id, "claude-3-5-sonnet-20240620");
    assert_eq!(model.display_name, "Claude 3.5 Sonnet");
}

#[tokio::test]
async fn get_model_surfaces_not_found_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models/unknown"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "type": "error",
            "error": {
                "type": "not_found_error",
                "message": "model unknown not found"
            }
        })))
        .mount(&server)
        .await;

    let client = Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap();
    let err = client.get_model("unknown").await.unwrap_err();
    match err {
        anthropic::AnthropicError::Api(api) => {
            assert_eq!(api.error_type, "not_found_error");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}
