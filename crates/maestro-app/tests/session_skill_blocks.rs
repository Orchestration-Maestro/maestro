//! Skill blocks a prompt carries when a skill command expands.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::corpus;
use maestro_app::agent_session::parse_skill_block;
use serde_json::{Value, json};

#[test]
fn recorded_skill_blocks_parse_alike() {
    let cases = corpus("skill_blocks");
    for case in cases.as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let parsed = parse_skill_block(input).map_or(Value::Null, |block| {
            json!({
                "name": block.name,
                "location": block.location,
                "content": block.content,
                "userMessage": block.user_message,
            })
        });
        assert_eq!(parsed, case["parsed"], "{input:?}");
    }
    assert_eq!(cases.as_array().unwrap().len(), 14);
}
