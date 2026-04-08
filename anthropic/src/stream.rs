//! Helpers for consuming a `messages_stream` response.
//!
//! [`StreamAccumulator`] folds every [`MessagesStreamEvent`] emitted by the
//! Messages SSE stream into a complete [`MessagesResponse`], handling:
//!
//! - text deltas (`text_delta` on `text` blocks)
//! - tool-use input deltas (`input_json_delta` on `tool_use` blocks — the
//!   partial JSON chunks are concatenated and re-parsed on the terminal event)
//! - extended-thinking deltas (`thinking_delta` and `signature_delta` on
//!   `thinking` blocks)
//! - `message_delta` events carrying stop reasons and usage updates
//!
//! It is designed so that callers can either stream one event at a time and
//! pull the running state, or provide an async `Stream` and receive the final
//! materialized response.

use futures_util::StreamExt;
use tokio_stream::Stream;

use crate::client::MessagesResponseStream;
use crate::error::AnthropicError;
use crate::types::{
    ContentBlock, ContentBlockDelta, MessageDelta, MessageDeltaUsage, MessagesResponse, MessagesStreamEvent, Role,
    Usage,
};

/// Running state of a partially-received streamed message.
///
/// Call [`StreamAccumulator::push`] for every event, then [`StreamAccumulator::finish`]
/// when the stream terminates.
#[derive(Debug, Clone)]
pub struct StreamAccumulator {
    message: Option<MessagesResponse>,
    /// Per-content-block buffer of incoming `input_json_delta` partial JSON.
    partial_json: Vec<String>,
    final_delta: Option<MessageDelta>,
    finished: bool,
}

impl Default for StreamAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self { message: None, partial_json: Vec::new(), final_delta: None, finished: false }
    }

    /// Return the current snapshot of the aggregated message, if `message_start`
    /// has been observed.
    pub fn snapshot(&self) -> Option<&MessagesResponse> {
        self.message.as_ref()
    }

    /// True once a `message_stop` event has been observed.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Apply a single stream event.
    pub fn push(&mut self, event: MessagesStreamEvent) -> Result<(), AnthropicError> {
        match event {
            MessagesStreamEvent::MessageStart { message } => {
                self.partial_json.clear();
                self.message = Some(message);
            }
            MessagesStreamEvent::ContentBlockStart { index, content_block } => {
                let message = self
                    .message
                    .as_mut()
                    .ok_or_else(|| AnthropicError::InvalidRequest("stream event before message_start".into()))?;
                while message.content.len() <= index {
                    message.content.push(ContentBlock::text(""));
                }
                message.content[index] = content_block;
                if self.partial_json.len() <= index {
                    self.partial_json.resize(index + 1, String::new());
                }
                self.partial_json[index].clear();
            }
            MessagesStreamEvent::ContentBlockDelta { index, delta } => {
                if self.message.is_none() {
                    return Err(AnthropicError::InvalidRequest("stream event before message_start".into()));
                }
                if self.partial_json.len() <= index {
                    self.partial_json.resize(index + 1, String::new());
                }
                let message = self.message.as_mut().expect("checked above");
                if index >= message.content.len() {
                    return Err(AnthropicError::InvalidRequest(format!(
                        "content_block_delta for unknown index {index}"
                    )));
                }
                match (&mut message.content[index], delta) {
                    (ContentBlock::Text { text, .. }, ContentBlockDelta::TextDelta { text: delta }) => {
                        text.push_str(&delta);
                    }
                    (ContentBlock::ToolUse { .. }, ContentBlockDelta::InputJsonDelta { partial_json }) => {
                        self.partial_json[index].push_str(&partial_json);
                    }
                    (ContentBlock::Thinking { thinking, .. }, ContentBlockDelta::ThinkingDelta { thinking: delta }) => {
                        thinking.push_str(&delta);
                    }
                    (
                        ContentBlock::Thinking { signature, .. },
                        ContentBlockDelta::SignatureDelta { signature: sig },
                    ) => match signature {
                        Some(existing) => existing.push_str(&sig),
                        None => *signature = Some(sig),
                    },
                    (block, delta) => {
                        return Err(AnthropicError::InvalidRequest(format!(
                            "unexpected delta {delta:?} for content block {block:?}"
                        )));
                    }
                }
            }
            MessagesStreamEvent::ContentBlockStop { index } => {
                let buffer = self.partial_json.get(index).cloned();
                let message = self
                    .message
                    .as_mut()
                    .ok_or_else(|| AnthropicError::InvalidRequest("stream event before message_start".into()))?;
                if let Some(ContentBlock::ToolUse { input, .. }) = message.content.get_mut(index) {
                    if let Some(buffer) = buffer {
                        if !buffer.is_empty() {
                            *input = serde_json::from_str(&buffer).map_err(AnthropicError::Deserialize)?;
                        }
                    }
                }
                if let Some(buf) = self.partial_json.get_mut(index) {
                    buf.clear();
                }
            }
            MessagesStreamEvent::MessageDelta { delta, usage } => {
                let message = self
                    .message
                    .as_mut()
                    .ok_or_else(|| AnthropicError::InvalidRequest("stream event before message_start".into()))?;
                if delta.stop_reason.is_some() {
                    message.stop_reason = delta.stop_reason;
                }
                if delta.stop_sequence.is_some() {
                    message.stop_sequence = delta.stop_sequence.clone();
                }
                merge_usage(&mut message.usage, &usage);
                self.final_delta = Some(delta);
            }
            MessagesStreamEvent::MessageStop => {
                self.finished = true;
            }
        }
        Ok(())
    }

    /// Consume the accumulator and return the final response.
    ///
    /// Returns [`AnthropicError::InvalidRequest`] if no `message_start` was
    /// ever observed.
    pub fn finish(self) -> Result<MessagesResponse, AnthropicError> {
        self.message.ok_or_else(|| AnthropicError::InvalidRequest("stream ended before any message_start event".into()))
    }
}

fn merge_usage(target: &mut Usage, delta: &MessageDeltaUsage) {
    target.output_tokens = delta.output_tokens;
    if let Some(input_tokens) = delta.input_tokens {
        target.input_tokens = input_tokens;
    }
    if delta.cache_creation_input_tokens.is_some() {
        target.cache_creation_input_tokens = delta.cache_creation_input_tokens;
    }
    if delta.cache_read_input_tokens.is_some() {
        target.cache_read_input_tokens = delta.cache_read_input_tokens;
    }
}

/// Drive a [`MessagesResponseStream`] (or any compatible [`Stream`]) to
/// completion and return the fully-materialized response.
pub async fn collect_stream<S>(mut stream: S) -> Result<MessagesResponse, AnthropicError>
where
    S: Stream<Item = Result<MessagesStreamEvent, AnthropicError>> + Unpin,
{
    let mut acc = StreamAccumulator::new();
    while let Some(event) = stream.next().await {
        acc.push(event?)?;
    }
    acc.finish()
}

/// Convenience alias of [`collect_stream`] that accepts the specific boxed
/// stream returned by [`crate::client::Client::messages_stream`].
pub async fn collect(stream: MessagesResponseStream) -> Result<MessagesResponse, AnthropicError> {
    collect_stream(stream).await
}

/// Seed an empty [`MessagesResponse`] that the accumulator can populate when
/// a provider sends a `message_start` without content blocks pre-populated.
pub fn empty_response(model: impl Into<String>) -> MessagesResponse {
    MessagesResponse {
        id: String::new(),
        message_type: "message".into(),
        role: Role::Assistant,
        content: Vec::new(),
        model: model.into(),
        stop_reason: None,
        stop_sequence: None,
        usage: Usage::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MessageDelta, MessageDeltaUsage, StopReason};
    use futures_util::stream;
    use serde_json::json;

    fn message_start() -> MessagesStreamEvent {
        MessagesStreamEvent::MessageStart {
            message: MessagesResponse {
                id: "msg_1".into(),
                message_type: "message".into(),
                role: Role::Assistant,
                content: Vec::new(),
                model: "claude".into(),
                stop_reason: None,
                stop_sequence: None,
                usage: Usage { input_tokens: 5, output_tokens: 0, ..Default::default() },
            },
        }
    }

    fn block_start_text(index: usize) -> MessagesStreamEvent {
        MessagesStreamEvent::ContentBlockStart { index, content_block: ContentBlock::text("") }
    }

    fn text_delta(index: usize, text: &str) -> MessagesStreamEvent {
        MessagesStreamEvent::ContentBlockDelta { index, delta: ContentBlockDelta::TextDelta { text: text.into() } }
    }

    #[test]
    fn accumulates_text_blocks_into_response() {
        let mut acc = StreamAccumulator::new();
        acc.push(message_start()).unwrap();
        acc.push(block_start_text(0)).unwrap();
        acc.push(text_delta(0, "Hello, ")).unwrap();
        acc.push(text_delta(0, "world!")).unwrap();
        acc.push(MessagesStreamEvent::ContentBlockStop { index: 0 }).unwrap();
        acc.push(MessagesStreamEvent::MessageDelta {
            delta: MessageDelta { stop_reason: Some(StopReason::EndTurn), stop_sequence: None },
            usage: MessageDeltaUsage {
                output_tokens: 12,
                input_tokens: None,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::MessageStop).unwrap();

        assert!(acc.is_finished());
        let response = acc.finish().unwrap();
        assert_eq!(response.text(), "Hello, world!");
        assert_eq!(response.stop_reason, Some(StopReason::EndTurn));
        assert_eq!(response.usage.input_tokens, 5);
        assert_eq!(response.usage.output_tokens, 12);
    }

    #[test]
    fn accumulates_tool_use_input_json_deltas() {
        let mut acc = StreamAccumulator::new();
        acc.push(message_start()).unwrap();
        acc.push(MessagesStreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::tool_use("tu_1", "get_weather", json!({})),
        })
        .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::InputJsonDelta { partial_json: "{\"city\": ".into() },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::InputJsonDelta { partial_json: "\"Paris\"}".into() },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockStop { index: 0 }).unwrap();
        acc.push(MessagesStreamEvent::MessageStop).unwrap();

        let response = acc.finish().unwrap();
        let tool_uses: Vec<_> = response.tool_uses().collect();
        assert_eq!(tool_uses.len(), 1);
        assert_eq!(tool_uses[0].2, &json!({"city": "Paris"}));
    }

    #[test]
    fn accumulates_thinking_and_signature_deltas() {
        let mut acc = StreamAccumulator::new();
        acc.push(message_start()).unwrap();
        acc.push(MessagesStreamEvent::ContentBlockStart { index: 0, content_block: ContentBlock::thinking("") })
            .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::ThinkingDelta { thinking: "Let me ".into() },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::ThinkingDelta { thinking: "think...".into() },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::SignatureDelta { signature: "sig-abc".into() },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::ContentBlockStop { index: 0 }).unwrap();
        acc.push(MessagesStreamEvent::MessageStop).unwrap();

        let response = acc.finish().unwrap();
        match &response.content[0] {
            ContentBlock::Thinking { thinking, signature } => {
                assert_eq!(thinking, "Let me think...");
                assert_eq!(signature.as_deref(), Some("sig-abc"));
            }
            other => panic!("expected Thinking, got {other:?}"),
        }
    }

    #[test]
    fn rejects_delta_before_message_start() {
        let mut acc = StreamAccumulator::new();
        let err = acc.push(text_delta(0, "oops")).unwrap_err();
        assert!(format!("{err}").contains("message_start"));
    }

    #[test]
    fn rejects_delta_for_unknown_index() {
        let mut acc = StreamAccumulator::new();
        acc.push(message_start()).unwrap();
        let err = acc.push(text_delta(5, "oops")).unwrap_err();
        assert!(format!("{err}").contains("index 5"));
    }

    #[test]
    fn rejects_mismatched_delta_kind() {
        let mut acc = StreamAccumulator::new();
        acc.push(message_start()).unwrap();
        acc.push(block_start_text(0)).unwrap();
        // text block cannot accept an input_json_delta
        let err = acc
            .push(MessagesStreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::InputJsonDelta { partial_json: "x".into() },
            })
            .unwrap_err();
        assert!(format!("{err}").contains("unexpected delta"));
    }

    #[test]
    fn finish_without_message_start_fails() {
        let acc = StreamAccumulator::new();
        let err = acc.finish().unwrap_err();
        assert!(format!("{err}").contains("message_start"));
    }

    #[test]
    fn reports_final_delta_usage_and_stop_sequence() {
        let mut acc = StreamAccumulator::new();
        acc.push(message_start()).unwrap();
        acc.push(block_start_text(0)).unwrap();
        acc.push(text_delta(0, "hi")).unwrap();
        acc.push(MessagesStreamEvent::ContentBlockStop { index: 0 }).unwrap();
        acc.push(MessagesStreamEvent::MessageDelta {
            delta: MessageDelta { stop_reason: Some(StopReason::StopSequence), stop_sequence: Some("STOP".into()) },
            usage: MessageDeltaUsage {
                output_tokens: 7,
                input_tokens: Some(11),
                cache_creation_input_tokens: Some(3),
                cache_read_input_tokens: Some(4),
            },
        })
        .unwrap();
        acc.push(MessagesStreamEvent::MessageStop).unwrap();

        let response = acc.finish().unwrap();
        assert_eq!(response.stop_reason, Some(StopReason::StopSequence));
        assert_eq!(response.stop_sequence.as_deref(), Some("STOP"));
        assert_eq!(response.usage.input_tokens, 11);
        assert_eq!(response.usage.output_tokens, 7);
        assert_eq!(response.usage.cache_creation_input_tokens, Some(3));
        assert_eq!(response.usage.cache_read_input_tokens, Some(4));
    }

    #[tokio::test]
    async fn collect_stream_builds_response_from_async_stream() {
        let events: Vec<Result<MessagesStreamEvent, AnthropicError>> = vec![
            Ok(message_start()),
            Ok(block_start_text(0)),
            Ok(text_delta(0, "hello")),
            Ok(MessagesStreamEvent::ContentBlockStop { index: 0 }),
            Ok(MessagesStreamEvent::MessageStop),
        ];
        let s = stream::iter(events);
        let response = collect_stream(s).await.unwrap();
        assert_eq!(response.text(), "hello");
    }

    #[tokio::test]
    async fn collect_stream_propagates_errors() {
        let err = AnthropicError::InvalidRequest("kaboom".into());
        let events: Vec<Result<MessagesStreamEvent, AnthropicError>> = vec![Ok(message_start()), Err(err)];
        let s = stream::iter(events);
        let result = collect_stream(s).await;
        assert!(matches!(result, Err(AnthropicError::InvalidRequest(_))));
    }

    #[test]
    fn empty_response_helper_seeds_defaults() {
        let resp = empty_response("claude");
        assert_eq!(resp.model, "claude");
        assert_eq!(resp.role, Role::Assistant);
        assert!(resp.content.is_empty());
    }
}
