//! Integration tests for the optional `tracing` feature.
//!
//! Verify that `execute_bytes` emits a span with the documented fields on the
//! happy path, and that the span's `attempts` field reflects every retry.

#![cfg(feature = "tracing")]

use std::sync::{Arc, Mutex};

use anthropic::types::{Message, MessagesRequestBuilder};
use anthropic::Client;
use serde_json::json;
use tracing::field::{Field, Visit};
use tracing::instrument::WithSubscriber;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Recorded fields for a single span.
#[derive(Clone, Debug, Default)]
struct SpanRecord {
    name: String,
    method: Option<String>,
    path: Option<String>,
    status: Option<u64>,
    attempts: Option<u64>,
    duration_ms: Option<u64>,
}

#[derive(Default, Debug)]
struct CapturingVisitor {
    method: Option<String>,
    path: Option<String>,
    status: Option<u64>,
    attempts: Option<u64>,
    duration_ms: Option<u64>,
}

impl Visit for CapturingVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "method" => self.method = Some(format!("{value:?}").trim_matches('"').to_string()),
            "path" => self.path = Some(format!("{value:?}").trim_matches('"').to_string()),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "method" => self.method = Some(value.to_string()),
            "path" => self.path = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "status" => self.status = Some(value),
            "attempts" => self.attempts = Some(value),
            "duration_ms" => self.duration_ms = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_u64(field, value as u64);
    }
}

/// A minimal subscriber that captures `anthropic.http` span fields into a
/// shared vector. We only care about two properties for the tests: (1) the
/// span is emitted for every call, and (2) the `attempts` field records the
/// real retry count.
#[derive(Clone)]
struct CapturingSubscriber {
    spans: Arc<Mutex<Vec<SpanRecord>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    active: Arc<Mutex<std::collections::HashMap<u64, SpanRecord>>>,
}

impl CapturingSubscriber {
    fn new() -> (Self, Arc<Mutex<Vec<SpanRecord>>>) {
        let spans = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                spans: spans.clone(),
                next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
                active: Arc::new(Mutex::new(std::collections::HashMap::new())),
            },
            spans,
        )
    }
}

impl Subscriber for CapturingSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut visitor = CapturingVisitor::default();
        span.record(&mut visitor);
        let record = SpanRecord {
            name: span.metadata().name().to_string(),
            method: visitor.method,
            path: visitor.path,
            status: visitor.status,
            attempts: visitor.attempts,
            duration_ms: visitor.duration_ms,
        };
        self.active.lock().unwrap().insert(id, record);
        Id::from_u64(id)
    }

    fn record(&self, span: &Id, values: &Record<'_>) {
        let mut visitor = CapturingVisitor::default();
        values.record(&mut visitor);
        if let Some(record) = self.active.lock().unwrap().get_mut(&span.into_u64()) {
            if let Some(v) = visitor.method {
                record.method = Some(v);
            }
            if let Some(v) = visitor.path {
                record.path = Some(v);
            }
            if let Some(v) = visitor.status {
                record.status = Some(v);
            }
            if let Some(v) = visitor.attempts {
                record.attempts = Some(v);
            }
            if let Some(v) = visitor.duration_ms {
                record.duration_ms = Some(v);
            }
        }
    }

    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &Event<'_>) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}

    fn try_close(&self, id: Id) -> bool {
        if let Some(record) = self.active.lock().unwrap().remove(&id.into_u64()) {
            if record.name == "anthropic.http" {
                self.spans.lock().unwrap().push(record);
            }
        }
        true
    }
}

fn build_client(server: &MockServer) -> Client {
    Client::builder().api_key("test-key").api_base(server.uri()).build().expect("client")
}

fn success_body() -> serde_json::Value {
    json!({
        "id": "msg_ok",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "ok"}],
        "model": "claude",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 1, "output_tokens": 1}
    })
}

#[tokio::test]
async fn tracing_records_span_fields_on_success() {
    let (subscriber, spans) = CapturingSubscriber::new();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(&server);
    let request = MessagesRequestBuilder::new("claude", vec![Message::user("hi")], 10).no_retries().build().unwrap();

    let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
    async {
        client.messages(request).await.expect("ok");
    }
    .with_subscriber(dispatch)
    .await;

    let collected = spans.lock().unwrap().clone();
    let http_spans: Vec<_> = collected.iter().filter(|s| s.name == "anthropic.http").collect();
    assert!(!http_spans.is_empty(), "expected at least one anthropic.http span");
    let span = http_spans[0];
    assert_eq!(span.method.as_deref(), Some("POST"), "method field missing");
    assert_eq!(span.path.as_deref(), Some("/v1/messages"), "path field missing");
    assert_eq!(span.attempts, Some(1));
    assert!(span.duration_ms.is_some(), "duration_ms must be recorded");
}

#[tokio::test]
async fn tracing_records_attempt_count_on_retries() {
    let (subscriber, spans) = CapturingSubscriber::new();

    let server = MockServer::start().await;
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
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(&server);
    let request = MessagesRequestBuilder::new("claude", vec![Message::user("hi")], 10).build().unwrap();
    let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
    async {
        client.messages(request).await.expect("ok");
    }
    .with_subscriber(dispatch)
    .await;

    let collected = spans.lock().unwrap().clone();
    let http_spans: Vec<_> = collected.iter().filter(|s| s.name == "anthropic.http").collect();
    assert!(!http_spans.is_empty(), "expected at least one anthropic.http span");
    // Three attempts: two 429s + one success.
    assert_eq!(http_spans[0].attempts, Some(3));
}
