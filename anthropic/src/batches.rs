//! Types for the Message Batches API (`/v1/messages/batches`).
//!
//! Message batches let callers submit many Messages requests in a single
//! operation and poll for results asynchronously. Each request in the batch is
//! identified by a `custom_id` that the caller chooses.

use serde::{Deserialize, Serialize};

use crate::error::AnthropicError;
use crate::types::{MessagesRequest, MessagesResponse};

/// Individual request entry submitted as part of a batch.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BatchRequest {
    pub custom_id: String,
    pub params: MessagesRequest,
}

impl BatchRequest {
    pub fn new(custom_id: impl Into<String>, params: MessagesRequest) -> Self {
        Self { custom_id: custom_id.into(), params }
    }
}

/// Payload for `POST /v1/messages/batches`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CreateBatchRequest {
    pub requests: Vec<BatchRequest>,
}

impl CreateBatchRequest {
    pub fn new(requests: Vec<BatchRequest>) -> Self {
        Self { requests }
    }

    /// Validate that a batch has at least one request before sending.
    pub fn validate(&self) -> Result<(), AnthropicError> {
        if self.requests.is_empty() {
            return Err(AnthropicError::InvalidRequest("batch must contain at least one request".into()));
        }
        Ok(())
    }
}

/// Processing state of a batch as a whole.
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchProcessingStatus {
    InProgress,
    Canceling,
    Ended,
}

/// Per-status request counts that the API returns alongside every batch.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct BatchRequestCounts {
    #[serde(default)]
    pub processing: u32,
    #[serde(default)]
    pub succeeded: u32,
    #[serde(default)]
    pub errored: u32,
    #[serde(default)]
    pub canceled: u32,
    #[serde(default)]
    pub expired: u32,
}

/// Batch metadata returned by `POST /v1/messages/batches` and the get/list
/// endpoints.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct MessageBatch {
    pub id: String,
    #[serde(rename = "type")]
    pub batch_type: String,
    pub processing_status: BatchProcessingStatus,
    pub request_counts: BatchRequestCounts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_initiated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results_url: Option<String>,
}

impl MessageBatch {
    /// True when the batch has stopped processing (ended, canceled, or expired).
    pub fn is_complete(&self) -> bool {
        matches!(self.processing_status, BatchProcessingStatus::Ended)
    }
}

/// Paginated list of batches returned by `GET /v1/messages/batches`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct MessageBatchList {
    pub data: Vec<MessageBatch>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
}

/// Pagination parameters for listing batches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListBatchesParams {
    pub before_id: Option<String>,
    pub after_id: Option<String>,
    pub limit: Option<u32>,
}

impl ListBatchesParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn before_id(mut self, before_id: impl Into<String>) -> Self {
        self.before_id = Some(before_id.into());
        self
    }

    pub fn after_id(mut self, after_id: impl Into<String>) -> Self {
        self.after_id = Some(after_id.into());
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub(crate) fn as_query(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        if let Some(before) = &self.before_id {
            out.push(("before_id", before.clone()));
        }
        if let Some(after) = &self.after_id {
            out.push(("after_id", after.clone()));
        }
        if let Some(limit) = self.limit {
            out.push(("limit", limit.to_string()));
        }
        out
    }
}

/// Outcome of a single request inside a batch result payload.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum BatchRequestResult {
    Succeeded { message: MessagesResponse },
    Errored { error: serde_json::Value },
    Canceled,
    Expired,
}

/// Individual line item returned by the results endpoint.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BatchResultItem {
    pub custom_id: String,
    pub result: BatchRequestResult,
}

/// Parse a JSON-Lines payload (one `BatchResultItem` per line) into a vector.
///
/// Blank lines are ignored.
pub fn parse_results_jsonl(body: &str) -> Result<Vec<BatchResultItem>, AnthropicError> {
    let mut out = Vec::new();
    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let item: BatchResultItem = serde_json::from_str(trimmed).map_err(|e| {
            AnthropicError::InvalidRequest(format!("failed to parse batch result line {}: {}", idx + 1, e))
        })?;
        out.push(item);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessagesRequestBuilder};
    use serde_json::json;

    fn sample_request() -> MessagesRequest {
        MessagesRequestBuilder::new("claude", vec![Message::user("hi")], 128).build().unwrap()
    }

    #[test]
    fn create_batch_request_validates_non_empty() {
        let err = CreateBatchRequest::new(vec![]).validate().unwrap_err();
        assert!(format!("{err}").contains("at least one"));

        CreateBatchRequest::new(vec![BatchRequest::new("a", sample_request())]).validate().unwrap();
    }

    #[test]
    fn message_batch_deserializes() {
        let batch: MessageBatch = serde_json::from_value(json!({
            "id": "msgbatch_01",
            "type": "message_batch",
            "processing_status": "in_progress",
            "request_counts": {
                "processing": 2,
                "succeeded": 1,
                "errored": 0,
                "canceled": 0,
                "expired": 0
            },
            "ended_at": null,
            "created_at": "2024-10-01T00:00:00Z",
            "expires_at": "2024-10-02T00:00:00Z",
            "archived_at": null,
            "cancel_initiated_at": null,
            "results_url": null
        }))
        .unwrap();
        assert_eq!(batch.id, "msgbatch_01");
        assert_eq!(batch.processing_status, BatchProcessingStatus::InProgress);
        assert_eq!(batch.request_counts.processing, 2);
        assert_eq!(batch.request_counts.succeeded, 1);
        assert!(!batch.is_complete());
    }

    #[test]
    fn message_batch_is_complete_when_ended() {
        let batch: MessageBatch = serde_json::from_value(json!({
            "id": "msgbatch_01",
            "type": "message_batch",
            "processing_status": "ended",
            "request_counts": {"processing": 0, "succeeded": 2, "errored": 0, "canceled": 0, "expired": 0},
            "created_at": "2024-10-01T00:00:00Z",
            "expires_at": "2024-10-02T00:00:00Z",
            "results_url": "https://api.anthropic.com/v1/messages/batches/msgbatch_01/results"
        }))
        .unwrap();
        assert!(batch.is_complete());
        assert_eq!(
            batch.results_url.as_deref(),
            Some("https://api.anthropic.com/v1/messages/batches/msgbatch_01/results")
        );
    }

    #[test]
    fn list_batches_params_builds_query() {
        let params = ListBatchesParams::new().limit(5).before_id("b1");
        assert_eq!(params.as_query(), vec![("before_id", "b1".to_string()), ("limit", "5".to_string())]);
    }

    #[test]
    fn batch_result_succeeded_roundtrip() {
        let body = json!({
            "custom_id": "req_1",
            "result": {
                "type": "succeeded",
                "message": {
                    "id": "msg_1",
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "text", "text": "done"}],
                    "model": "claude",
                    "stop_reason": "end_turn",
                    "stop_sequence": null,
                    "usage": {"input_tokens": 3, "output_tokens": 1}
                }
            }
        });
        let item: BatchResultItem = serde_json::from_value(body).unwrap();
        assert_eq!(item.custom_id, "req_1");
        match item.result {
            BatchRequestResult::Succeeded { message } => {
                assert_eq!(message.text(), "done");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn batch_result_errored_and_canceled_roundtrip() {
        let errored: BatchResultItem = serde_json::from_value(json!({
            "custom_id": "req_2",
            "result": {"type": "errored", "error": {"type": "invalid_request_error", "message": "bad"}}
        }))
        .unwrap();
        matches!(errored.result, BatchRequestResult::Errored { .. });

        let canceled: BatchResultItem = serde_json::from_value(json!({
            "custom_id": "req_3",
            "result": {"type": "canceled"}
        }))
        .unwrap();
        assert!(matches!(canceled.result, BatchRequestResult::Canceled));
    }

    #[test]
    fn parse_jsonl_handles_multiple_lines_and_blanks() {
        let body = r#"{"custom_id":"a","result":{"type":"canceled"}}

{"custom_id":"b","result":{"type":"expired"}}
"#;
        let items = parse_results_jsonl(body).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].custom_id, "a");
        assert!(matches!(items[0].result, BatchRequestResult::Canceled));
        assert_eq!(items[1].custom_id, "b");
        assert!(matches!(items[1].result, BatchRequestResult::Expired));
    }

    #[test]
    fn parse_jsonl_reports_line_number_on_error() {
        let body = "{\"custom_id\":\"a\",\"result\":{\"type\":\"canceled\"}}\nnot json";
        let err = parse_results_jsonl(body).unwrap_err();
        assert!(format!("{err}").contains("line 2"));
    }

    #[test]
    fn batch_request_counts_default_zero() {
        let counts = BatchRequestCounts::default();
        assert_eq!(counts.processing, 0);
        assert_eq!(counts.succeeded, 0);
    }
}
