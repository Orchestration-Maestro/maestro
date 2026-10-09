//! Callback-visible request projection.

use super::super::request::build_chat_payload;
use crate::{Context, MistralOptions};

#[test]
fn request_payload_preserves_raw_options() {
    let model = crate::get_model("mistral", "magistral-medium-latest").unwrap();
    let context: Context = serde_json::from_str(
        r#"{"systemPrompt":" sys ","messages":[{"role":"user","content":"Hi","timestamp":0}]}"#,
    )
    .unwrap();
    let mut options = MistralOptions::default();
    options.common.temperature = Some(-0.0);
    options.common.max_tokens = Some(0.0);
    let payload = build_chat_payload(&model, &context, &options, None).unwrap();
    assert!(payload["temperature"].as_f64().unwrap().is_sign_negative());
    assert_eq!(payload["maxTokens"], 0.0);
    assert_eq!(payload["messages"][0]["content"], " sys ");
    assert_eq!(payload["messages"][1]["content"], "Hi");
    super::request_corpus::corpus("request_payload_preserves_raw_options", 18);
}

#[test]
fn request_tools_preserve_nested_schema() {
    super::request_corpus::corpus("request_tools_preserve_nested_schema", 10);
}

#[test]
fn request_history_preserves_projection_and_content() {
    super::request_corpus::corpus("request_history_preserves_projection_and_content", 42);
}

#[test]
fn request_tool_result_notices_match_content() {
    super::request_corpus::corpus("request_tool_result_notices_match_content", 38);
}

#[test]
fn request_ids_preserve_mapping_and_collisions() {
    super::request_corpus::corpus("request_ids_preserve_mapping_and_collisions", 30);
}
