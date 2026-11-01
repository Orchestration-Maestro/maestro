#![cfg(test)]
//! Exact user-facing thinking-level spellings.
use maestro_request::types::ModelThinkingLevel;
use serde::Deserialize;
/// One recorded parse decision.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    input: Input,
    expected: bool,
}
/// Authored operation and spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    op: String,
    value: String,
}
#[test]
fn model_thinking_level_parses_only_exact_spellings() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/thinking_levels.json")).unwrap();
    let mut admitted = Vec::new();
    for case in cases {
        assert_eq!(case.input.op, "level");
        let parsed = case.input.value.parse::<ModelThinkingLevel>();
        assert_eq!(parsed.is_ok(), case.expected, "{}", case.id);
        let decoded = serde_json::from_value::<ModelThinkingLevel>(case.input.value.clone().into());
        assert_eq!(parsed.ok(), decoded.ok(), "{}", case.id);
        if case.expected {
            admitted.push(case.input.value);
        } else {
            assert_eq!(
                case.input
                    .value
                    .parse::<ModelThinkingLevel>()
                    .unwrap_err()
                    .to_string(),
                "invalid model thinking level"
            );
        }
    }
    assert_eq!(
        admitted,
        ["off", "minimal", "low", "medium", "high", "xhigh"]
    );
}

#[test]
fn model_thinking_level_display_matches_wire_name() {
    for (level, name) in [
        (ModelThinkingLevel::Off, "off"),
        (ModelThinkingLevel::Minimal, "minimal"),
        (ModelThinkingLevel::Low, "low"),
        (ModelThinkingLevel::Medium, "medium"),
        (ModelThinkingLevel::High, "high"),
        (ModelThinkingLevel::Xhigh, "xhigh"),
    ] {
        assert_eq!(level.to_string(), name);
    }
}
