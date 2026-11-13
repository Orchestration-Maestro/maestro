//! Controlled-test records of the request owner.

use serde_json::{Value, json};

use crate::parameter_schema::{ParameterSchema, SchemaKind};
use crate::types::{Model, ModelCost, ModelInput};

/// A usage record whose counts and costs are all zero, as stored in a message.
#[must_use]
pub fn zero_usage() -> Value {
    json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}})
}

/// A schema declaring one authored and one plain member.
#[must_use]
pub fn mixed_schema() -> ParameterSchema {
    ParameterSchema::authored(
        json!({
            "type": "object",
            "required": ["authored", "plain"],
            "properties": {
                "authored": {"type": "string"},
                "plain": {"type": "string"},
            },
        }),
        [
            (String::new(), SchemaKind::Object),
            ("/properties/authored".to_owned(), SchemaKind::String),
        ],
    )
}

/// A model a message projection targets, with no catalog entry behind it.
#[must_use]
pub fn target_model() -> Model {
    Model {
        id: "target".into(),
        name: "Target".into(),
        api: "anthropic-messages".into(),
        provider: "target-provider".into(),
        base_url: "https://example.invalid".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 1000.0,
        max_tokens: 100.0,
        headers: None,
        compat: None,
    }
}
