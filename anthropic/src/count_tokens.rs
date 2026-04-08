//! Types for the `/v1/messages/count_tokens` endpoint.
//!
//! Lets callers pre-compute the input-token cost of a Messages request without
//! actually generating a response.

use serde::{Deserialize, Serialize};

use crate::error::AnthropicError;
use crate::types::{Message, MessagesRequest, SystemPrompt, ThinkingConfig, Tool, ToolChoice};

/// Request payload for `POST /v1/messages/count_tokens`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CountTokensRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
}

impl CountTokensRequest {
    /// Build a count-tokens request from an existing [`MessagesRequest`].
    ///
    /// Fields that don't affect token counting (max_tokens, temperature, etc.)
    /// are dropped.
    pub fn from_messages_request(request: &MessagesRequest) -> Self {
        Self {
            model: request.model.clone(),
            messages: request.messages.clone(),
            system: request.system.clone(),
            tools: request.tools.clone(),
            tool_choice: request.tool_choice.clone(),
            thinking: request.thinking.clone(),
        }
    }
}

/// Builder for [`CountTokensRequest`].
#[derive(Debug, Default)]
pub struct CountTokensRequestBuilder {
    model: Option<String>,
    messages: Option<Vec<Message>>,
    system: Option<SystemPrompt>,
    tools: Option<Vec<Tool>>,
    tool_choice: Option<ToolChoice>,
    thinking: Option<ThinkingConfig>,
}

impl CountTokensRequestBuilder {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self { model: Some(model.into()), messages: Some(messages), ..Default::default() }
    }

    pub fn system(mut self, system: impl Into<SystemPrompt>) -> Self {
        self.system = Some(system.into());
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

    pub fn build(self) -> Result<CountTokensRequest, AnthropicError> {
        let model = self.model.ok_or_else(|| AnthropicError::InvalidRequest("model is required".into()))?;
        if model.is_empty() {
            return Err(AnthropicError::InvalidRequest("model must not be empty".into()));
        }
        let messages = self.messages.ok_or_else(|| AnthropicError::InvalidRequest("messages is required".into()))?;
        if messages.is_empty() {
            return Err(AnthropicError::InvalidRequest("messages must not be empty".into()));
        }
        Ok(CountTokensRequest {
            model,
            messages,
            system: self.system,
            tools: self.tools,
            tool_choice: self.tool_choice,
            thinking: self.thinking,
        })
    }
}

/// Response from `POST /v1/messages/count_tokens`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CountTokensResponse {
    pub input_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MessagesRequestBuilder;
    use serde_json::json;

    #[test]
    fn builder_roundtrip_from_messages_request() {
        let req = MessagesRequestBuilder::new("claude", vec![Message::user("hi")], 100)
            .system("be nice")
            .tools(vec![Tool::new("t", "d", json!({}))])
            .tool_choice(ToolChoice::Auto)
            .thinking(ThinkingConfig::enabled(256))
            .build()
            .unwrap();

        let ct = CountTokensRequest::from_messages_request(&req);
        assert_eq!(ct.model, "claude");
        assert_eq!(ct.messages.len(), 1);
        assert!(ct.system.is_some());
        assert!(ct.tools.is_some());
        assert_eq!(ct.tool_choice, Some(ToolChoice::Auto));
        assert_eq!(ct.thinking, Some(ThinkingConfig::enabled(256)));
    }

    #[test]
    fn builder_rejects_empty_messages() {
        let err = CountTokensRequestBuilder::new("m", vec![]).build().unwrap_err();
        assert!(format!("{err}").contains("messages"));
    }

    #[test]
    fn builder_rejects_empty_model() {
        let err = CountTokensRequestBuilder::new("", vec![Message::user("hi")]).build().unwrap_err();
        assert!(format!("{err}").contains("model"));
    }

    #[test]
    fn request_serializes_minimum_fields_only() {
        let req = CountTokensRequestBuilder::new("m", vec![Message::user("hi")]).build().unwrap();
        let value = serde_json::to_value(&req).unwrap();
        let obj = value.as_object().unwrap();
        assert!(obj.contains_key("model"));
        assert!(obj.contains_key("messages"));
        assert!(!obj.contains_key("system"));
        assert!(!obj.contains_key("tools"));
        assert!(!obj.contains_key("tool_choice"));
    }

    #[test]
    fn response_deserializes() {
        let resp: CountTokensResponse = serde_json::from_value(json!({"input_tokens": 42})).unwrap();
        assert_eq!(resp.input_tokens, 42);
    }
}
