//! Text-only result construction.
use super::AgentToolResult;

#[test]
fn text_result_preserves_text_details_and_absent_termination() {
    for details in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!({"answer": 42})),
    ] {
        let result = AgentToolResult::text_result("first\n雪".into(), details.clone());
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            match details {
                Some(details) => serde_json::json!({
                    "content": [{"type": "text", "text": "first\n雪"}],
                    "details": details
                }),
                None => serde_json::json!({
                    "content": [{"type": "text", "text": "first\n雪"}]
                }),
            }
        );
    }
}
