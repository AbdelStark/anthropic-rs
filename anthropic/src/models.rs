//! Types for the `/v1/models` endpoint.

use serde::{Deserialize, Serialize};

/// Model entry returned by the Anthropic API.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Model {
    pub id: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub display_name: String,
    pub created_at: String,
}

/// Paginated list response returned by `GET /v1/models`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ModelList {
    pub data: Vec<Model>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
}

/// Query parameters for paginating through the model list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListModelsParams {
    pub before_id: Option<String>,
    pub after_id: Option<String>,
    pub limit: Option<u32>,
}

impl ListModelsParams {
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

    /// Serialize the parameters as `(key, value)` tuples suitable for a query
    /// string. Returns an empty vector when no parameters are set.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_deserializes() {
        let model: Model = serde_json::from_value(json!({
            "id": "claude-3-5-sonnet-20240620",
            "type": "model",
            "display_name": "Claude 3.5 Sonnet",
            "created_at": "2024-06-20T00:00:00Z"
        }))
        .unwrap();
        assert_eq!(model.id, "claude-3-5-sonnet-20240620");
        assert_eq!(model.model_type, "model");
        assert_eq!(model.display_name, "Claude 3.5 Sonnet");
    }

    #[test]
    fn model_list_deserializes_with_pagination() {
        let list: ModelList = serde_json::from_value(json!({
            "data": [
                {
                    "id": "m1",
                    "type": "model",
                    "display_name": "M1",
                    "created_at": "2024-01-01T00:00:00Z"
                }
            ],
            "has_more": true,
            "first_id": "m1",
            "last_id": "m1"
        }))
        .unwrap();
        assert_eq!(list.data.len(), 1);
        assert!(list.has_more);
        assert_eq!(list.first_id.as_deref(), Some("m1"));
    }

    #[test]
    fn model_list_deserializes_without_optional_fields() {
        let list: ModelList = serde_json::from_value(json!({"data": []})).unwrap();
        assert_eq!(list.data.len(), 0);
        assert!(!list.has_more);
        assert!(list.first_id.is_none());
    }

    #[test]
    fn list_models_params_builds_query() {
        let params = ListModelsParams::new().limit(10).after_id("abc");
        let query = params.as_query();
        assert_eq!(query, vec![("after_id", "abc".to_string()), ("limit", "10".to_string())]);
    }

    #[test]
    fn list_models_params_empty_query() {
        assert!(ListModelsParams::new().as_query().is_empty());
    }
}
