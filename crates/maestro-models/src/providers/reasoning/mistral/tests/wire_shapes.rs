//! Literal options and request shapes.

use crate::{MistralOptions, MistralPromptMode, MistralReasoningEffort, MistralToolChoice};

#[test]
fn wire_raw_options_keep_distinct_literals() {
    let cases: Vec<super::Case> =
        serde_json::from_str(include_str!("fixtures/reasoning-wire.json")).unwrap();
    let mut count = 0;
    for case in cases
        .into_iter()
        .filter(|case| case.test == "wire_raw_options_keep_distinct_literals")
    {
        let choice = match case.input["toolChoice"].as_str() {
            Some("auto") => MistralToolChoice::Auto,
            Some("none") => MistralToolChoice::None,
            Some("any") => MistralToolChoice::Any,
            Some("required") => MistralToolChoice::Required,
            None => MistralToolChoice::Function {
                name: case.input["toolChoice"]["function"]["name"]
                    .as_str()
                    .unwrap()
                    .into(),
            },
            _ => panic!("unexpected fixture choice"),
        };
        let options = MistralOptions {
            tool_choice: Some(choice),
            reasoning_effort: case.input.get("reasoningEffort").map(|value| {
                match value.as_str().unwrap() {
                    "none" => MistralReasoningEffort::None,
                    "high" => MistralReasoningEffort::High,
                    _ => panic!("unexpected fixture effort"),
                }
            }),
            prompt_mode: case.input.get("promptMode").map(|value| {
                assert_eq!(value, "reasoning");
                MistralPromptMode::Reasoning
            }),
            ..Default::default()
        };
        let mut input = serde_json::json!({"model":"raw-options", "messages":[]});
        input["toolChoice"] = serde_json::to_value(options.tool_choice.unwrap()).unwrap();
        if let Some(effort) = options.reasoning_effort {
            input["reasoningEffort"] = serde_json::to_value(effort).unwrap();
        }
        if let Some(prompt) = options.prompt_mode {
            input["promptMode"] = serde_json::to_value(prompt).unwrap();
        }
        assert_eq!(input, case.input);
        super::check(&input, &case.expected);
        count += 1;
    }
    assert_eq!(count, 30);
}

#[test]
fn wire_records_require_objects() {
    super::corpus("wire_records_require_objects", 88);
}

#[test]
fn wire_nested_unions_keep_selected_data() {
    super::corpus("wire_nested_unions_keep_selected_data", 9);
}

#[test]
fn wire_thinking_parts_choose_richest_shape() {
    super::corpus("wire_thinking_parts_choose_richest_shape", 37);
}

#[test]
fn wire_preserves_literal_dictionary_keys() {
    super::corpus("wire_preserves_literal_dictionary_keys", 5);
}

#[test]
fn wire_unknown_members_do_not_add_depth_limits() {
    for depth in [126, 127, 128, 256] {
        let mut nested = serde_json::json!({"leaf":"retained"});
        for _ in 0..depth {
            nested = serde_json::json!({"nested":nested});
        }
        for member in ["unknown", "metadata"] {
            let mut input = serde_json::json!({"model":"m", "messages":[]});
            input[member] = nested.clone();
            let metadata = if member == "metadata" {
                format!(
                    ",\"metadata\":{}{{\"leaf\":\"retained\"}}{}",
                    "{\"nested\":".repeat(depth),
                    "}".repeat(depth)
                )
            } else {
                String::new()
            };
            super::check(
                &input,
                &super::Expected::Json {
                    json: format!("{{\"model\":\"m\",\"stream\":true{metadata},\"messages\":[]}}"),
                },
            );
            assert_eq!(input[member], nested);
        }
    }
}
