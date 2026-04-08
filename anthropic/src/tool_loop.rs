//! High-level helper that drives the agentic "tool use" loop.
//!
//! Many real applications follow the same pattern: send a request to Claude,
//! inspect the response for `tool_use` blocks, execute the tools locally, feed
//! the results back into the conversation, and repeat until Claude returns a
//! plain-text answer (or the iteration budget is exhausted).
//!
//! [`run_tool_loop`] captures that pattern behind a minimal callback-based API
//! so callers only have to provide the tool executor.

use std::future::Future;

use crate::client::Client;
use crate::error::AnthropicError;
use crate::types::{ContentBlock, Message, MessagesRequest, MessagesResponse, Role};

/// Result of executing a single tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    /// Successful tool result.
    pub fn ok(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false }
    }

    /// Tool-level error reported back to the model.
    pub fn error(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true }
    }
}

/// Configuration for the tool loop.
#[derive(Debug, Clone)]
pub struct ToolLoopConfig {
    /// Maximum number of model round-trips before the loop bails out.
    pub max_iterations: usize,
}

impl Default for ToolLoopConfig {
    fn default() -> Self {
        Self { max_iterations: 8 }
    }
}

impl ToolLoopConfig {
    pub fn new(max_iterations: usize) -> Self {
        Self { max_iterations }
    }
}

/// Drive the "call model → run tools → feed results back" loop until Claude
/// returns a tool-free response, an error propagates, or the iteration budget
/// is exhausted.
///
/// `request` is cloned on every iteration with `messages` extended to contain
/// the running transcript, so the original tool list, tool choice, system
/// prompt, etc. are preserved throughout the loop.
///
/// The `executor` callback receives the tool name and input JSON and returns
/// either a [`ToolOutput`] (success or tool-level error, both fed back to the
/// model) or a propagated [`AnthropicError`] that aborts the loop.
pub async fn run_tool_loop<F, Fut>(
    client: &Client,
    mut request: MessagesRequest,
    mut executor: F,
    config: ToolLoopConfig,
) -> Result<MessagesResponse, AnthropicError>
where
    F: FnMut(String, serde_json::Value) -> Fut,
    Fut: Future<Output = Result<ToolOutput, AnthropicError>>,
{
    if config.max_iterations == 0 {
        return Err(AnthropicError::InvalidRequest("tool loop max_iterations must be non-zero".into()));
    }

    for _ in 0..config.max_iterations {
        let response = client.messages(request.clone()).await?;

        if !response.has_tool_use() {
            return Ok(response);
        }

        // Collect tool calls BEFORE mutating the transcript so we can run
        // every tool even if one of them errors.
        let mut pending: Vec<(String, String, serde_json::Value)> = Vec::new();
        for block in &response.content {
            if let ContentBlock::ToolUse { id, name, input, .. } = block {
                pending.push((id.clone(), name.clone(), input.clone()));
            }
        }

        // Append the assistant turn to the transcript so the next request
        // sends the full history back to Claude.
        request.messages.push(Message::new(Role::Assistant, response.content.clone()));

        let mut tool_results: Vec<ContentBlock> = Vec::with_capacity(pending.len());
        for (id, name, input) in pending {
            let output = executor(name, input).await?;
            let block = if output.is_error {
                ContentBlock::tool_result_error(id, output.content)
            } else {
                ContentBlock::tool_result_text(id, output.content)
            };
            tool_results.push(block);
        }

        request.messages.push(Message::new(Role::User, tool_results));
    }

    Err(AnthropicError::InvalidRequest(format!(
        "tool loop exceeded {} iterations without reaching a final response",
        config.max_iterations
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_output_helpers_set_is_error_correctly() {
        let ok = ToolOutput::ok("done");
        assert_eq!(ok.content, "done");
        assert!(!ok.is_error);

        let err = ToolOutput::error("nope");
        assert_eq!(err.content, "nope");
        assert!(err.is_error);
    }

    #[test]
    fn tool_loop_config_default_and_custom() {
        assert_eq!(ToolLoopConfig::default().max_iterations, 8);
        assert_eq!(ToolLoopConfig::new(3).max_iterations, 3);
    }

    #[tokio::test]
    async fn run_tool_loop_rejects_zero_max_iterations() {
        // Build a throwaway client that we expect to never be called.
        let client = Client::builder().api_key("x").api_base("http://127.0.0.1:1").build().unwrap();
        let request = crate::types::MessagesRequestBuilder::new("m", vec![Message::user("hi")], 10).build().unwrap();
        let err = run_tool_loop(&client, request, |_n, _i| async { Ok(ToolOutput::ok("x")) }, ToolLoopConfig::new(0))
            .await
            .unwrap_err();
        assert!(matches!(err, AnthropicError::InvalidRequest(_)));
    }
}
