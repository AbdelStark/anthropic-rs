//! Integration tests for the Message Batches API.

use anthropic::batches::{
    BatchProcessingStatus, BatchRequest, BatchRequestResult, CreateBatchRequest, ListBatchesParams,
};
use anthropic::types::{Message, MessagesRequestBuilder};
use anthropic::{AnthropicError, Client};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> Client {
    Client::builder().api_key("test-key").api_base(server.uri()).build().unwrap()
}

fn sample_request() -> anthropic::types::MessagesRequest {
    MessagesRequestBuilder::new("claude-3-5-sonnet-20240620", vec![Message::user("hello")], 64).build().unwrap()
}

fn sample_batch_json(id: &str, status: &str) -> serde_json::Value {
    json!({
        "id": id,
        "type": "message_batch",
        "processing_status": status,
        "request_counts": {
            "processing": 1,
            "succeeded": 0,
            "errored": 0,
            "canceled": 0,
            "expired": 0
        },
        "created_at": "2024-10-01T00:00:00Z",
        "expires_at": "2024-10-02T00:00:00Z"
    })
}

#[tokio::test]
async fn create_batch_sends_request_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages/batches"))
        .and(body_json(json!({
            "requests": [
                {
                    "custom_id": "req_1",
                    "params": {
                        "model": "claude-3-5-sonnet-20240620",
                        "messages": [{"role": "user", "content": [{"type": "text", "text": "hello"}]}],
                        "max_tokens": 64
                    }
                }
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_batch_json("msgbatch_01", "in_progress")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let batch =
        client.create_batch(CreateBatchRequest::new(vec![BatchRequest::new("req_1", sample_request())])).await.unwrap();
    assert_eq!(batch.id, "msgbatch_01");
    assert_eq!(batch.processing_status, BatchProcessingStatus::InProgress);
    assert!(!batch.is_complete());
}

#[tokio::test]
async fn create_batch_rejects_empty_requests_locally() {
    let server = MockServer::start().await;
    let client = client(&server);
    // Must not hit the server when validation fails.
    let err = client.create_batch(CreateBatchRequest::new(vec![])).await.unwrap_err();
    assert!(matches!(err, AnthropicError::InvalidRequest(_)));
}

#[tokio::test]
async fn list_batches_forwards_pagination() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/messages/batches"))
        .and(query_param("limit", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [sample_batch_json("msgbatch_01", "ended")],
            "has_more": false,
            "first_id": "msgbatch_01",
            "last_id": "msgbatch_01"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let list = client.list_batches(&ListBatchesParams::new().limit(3)).await.unwrap();
    assert_eq!(list.data.len(), 1);
    assert!(list.data[0].is_complete());
}

#[tokio::test]
async fn get_batch_fetches_by_id() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/messages/batches/msgbatch_42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_batch_json("msgbatch_42", "ended")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let batch = client.get_batch("msgbatch_42").await.unwrap();
    assert_eq!(batch.id, "msgbatch_42");
    assert!(batch.is_complete());
}

#[tokio::test]
async fn cancel_batch_posts_to_cancel_endpoint() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages/batches/msgbatch_99/cancel"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_batch_json("msgbatch_99", "canceling")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let batch = client.cancel_batch("msgbatch_99").await.unwrap();
    assert_eq!(batch.processing_status, BatchProcessingStatus::Canceling);
}

#[tokio::test]
async fn delete_batch_sends_delete_verb() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/v1/messages/batches/msgbatch_99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msgbatch_99",
            "type": "message_batch_deleted"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let body = client.delete_batch("msgbatch_99").await.unwrap();
    assert_eq!(body["id"], "msgbatch_99");
}

#[tokio::test]
async fn get_batch_results_parses_jsonl() {
    let server = MockServer::start().await;

    let body = r#"{"custom_id":"req_1","result":{"type":"succeeded","message":{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"done"}],"model":"claude","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":3,"output_tokens":1}}}}
{"custom_id":"req_2","result":{"type":"canceled"}}
"#;

    Mock::given(method("GET"))
        .and(path("/v1/messages/batches/msgbatch_01/results"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let items = client.get_batch_results("msgbatch_01").await.unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].custom_id, "req_1");
    match &items[0].result {
        BatchRequestResult::Succeeded { message } => assert_eq!(message.text(), "done"),
        other => panic!("expected Succeeded, got {other:?}"),
    }
    assert!(matches!(items[1].result, BatchRequestResult::Canceled));
}

#[tokio::test]
async fn batch_endpoints_surface_api_errors() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/messages/batches/nope"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "type": "error",
            "error": {"type": "not_found_error", "message": "batch not found"}
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let err = client.get_batch("nope").await.unwrap_err();
    match err {
        AnthropicError::Api(api) => assert_eq!(api.error_type, "not_found_error"),
        other => panic!("expected Api error, got {other:?}"),
    }
}
