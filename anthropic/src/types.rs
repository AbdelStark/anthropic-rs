//! Types for Anthropic's Messages API.

use serde::{Deserialize, Serialize};

use crate::error::AnthropicError;

/// Role a message belongs to in a conversation.
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// Prompt caching marker attached to content blocks, tools, and system prompts.
///
/// The Anthropic API only accepts `ephemeral` cache entries today, optionally
/// with a custom TTL ("5m" or "1h").
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum CacheControl {
    Ephemeral {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttl: Option<String>,
    },
}

impl CacheControl {
    /// Default `{"type": "ephemeral"}` cache marker.
    pub fn ephemeral() -> Self {
        Self::Ephemeral { ttl: None }
    }

    /// `ephemeral` marker with a custom TTL string (e.g. "5m" or "1h").
    pub fn ephemeral_ttl(ttl: impl Into<String>) -> Self {
        Self::Ephemeral { ttl: Some(ttl.into()) }
    }
}

/// Content block variants understood by the Messages API.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ContentBlock {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Image {
        source: ImageSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Document {
        source: DocumentSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        citations: Option<CitationsConfig>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        content: ToolResultContent,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Thinking {
        thinking: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    RedactedThinking {
        data: String,
    },
}

impl ContentBlock {
    /// Plain text block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into(), cache_control: None }
    }

    /// Image block backed by inline base64 data.
    pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Base64 { media_type: media_type.into(), data: data.into() },
            cache_control: None,
        }
    }

    /// Image block that references a remote URL.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::Image { source: ImageSource::Url { url: url.into() }, cache_control: None }
    }

    /// Document block backed by inline base64-encoded data.
    pub fn document_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Document {
            source: DocumentSource::Base64 { media_type: media_type.into(), data: data.into() },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    /// Document block backed by a remote URL.
    pub fn document_url(url: impl Into<String>) -> Self {
        Self::Document {
            source: DocumentSource::Url { url: url.into() },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    /// Document block backed by inline text (useful for text files).
    pub fn document_text(text: impl Into<String>) -> Self {
        Self::Document {
            source: DocumentSource::Text { media_type: "text/plain".into(), data: text.into() },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    /// Tool-use block representing a call requested by the model.
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: serde_json::Value) -> Self {
        Self::ToolUse { id: id.into(), name: name.into(), input, cache_control: None }
    }

    /// Successful tool result, replying with plain text.
    pub fn tool_result_text(tool_use_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            is_error: None,
            content: ToolResultContent::Text(text.into()),
            cache_control: None,
        }
    }

    /// Tool result with structured content blocks.
    pub fn tool_result_blocks(tool_use_id: impl Into<String>, blocks: Vec<ContentBlock>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            is_error: None,
            content: ToolResultContent::Blocks(blocks),
            cache_control: None,
        }
    }

    /// Tool result marked as an error.
    pub fn tool_result_error(tool_use_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            is_error: Some(true),
            content: ToolResultContent::Text(text.into()),
            cache_control: None,
        }
    }

    /// Thinking block (extended thinking output from the model).
    pub fn thinking(thinking: impl Into<String>) -> Self {
        Self::Thinking { thinking: thinking.into(), signature: None }
    }

    /// Attach a `cache_control` marker to a block that supports caching.
    ///
    /// Blocks that do not support caching (thinking / redacted thinking) are
    /// returned unchanged.
    pub fn with_cache_control(mut self, cache: CacheControl) -> Self {
        match &mut self {
            Self::Text { cache_control, .. }
            | Self::Image { cache_control, .. }
            | Self::Document { cache_control, .. }
            | Self::ToolUse { cache_control, .. }
            | Self::ToolResult { cache_control, .. } => {
                *cache_control = Some(cache);
            }
            Self::Thinking { .. } | Self::RedactedThinking { .. } => {}
        }
        self
    }

    /// Extract the textual payload from this block if it has one.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }
    }

    /// Return the tool-use id, name, and input if this block is a [`ContentBlock::ToolUse`].
    pub fn as_tool_use(&self) -> Option<(&str, &str, &serde_json::Value)> {
        match self {
            Self::ToolUse { id, name, input, .. } => Some((id.as_str(), name.as_str(), input)),
            _ => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum DocumentSource {
    Base64 { media_type: String, data: String },
    Text { media_type: String, data: String },
    Url { url: String },
    Content { content: Vec<ContentBlock> },
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CitationsConfig {
    pub enabled: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Construct a user message whose entire content is a single text block.
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, content: vec![ContentBlock::text(text)] }
    }

    /// Construct an assistant message whose entire content is a single text block.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: vec![ContentBlock::text(text)] }
    }

    /// Construct a message with arbitrary content blocks.
    pub fn new(role: Role, content: Vec<ContentBlock>) -> Self {
        Self { role, content }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum SystemPrompt {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl SystemPrompt {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn blocks(blocks: Vec<ContentBlock>) -> Self {
        Self::Blocks(blocks)
    }
}

impl From<&str> for SystemPrompt {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for SystemPrompt {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl Tool {
    /// Construct a tool definition without prompt caching.
    pub fn new(name: impl Into<String>, description: impl Into<String>, input_schema: serde_json::Value) -> Self {
        Self { name: name.into(), description: description.into(), input_schema, cache_control: None }
    }

    /// Attach a cache-control marker to this tool.
    pub fn with_cache_control(mut self, cache: CacheControl) -> Self {
        self.cache_control = Some(cache);
        self
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
    None,
}

/// Extended-thinking configuration attached to a request.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ThinkingConfig {
    Enabled { budget_tokens: u32 },
    Disabled,
}

impl ThinkingConfig {
    pub fn enabled(budget_tokens: u32) -> Self {
        Self::Enabled { budget_tokens }
    }

    pub fn disabled() -> Self {
        Self::Disabled
    }
}

/// Quality-of-service tier for a request.
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTier {
    Auto,
    StandardOnly,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
}

#[derive(Debug, Default)]
pub struct MessagesRequestBuilder {
    model: Option<String>,
    messages: Option<Vec<Message>>,
    max_tokens: Option<u32>,
    system: Option<SystemPrompt>,
    metadata: Option<Metadata>,
    stop_sequences: Option<Vec<String>>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    top_k: Option<u32>,
    stream: Option<bool>,
    tools: Option<Vec<Tool>>,
    tool_choice: Option<ToolChoice>,
    thinking: Option<ThinkingConfig>,
    service_tier: Option<ServiceTier>,
}

impl MessagesRequestBuilder {
    pub fn new(model: impl Into<String>, messages: Vec<Message>, max_tokens: u32) -> Self {
        Self { model: Some(model.into()), messages: Some(messages), max_tokens: Some(max_tokens), ..Default::default() }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = Some(messages);
        self
    }

    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn system(mut self, system: impl Into<SystemPrompt>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn stop_sequences(mut self, stop_sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(stop_sequences);
        self
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn top_p(mut self, top_p: f64) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }

    pub fn thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.thinking = Some(thinking);
        self
    }

    pub fn service_tier(mut self, tier: ServiceTier) -> Self {
        self.service_tier = Some(tier);
        self
    }

    pub fn build(self) -> Result<MessagesRequest, AnthropicError> {
        let model = self.model.ok_or_else(|| AnthropicError::InvalidRequest("model is required".into()))?;
        if model.is_empty() {
            return Err(AnthropicError::InvalidRequest("model must not be empty".into()));
        }
        let messages = self.messages.ok_or_else(|| AnthropicError::InvalidRequest("messages is required".into()))?;
        if messages.is_empty() {
            return Err(AnthropicError::InvalidRequest("messages must not be empty".into()));
        }
        let max_tokens =
            self.max_tokens.ok_or_else(|| AnthropicError::InvalidRequest("max_tokens is required".into()))?;
        if max_tokens == 0 {
            return Err(AnthropicError::InvalidRequest("max_tokens must be greater than zero".into()));
        }

        Ok(MessagesRequest {
            model,
            messages,
            max_tokens,
            system: self.system,
            metadata: self.metadata,
            stop_sequences: self.stop_sequences,
            temperature: self.temperature,
            top_p: self.top_p,
            top_k: self.top_k,
            stream: self.stream,
            tools: self.tools,
            tool_choice: self.tool_choice,
            thinking: self.thinking,
            service_tier: self.service_tier,
        })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    PauseTurn,
    Refusal,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

impl MessagesResponse {
    /// Concatenate every text block in the response into a single string.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for block in &self.content {
            if let ContentBlock::Text { text, .. } = block {
                out.push_str(text);
            }
        }
        out
    }

    /// Borrow the first text block, if any.
    pub fn first_text(&self) -> Option<&str> {
        self.content.iter().find_map(|b| b.as_text())
    }

    /// Iterate over every tool-use block in the response.
    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &str, &serde_json::Value)> {
        self.content.iter().filter_map(|b| b.as_tool_use())
    }

    /// Returns `true` if the response contains at least one tool-use block.
    pub fn has_tool_use(&self) -> bool {
        self.tool_uses().next().is_some()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
    SignatureDelta { signature: String },
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct MessageDeltaUsage {
    pub output_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct MessageDelta {
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MessagesStreamEvent {
    MessageStart { message: MessagesResponse },
    ContentBlockStart { index: usize, content_block: ContentBlock },
    ContentBlockDelta { index: usize, delta: ContentBlockDelta },
    ContentBlockStop { index: usize },
    MessageDelta { delta: MessageDelta, usage: MessageDeltaUsage },
    MessageStop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roundtrip<T>(value: &T, expected: serde_json::Value)
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let serialized = serde_json::to_value(value).expect("serialize");
        assert_eq!(serialized, expected, "serialize mismatch");
        let deserialized: T = serde_json::from_value(expected).expect("deserialize");
        assert_eq!(&deserialized, value, "roundtrip mismatch");
    }

    #[test]
    fn content_block_text_serializes_without_cache_control() {
        roundtrip(&ContentBlock::text("hello"), json!({"type": "text", "text": "hello"}));
    }

    #[test]
    fn content_block_text_with_cache_control() {
        let block = ContentBlock::text("hello").with_cache_control(CacheControl::ephemeral());
        roundtrip(
            &block,
            json!({
                "type": "text",
                "text": "hello",
                "cache_control": {"type": "ephemeral"}
            }),
        );
    }

    #[test]
    fn cache_control_ttl_variant_roundtrip() {
        roundtrip(&CacheControl::ephemeral_ttl("1h"), json!({"type": "ephemeral", "ttl": "1h"}));
    }

    #[test]
    fn content_block_image_base64() {
        let block = ContentBlock::image_base64("image/png", "YmFzZTY0");
        roundtrip(
            &block,
            json!({
                "type": "image",
                "source": {"type": "base64", "media_type": "image/png", "data": "YmFzZTY0"}
            }),
        );
    }

    #[test]
    fn content_block_image_url() {
        let block = ContentBlock::image_url("https://example.com/a.png");
        roundtrip(
            &block,
            json!({
                "type": "image",
                "source": {"type": "url", "url": "https://example.com/a.png"}
            }),
        );
    }

    #[test]
    fn content_block_tool_use_roundtrip() {
        let block = ContentBlock::tool_use("tu_1", "get_weather", json!({"city": "Paris"}));
        roundtrip(
            &block,
            json!({
                "type": "tool_use",
                "id": "tu_1",
                "name": "get_weather",
                "input": {"city": "Paris"}
            }),
        );
    }

    #[test]
    fn content_block_tool_result_text_ok() {
        let block = ContentBlock::tool_result_text("tu_1", "sunny");
        roundtrip(
            &block,
            json!({
                "type": "tool_result",
                "tool_use_id": "tu_1",
                "content": "sunny"
            }),
        );
    }

    #[test]
    fn content_block_tool_result_error() {
        let block = ContentBlock::tool_result_error("tu_1", "boom");
        roundtrip(
            &block,
            json!({
                "type": "tool_result",
                "tool_use_id": "tu_1",
                "is_error": true,
                "content": "boom"
            }),
        );
    }

    #[test]
    fn content_block_tool_result_with_blocks() {
        let block = ContentBlock::tool_result_blocks("tu_1", vec![ContentBlock::text("inner")]);
        let expected = json!({
            "type": "tool_result",
            "tool_use_id": "tu_1",
            "content": [{"type": "text", "text": "inner"}]
        });
        roundtrip(&block, expected);
    }

    #[test]
    fn content_block_thinking_with_signature() {
        let block = ContentBlock::Thinking { thinking: "musing".into(), signature: Some("sig".into()) };
        roundtrip(&block, json!({"type": "thinking", "thinking": "musing", "signature": "sig"}));
    }

    #[test]
    fn content_block_document_url_roundtrip() {
        let block = ContentBlock::document_url("https://example.com/a.pdf");
        roundtrip(
            &block,
            json!({
                "type": "document",
                "source": {"type": "url", "url": "https://example.com/a.pdf"}
            }),
        );
    }

    #[test]
    fn content_block_document_base64_with_title() {
        let block = match ContentBlock::document_base64("application/pdf", "ZGF0YQ==") {
            ContentBlock::Document { source, .. } => ContentBlock::Document {
                source,
                title: Some("Book".into()),
                context: None,
                citations: Some(CitationsConfig { enabled: true }),
                cache_control: None,
            },
            other => panic!("unexpected: {other:?}"),
        };
        roundtrip(
            &block,
            json!({
                "type": "document",
                "source": {"type": "base64", "media_type": "application/pdf", "data": "ZGF0YQ=="},
                "title": "Book",
                "citations": {"enabled": true}
            }),
        );
    }

    #[test]
    fn tool_serializes_with_optional_cache_control() {
        let tool = Tool::new("get_weather", "fetch weather", json!({"type": "object"}));
        roundtrip(
            &tool,
            json!({
                "name": "get_weather",
                "description": "fetch weather",
                "input_schema": {"type": "object"}
            }),
        );

        let cached = tool.with_cache_control(CacheControl::ephemeral());
        let value = serde_json::to_value(&cached).unwrap();
        assert_eq!(value["cache_control"], json!({"type": "ephemeral"}));
    }

    #[test]
    fn tool_choice_none_variant_serializes() {
        let choice = ToolChoice::None;
        roundtrip(&choice, json!({"type": "none"}));
    }

    #[test]
    fn thinking_config_enabled_serializes() {
        roundtrip(&ThinkingConfig::enabled(1024), json!({"type": "enabled", "budget_tokens": 1024}));
    }

    #[test]
    fn service_tier_serializes_snake_case() {
        roundtrip(&ServiceTier::StandardOnly, json!("standard_only"));
    }

    #[test]
    fn messages_request_builder_rejects_empty_messages() {
        let err = MessagesRequestBuilder::new("m", vec![], 100).build().unwrap_err();
        assert!(format!("{err}").contains("messages"));
    }

    #[test]
    fn messages_request_builder_rejects_zero_max_tokens() {
        let err = MessagesRequestBuilder::new("m", vec![Message::user("hi")], 0).build().unwrap_err();
        assert!(format!("{err}").contains("max_tokens"));
    }

    #[test]
    fn messages_request_builder_rejects_empty_model() {
        let err = MessagesRequestBuilder::new("", vec![Message::user("hi")], 10).build().unwrap_err();
        assert!(format!("{err}").contains("model"));
    }

    #[test]
    fn messages_request_builder_requires_model() {
        let err =
            MessagesRequestBuilder::default().messages(vec![Message::user("hi")]).max_tokens(10).build().unwrap_err();
        assert!(format!("{err}").contains("model"));
    }

    #[test]
    fn messages_request_builder_builds_full_request() {
        let req = MessagesRequestBuilder::new("claude", vec![Message::user("hi")], 512)
            .temperature(0.2)
            .top_p(0.9)
            .top_k(40)
            .system("be helpful")
            .thinking(ThinkingConfig::enabled(1024))
            .service_tier(ServiceTier::Auto)
            .tools(vec![Tool::new("t", "d", json!({}))])
            .tool_choice(ToolChoice::Auto)
            .build()
            .unwrap();
        assert_eq!(req.model, "claude");
        assert_eq!(req.max_tokens, 512);
        assert_eq!(req.temperature, Some(0.2));
        assert_eq!(req.top_p, Some(0.9));
        assert_eq!(req.top_k, Some(40));
        assert_eq!(req.thinking, Some(ThinkingConfig::enabled(1024)));
        assert_eq!(req.service_tier, Some(ServiceTier::Auto));
        assert!(matches!(req.system, Some(SystemPrompt::Text(_))));
    }

    #[test]
    fn messages_request_skips_none_fields_in_serialization() {
        let req = MessagesRequestBuilder::new("m", vec![Message::user("hi")], 10).build().unwrap();
        let value = serde_json::to_value(&req).unwrap();
        let obj = value.as_object().unwrap();
        assert!(obj.contains_key("model"));
        assert!(obj.contains_key("messages"));
        assert!(obj.contains_key("max_tokens"));
        assert!(!obj.contains_key("temperature"));
        assert!(!obj.contains_key("thinking"));
        assert!(!obj.contains_key("service_tier"));
    }

    #[test]
    fn stop_reason_roundtrips_pause_and_refusal() {
        roundtrip(&StopReason::PauseTurn, json!("pause_turn"));
        roundtrip(&StopReason::Refusal, json!("refusal"));
    }

    #[test]
    fn usage_deserializes_with_only_required_fields() {
        let usage: Usage = serde_json::from_value(json!({"input_tokens": 12, "output_tokens": 5})).unwrap();
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 5);
        assert!(usage.cache_creation_input_tokens.is_none());
        assert!(usage.cache_read_input_tokens.is_none());
    }

    #[test]
    fn usage_deserializes_with_cache_fields() {
        let usage: Usage = serde_json::from_value(json!({
            "input_tokens": 12,
            "output_tokens": 5,
            "cache_creation_input_tokens": 100,
            "cache_read_input_tokens": 40,
            "service_tier": "auto"
        }))
        .unwrap();
        assert_eq!(usage.cache_creation_input_tokens, Some(100));
        assert_eq!(usage.cache_read_input_tokens, Some(40));
        assert_eq!(usage.service_tier.as_deref(), Some("auto"));
    }

    #[test]
    fn messages_response_text_concats_text_blocks() {
        let resp = MessagesResponse {
            id: "msg_1".into(),
            message_type: "message".into(),
            role: Role::Assistant,
            content: vec![
                ContentBlock::text("hello "),
                ContentBlock::tool_use("tu_1", "t", json!({})),
                ContentBlock::text("world"),
            ],
            model: "claude".into(),
            stop_reason: Some(StopReason::EndTurn),
            stop_sequence: None,
            usage: Usage::default(),
        };
        assert_eq!(resp.text(), "hello world");
        assert_eq!(resp.first_text(), Some("hello "));
        assert!(resp.has_tool_use());
        let tool_uses: Vec<_> = resp.tool_uses().collect();
        assert_eq!(tool_uses.len(), 1);
        assert_eq!(tool_uses[0].0, "tu_1");
        assert_eq!(tool_uses[0].1, "t");
    }

    #[test]
    fn stream_event_thinking_delta_roundtrip() {
        let evt = MessagesStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::ThinkingDelta { thinking: "...".into() },
        };
        roundtrip(
            &evt,
            json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "thinking_delta", "thinking": "..."}
            }),
        );
    }

    #[test]
    fn stream_event_signature_delta_roundtrip() {
        let evt = MessagesStreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentBlockDelta::SignatureDelta { signature: "abc".into() },
        };
        roundtrip(
            &evt,
            json!({
                "type": "content_block_delta",
                "index": 1,
                "delta": {"type": "signature_delta", "signature": "abc"}
            }),
        );
    }

    #[test]
    fn system_prompt_text_conversion() {
        let prompt: SystemPrompt = "hello".into();
        assert_eq!(prompt, SystemPrompt::Text("hello".into()));
        let prompt: SystemPrompt = String::from("world").into();
        assert_eq!(prompt, SystemPrompt::Text("world".into()));
    }

    #[test]
    fn message_helpers_construct_single_text_blocks() {
        let m = Message::user("hi");
        assert_eq!(m.role, Role::User);
        assert_eq!(m.content.len(), 1);
        assert_eq!(m.content[0].as_text(), Some("hi"));

        let m = Message::assistant("bye");
        assert_eq!(m.role, Role::Assistant);
        assert_eq!(m.content[0].as_text(), Some("bye"));
    }
}
