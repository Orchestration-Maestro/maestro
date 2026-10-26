//! Simple request reasoning remains distinct from agent thinking selection.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::SimpleStreamOptions;

#[test]
fn requested_thinking_levels_keep_agent_selection_unchanged() {
    for (level, wire) in [
        (crate::RequestedThinkingLevel::Minimal, "minimal"),
        (crate::RequestedThinkingLevel::Low, "low"),
        (crate::RequestedThinkingLevel::Medium, "medium"),
        (crate::RequestedThinkingLevel::High, "high"),
        (crate::RequestedThinkingLevel::Xhigh, "xhigh"),
    ] {
        let options = SimpleStreamOptions {
            reasoning: Some(level),
            ..Default::default()
        };
        let input: crate::StreamOptionsInput = options.clone().into();
        let crate::StreamOptionsInput::Simple(input) = input else {
            panic!("simple request options changed variant");
        };
        assert_eq!(input.reasoning, options.reasoning);
        assert_eq!(serde_json::to_value(options.reasoning).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<crate::RequestedThinkingLevel>(&format!("\"{wire}\"")).unwrap(),
            level
        );
    }
    let raw: crate::StreamOptionsInput = crate::ProviderStreamOptions::default().into();
    assert!(matches!(raw, crate::StreamOptionsInput::Raw(_)));
    assert!(serde_json::from_str::<crate::RequestedThinkingLevel>("\"off\"").is_err());
    let selection: crate::ThinkingLevel = crate::ModelThinkingLevel::Off;
    assert_eq!(serde_json::to_value(selection).unwrap(), "off");
}
