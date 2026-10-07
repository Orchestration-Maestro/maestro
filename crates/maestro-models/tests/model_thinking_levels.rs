use ModelThinkingLevel::{High, Low, Medium, Minimal, Off, Xhigh};
use maestro_models::{
    Model, ModelThinkingLevel, clamp_thinking_level, get_model, get_supported_thinking_levels,
};
use std::collections::BTreeMap;

const LEVELS: [ModelThinkingLevel; 6] = [Off, Minimal, Low, Medium, High, Xhigh];

fn assert_selection(
    model: &Model,
    available: &[ModelThinkingLevel],
    clamped: [ModelThinkingLevel; 6],
) {
    assert_eq!(get_supported_thinking_levels(model), available);
    for (request, expected) in LEVELS.into_iter().zip(clamped) {
        assert_eq!(clamp_thinking_level(model, request), expected);
    }
}

fn assert_mapping_rules(model: &mut Model) {
    model.reasoning = true;
    model.thinking_level_map = None;
    assert_selection(
        model,
        &[Off, Minimal, Low, Medium, High],
        [Off, Minimal, Low, Medium, High, High],
    );
    model.thinking_level_map = Some(BTreeMap::new());
    assert_selection(
        model,
        &[Off, Minimal, Low, Medium, High],
        [Off, Minimal, Low, Medium, High, High],
    );
    for disabled in LEVELS {
        model.thinking_level_map =
            Some([(disabled.clone(), None), (Xhigh, Some(String::new()))].into());
        if disabled == Xhigh {
            model.thinking_level_map = Some([(Xhigh, None)].into());
        }
        let expected: Vec<_> = LEVELS
            .into_iter()
            .filter(|level| *level != disabled)
            .collect();
        assert_eq!(get_supported_thinking_levels(model), expected);
    }
    let mut map: BTreeMap<_, _> = LEVELS
        .into_iter()
        .rev()
        .map(|level| (level, Some("native-custom".into())))
        .collect();
    model.thinking_level_map = Some(map.clone());
    assert_selection(model, &LEVELS, LEVELS);
    map.insert(Xhigh, Some(String::new()));
    model.thinking_level_map = Some(map);
    assert_selection(model, &LEVELS, LEVELS);
    model.thinking_level_map =
        Some([(Minimal, None), (Low, None), (Medium, None), (Xhigh, None)].into());
    assert_selection(model, &[Off, High], [Off, High, High, High, High, High]);
    model.thinking_level_map =
        Some([(Low, None), (Medium, None), (High, None), (Xhigh, None)].into());
    assert_selection(
        model,
        &[Off, Minimal],
        [Off, Minimal, Minimal, Minimal, Minimal, Minimal],
    );
    model.thinking_level_map = Some(LEVELS.into_iter().map(|level| (level, None)).collect());
    assert_selection(model, &[], [Off, Off, Off, Off, Off, Off]);
    assert_nonreasoning_maps(model);
}

fn assert_nonreasoning_maps(model: &mut Model) {
    for map in [
        None,
        Some(BTreeMap::new()),
        Some(LEVELS.into_iter().map(|level| (level, None)).collect()),
        Some(
            LEVELS
                .into_iter()
                .map(|level| (level, Some("enabled".into())))
                .collect(),
        ),
    ] {
        model.reasoning = false;
        model.thinking_level_map = map;
        assert_selection(model, &[Off], [Off, Off, Off, Off, Off, Off]);
    }
}

#[test]
fn maestro_thinking_levels_follow_model_maps() {
    for (provider, id) in [
        ("anthropic", "claude-opus-4-6"),
        ("anthropic", "claude-opus-4-7"),
        ("openai-codex", "gpt-5.4"),
        ("openai-codex", "gpt-5.5"),
        ("openrouter", "anthropic/claude-opus-4.6"),
    ] {
        let model = get_model(provider, id).unwrap();
        assert!(get_supported_thinking_levels(&model).contains(&Xhigh));
    }
    let sonnet = get_model("anthropic", "claude-sonnet-4-5").unwrap();
    assert!(!get_supported_thinking_levels(&sonnet).contains(&Xhigh));
    for (provider, id) in [
        ("deepseek", "deepseek-v4-flash"),
        ("opencode-go", "deepseek-v4-flash"),
        ("openrouter", "deepseek/deepseek-v4-flash"),
    ] {
        let model = get_model(provider, id).unwrap();
        assert_selection(
            &model,
            &[Off, High, Xhigh],
            [Off, High, High, High, High, Xhigh],
        );
    }
    let mut custom = sonnet;
    custom.provider = "outside-catalog".into();
    assert_mapping_rules(&mut custom);
}
