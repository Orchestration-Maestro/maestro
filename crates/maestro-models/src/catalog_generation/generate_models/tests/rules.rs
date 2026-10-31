//! Authored generator rules checked from controlled feed input to the assembled catalog.
use super::super::{Catalog, assemble, models_dev};
use crate::Model;
use crate::ModelThinkingLevel as Level;
use serde_json::{Value, json};

/// Build a descriptor from the shared authored defaults.
fn model(provider: &str, id: &str, api: &str) -> Model {
    super::super::input::descriptor(id.into(), api, provider, "https://example.invalid/v1")
}

/// Assemble one feed of descriptors.
fn assembled(models: Vec<Model>) -> Catalog {
    assemble([models, Vec::new(), Vec::new()])
}

/// Borrow one assembled descriptor.
fn found<'a>(catalog: &'a Catalog, provider: &str, id: &str) -> &'a Model {
    catalog
        .get(provider)
        .and_then(|models| models.get(id))
        .unwrap_or_else(|| panic!("{provider}/{id} is missing"))
}

/// Project a models.dev document through the authored provider routes.
fn dev(document: &Value) -> Vec<Model> {
    let raw = serde_json::value::RawValue::from_string(document.to_string()).unwrap();
    models_dev::normalize(&raw, &mut Vec::new()).unwrap()
}

/// Select one projected descriptor.
fn projected<'a>(models: &'a [Model], provider: &str, id: &str) -> &'a Model {
    models
        .iter()
        .find(|model| model.provider == provider && model.id == id)
        .unwrap_or_else(|| panic!("{provider}/{id} is missing"))
}

/// Read one compatibility member.
fn compat<'a>(model: &'a Model, key: &str) -> Option<&'a Value> {
    model.compat.as_ref().and_then(|compat| compat.0.get(key))
}

/// Read the mapped value of one thinking level, distinguishing missing from unsupported.
fn level<'a>(model: &'a Model, level: &Level) -> Option<&'a Option<String>> {
    model
        .thinking_level_map
        .as_ref()
        .and_then(|map| map.get(level))
}

/// A models.dev entry the authored routes accept.
fn entry() -> Value {
    json!({"tool_call": true, "reasoning": true, "limit": {"context": 1000, "output": 100}})
}

#[test]
fn openai_responses_models_that_can_disable_reasoning_map_off_to_none() {
    for id in [
        "gpt-5.1",
        "gpt-5.2",
        "gpt-5.3-codex",
        "gpt-5.4",
        "gpt-5.4-mini",
        "gpt-5.4-nano",
        "gpt-5.5",
    ] {
        let catalog = assembled(vec![model("openai", id, "openai-responses")]);
        assert_eq!(
            level(found(&catalog, "openai", id), &Level::Off),
            Some(&Some("none".into())),
            "{id}"
        );
    }
    let catalog = assembled(vec![
        model("openai", "gpt-5", "openai-responses"),
        model("openai", "gpt-5.4-pro", "openai-responses"),
    ]);
    for id in ["gpt-5", "gpt-5.4-pro"] {
        assert_eq!(
            level(found(&catalog, "openai", id), &Level::Off),
            Some(&None),
            "{id}"
        );
    }
}

#[test]
fn grok_3_models_are_added_when_the_feed_omits_them() {
    let catalog = assembled(Vec::new());
    for (id, name) in [("grok-3", "Grok 3"), ("grok-3-fast", "Grok 3 Fast")] {
        let added = found(&catalog, "xai", id);
        assert_eq!(added.name, name);
        assert_eq!(added.api, "openai-completions");
        assert_eq!(added.context_window.to_bits(), 131_072.0_f64.to_bits());
        assert_eq!(added.max_tokens.to_bits(), 8192.0_f64.to_bits());
    }
    let mut feed = model("xai", "grok-3", "openai-completions");
    feed.name = "Feed Grok 3".into();
    let catalog = assembled(vec![feed]);
    assert_eq!(found(&catalog, "xai", "grok-3").name, "Feed Grok 3");
}

#[test]
fn xiaomi_variants_route_through_openai_completions_v1() {
    let models = dev(&json!({"xiaomi": {"models": {"mimo-v2-pro": entry()}}}));
    let mut seen = Vec::new();
    for (provider, base) in [
        ("xiaomi", "https://api.xiaomimimo.com/v1"),
        (
            "xiaomi-token-plan-cn",
            "https://token-plan-cn.xiaomimimo.com/v1",
        ),
        (
            "xiaomi-token-plan-ams",
            "https://token-plan-ams.xiaomimimo.com/v1",
        ),
        (
            "xiaomi-token-plan-sgp",
            "https://token-plan-sgp.xiaomimimo.com/v1",
        ),
    ] {
        let model = projected(&models, provider, "mimo-v2-pro");
        assert_eq!(model.api, "openai-completions", "{provider}");
        assert_eq!(model.base_url, base, "{provider}");
        seen.push(provider);
    }
    assert_eq!(seen.len(), models.len());
}

#[test]
fn xiaomi_models_replay_reasoning_content_with_the_deepseek_format() {
    let models = dev(&json!({"xiaomi": {"models": {"mimo-v2-pro": entry()}}}));
    for model in &models {
        assert_eq!(
            compat(model, "requiresReasoningContentOnAssistantMessages"),
            Some(&Value::Bool(true)),
            "{}",
            model.provider
        );
        assert_eq!(
            compat(model, "thinkingFormat"),
            Some(&Value::from("deepseek")),
            "{}",
            model.provider
        );
    }
}

#[test]
fn token_plan_catalogs_omit_mimo_v2_flash() {
    let models = dev(&json!({"xiaomi": {"models": {"mimo-v2-flash": entry()}}}));
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].provider, "xiaomi");
}

#[test]
fn gpt_5_5_pro_hides_off_minimal_and_low() {
    for (provider, id) in [
        ("openai", "gpt-5.5-pro"),
        ("openrouter", "openai/gpt-5.5-pro"),
    ] {
        let api = if provider == "openai" {
            "openai-responses"
        } else {
            "openai-completions"
        };
        let catalog = assembled(vec![model(provider, id, api)]);
        let pro = found(&catalog, provider, id);
        for hidden in [Level::Off, Level::Minimal, Level::Low] {
            assert_eq!(level(pro, &hidden), Some(&None), "{id} {hidden:?}");
        }
    }
}

#[test]
fn direct_gpt_5_5_hides_minimal_only_for_the_openai_provider() {
    let catalog = assembled(vec![
        model("openai", "gpt-5.5", "openai-responses"),
        model("openai", "gpt-5.4", "openai-responses"),
        model("github-copilot", "gpt-5.5", "openai-responses"),
    ]);
    assert_eq!(
        level(found(&catalog, "openai", "gpt-5.5"), &Level::Minimal),
        Some(&None)
    );
    assert_eq!(
        level(found(&catalog, "openai", "gpt-5.4"), &Level::Minimal),
        None
    );
    assert_eq!(
        level(
            found(&catalog, "github-copilot", "gpt-5.5"),
            &Level::Minimal
        ),
        None
    );
}

#[test]
fn openrouter_deepseek_v4_maps_xhigh_to_its_own_xhigh() {
    let catalog = assembled(vec![
        model(
            "openrouter",
            "deepseek/deepseek-v4-pro",
            "openai-completions",
        ),
        model("deepseek", "deepseek-v4-pro", "openai-completions"),
    ]);
    let router = found(&catalog, "openrouter", "deepseek/deepseek-v4-pro");
    assert_eq!(level(router, &Level::Xhigh), Some(&Some("xhigh".into())));
    assert_eq!(level(router, &Level::High), Some(&Some("high".into())));
    assert_eq!(
        compat(router, "requiresReasoningContentOnAssistantMessages"),
        Some(&Value::Bool(true))
    );
    assert_eq!(compat(router, "thinkingFormat"), None);
    let direct = found(&catalog, "deepseek", "deepseek-v4-pro");
    assert_eq!(level(direct, &Level::Xhigh), Some(&Some("max".into())));
    assert_eq!(
        compat(direct, "thinkingFormat"),
        Some(&Value::from("deepseek"))
    );
}

#[test]
fn opencode_kimi_k2_6_uses_the_thinking_object_format() {
    let document = json!({
        "opencode": {"models": {"kimi-k2.6": entry()}},
        "opencode-go": {"models": {"kimi-k2.6": entry()}},
    });
    let catalog = assembled(dev(&document));
    for provider in ["opencode", "opencode-go"] {
        let kimi = found(&catalog, provider, "kimi-k2.6");
        assert_eq!(
            compat(kimi, "thinkingFormat"),
            Some(&Value::from("deepseek")),
            "{provider}"
        );
        assert_eq!(
            compat(kimi, "supportsReasoningEffort"),
            Some(&Value::Bool(false)),
            "{provider}"
        );
    }
    assert_eq!(
        level(found(&catalog, "opencode-go", "kimi-k2.6"), &Level::Off),
        Some(&Some("none".into()))
    );
}

#[test]
fn openrouter_kimi_k2_6_sends_system_instructions_and_replays_reasoning() {
    let catalog = assembled(vec![
        model("openrouter", "moonshotai/kimi-k2.6", "openai-completions"),
        model(
            "openrouter",
            "moonshotai/kimi-k2.6:nitro",
            "openai-completions",
        ),
        model("openrouter", "moonshotai/kimi-k2.5", "openai-completions"),
    ]);
    for id in ["moonshotai/kimi-k2.6", "moonshotai/kimi-k2.6:nitro"] {
        let kimi = found(&catalog, "openrouter", id);
        assert_eq!(
            compat(kimi, "supportsDeveloperRole"),
            Some(&Value::Bool(false)),
            "{id}"
        );
        assert_eq!(
            compat(kimi, "requiresReasoningContentOnAssistantMessages"),
            Some(&Value::Bool(true)),
            "{id}"
        );
    }
    assert_eq!(
        compat(
            found(&catalog, "openrouter", "moonshotai/kimi-k2.5"),
            "supportsDeveloperRole"
        ),
        None
    );
}

#[test]
fn direct_minimax_models_keep_upstream_limits() {
    let mut feed = model("minimax", "MiniMax-M2.7", "anthropic-messages");
    feed.context_window = 1_048_576.0;
    feed.max_tokens = 65_536.0;
    let catalog = assembled(vec![feed]);
    let kept = found(&catalog, "minimax", "MiniMax-M2.7");
    assert_eq!(kept.context_window.to_bits(), 1_048_576.0_f64.to_bits());
    assert_eq!(kept.max_tokens.to_bits(), 65_536.0_f64.to_bits());
}

#[test]
fn moonshot_providers_use_the_deepseek_thinking_format() {
    let document = json!({
        "moonshotai": {"models": {"kimi-k2.6": entry()}},
        "moonshotai-cn": {"models": {"kimi-k2.6": entry()}},
    });
    for provider in ["moonshotai", "moonshotai-cn"] {
        let models = dev(&document);
        let kimi = projected(&models, provider, "kimi-k2.6");
        assert_eq!(
            compat(kimi, "thinkingFormat"),
            Some(&Value::from("deepseek")),
            "{provider}"
        );
    }
}

#[test]
fn gateway_completion_models_send_max_tokens() {
    let document = json!({
        "opencode": {"models": {
            "glm-5": {"tool_call": true, "provider": {"npm": "@ai-sdk/openai-compatible"}},
            "gpt-5.4": {"tool_call": true, "provider": {"npm": "@ai-sdk/openai"}},
        }},
    });
    let models = dev(&document);
    assert_eq!(
        compat(projected(&models, "opencode", "glm-5"), "maxTokensField"),
        Some(&Value::from("max_tokens"))
    );
    assert_eq!(
        compat(projected(&models, "opencode", "gpt-5.4"), "maxTokensField"),
        None
    );
}

#[test]
fn extended_context_copilot_models_have_a_one_million_window_for_that_provider() {
    let extended = [
        "claude-fable-5",
        "claude-opus-4.6",
        "claude-opus-4.7",
        "claude-opus-4.8",
        "claude-sonnet-4.6",
        "claude-sonnet-5",
        "gpt-5.3-codex",
        "gpt-5.4",
        "gpt-5.5",
    ];
    let mut feed = Vec::new();
    for id in extended {
        feed.push(model("github-copilot", id, "openai-completions"));
        feed.push(model("other", id, "openai-completions"));
    }
    let catalog = assembled(feed);
    for id in extended {
        let window = found(&catalog, "github-copilot", id).context_window;
        assert_eq!(window.to_bits(), 1_000_000.0_f64.to_bits(), "{id}");
        let other = found(&catalog, "other", id).context_window;
        assert_eq!(other.to_bits(), 4096.0_f64.to_bits(), "{id} elsewhere");
    }
}

#[test]
fn kimi_k2p7_alias_normalizes_to_the_canonical_coding_model() {
    let alias = json!({"kimi-for-coding": {"models": {"k2p7": entry()}}});
    let models = dev(&alias);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "kimi-for-coding");
    let both = json!({"kimi-for-coding": {"models": {
        "k2p7": entry(),
        "kimi-for-coding": entry(),
    }}});
    let models = dev(&both);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "kimi-for-coding");
}

#[test]
fn gateway_workers_models_are_derived_when_the_gateway_feed_drops_them() {
    let document = json!({
        "cloudflare-workers-ai": {"models": {
            "@cf/kept": entry(),
            "@cf/dropped": entry(),
        }},
        "cloudflare-ai-gateway": {"models": {
            "workers-ai/@cf/kept": entry(),
        }},
    });
    let models = dev(&document);
    let gateway: Vec<_> = models
        .iter()
        .filter(|model| model.provider == "cloudflare-ai-gateway")
        .collect();
    assert_eq!(gateway.len(), 2);
    let derived = projected(&models, "cloudflare-ai-gateway", "workers-ai/@cf/dropped");
    assert_eq!(derived.api, "openai-completions");
    assert_eq!(
        compat(derived, "sendSessionAffinityHeaders"),
        Some(&Value::Bool(true))
    );
}
