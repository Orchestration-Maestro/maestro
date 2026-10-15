//! Message persistence through the owning public enum.
#![cfg(test)]
use maestro_agent::{AgentMessage, CustomAgentMessages};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Open application payload without a second message union.
#[derive(Serialize, Deserialize)]
struct Custom {
    /// Supplied role.
    role: String,
    /// Arbitrary payload.
    #[serde(flatten)]
    data: serde_json::Map<String, Value>,
}
impl CustomAgentMessages for Custom {
    fn role(&self) -> &str {
        &self.role
    }
}
#[test]
fn agent_message_wire_roundtrip_keeps_default_and_custom_roles() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../maestro-app/tests/fixtures/browser_sessions.json"
    ))
    .unwrap();
    let rows = corpus["rows"].as_array().unwrap();
    let mut roles = Vec::new();
    for row in rows.iter().filter(|r| {
        r["test"] == "session_records_roundtrip_standard_browser_and_open_custom_history"
    }) {
        for value in row["input"]["data"]["messages"].as_array().unwrap() {
            let message: AgentMessage<Custom> = serde_json::from_value(value.clone()).unwrap();
            let encoded = serde_json::to_value(&message).unwrap();
            equal(&encoded, value);
            if ["user", "assistant", "toolResult"].contains(&value["role"].as_str().unwrap()) {
                let default: AgentMessage = serde_json::from_value(value.clone()).unwrap();
                assert!(default.to_message().is_some());
                let text = serde_json::to_string(&default).unwrap();
                assert_eq!(text.matches("\"role\":").count(), 1);
                equal(&serde_json::to_value(default).unwrap(), value);
                roles.push(value["role"].clone());
            }
        }
    }
    for role in ["user", "assistant", "toolResult"] {
        assert!(roles.contains(&json!(role)));
    }
    let value = json!({"role":"custom", "data":{"z":-0.0,"a":[null,false,""]}});
    let message: AgentMessage<Custom> = serde_json::from_value(value.clone()).unwrap();
    let encoded = serde_json::to_value(message).unwrap();
    assert_eq!(encoded, value);
    assert!(encoded["data"]["z"].as_f64().unwrap().is_sign_negative());
}

/// Compare numeric wire values without losing signed zero.
fn equal(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), b.as_f64().unwrap().to_bits());
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                equal(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (k, a) in a {
                equal(a, &b[k]);
            }
        }
        _ => assert_eq!(a, b),
    }
}
