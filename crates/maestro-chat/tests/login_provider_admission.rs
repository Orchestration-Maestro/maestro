//! Which providers `/login` offers for API-key sign-in.
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::BUILT_IN_PROVIDER_DISPLAY_NAMES;
use maestro_chat::interactive_mode::is_api_key_login_provider;
use std::collections::HashSet;

/// Collects borrowed identifiers into the owned set the predicate reads.
fn ids(values: &[&str]) -> HashSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Subscription providers registered beside the shipped ones.
const SUBSCRIPTIONS: &[&str] = &["anthropic", "github-copilot", "custom-oauth"];

/// Each shipped catalog provider and custom neighbour, checked against the default
/// built-in set: labelled providers keep key sign-in, unlabelled shipped ones lose it.
const DEFAULT_SET_CASES: &[(&str, bool)] = &[
    ("amazon-bedrock", true),
    ("anthropic", true),
    ("azure-openai-responses", true),
    ("cerebras", true),
    ("cloudflare-ai-gateway", true),
    ("cloudflare-workers-ai", true),
    ("deepseek", true),
    ("fireworks", true),
    ("github-copilot", false),
    ("google", true),
    ("google-vertex", true),
    ("groq", true),
    ("huggingface", true),
    ("kimi-coding", true),
    ("minimax", true),
    ("minimax-cn", true),
    ("mistral", true),
    ("moonshotai", true),
    ("moonshotai-cn", true),
    ("openai", true),
    ("openai-codex", false),
    ("opencode", true),
    ("opencode-go", true),
    ("openrouter", true),
    ("vercel-ai-gateway", true),
    ("xai", true),
    ("xiaomi", true),
    ("xiaomi-token-plan-ams", true),
    ("xiaomi-token-plan-cn", true),
    ("xiaomi-token-plan-sgp", true),
    ("zai", true),
    ("custom-oauth", false),
    ("custom-key", true),
];

/// Providers checked against a supplied built-in set.
const SUPPLIED_SET_CASES: &[(&str, bool)] = &[
    ("anthropic", true),
    ("openai", true),
    ("github-copilot", false),
    ("amazon-bedrock", true),
    ("custom-oauth", false),
    ("custom-api", true),
];

/// Shipped providers in the supplied built-in set.
const SUPPLIED_BUILT_INS: &[&str] = &["anthropic", "github-copilot", "amazon-bedrock", "openai"];

/// A provider identifier no shipped table names.
const LABEL_BOUNDARY: &str = "eligibility-label-boundary";

#[test]
/// Key sign-in keeps every shipped key provider and every custom key provider, and
/// never offers a provider that only signs in by subscription.
fn login_key_admission_preserves_subscription_and_key_capabilities() {
    let subscriptions = ids(SUBSCRIPTIONS);
    for &(provider, expected) in DEFAULT_SET_CASES {
        let admitted = is_api_key_login_provider(provider, &subscriptions, None);
        assert_eq!(admitted, expected, "default built-in set: {provider}");
    }
    let built_ins = ids(SUPPLIED_BUILT_INS);
    for &(provider, expected) in SUPPLIED_SET_CASES {
        let admitted = is_api_key_login_provider(provider, &subscriptions, Some(&built_ins));
        assert_eq!(admitted, expected, "supplied built-in set: {provider}");
    }
    assert!(is_api_key_login_provider("", &ids(&[]), None));
    assert!(is_api_key_login_provider(
        "Anthropic",
        &ids(&["anthropic"]),
        None
    ));
    let blocked = ids(&["blocked-custom"]);
    assert!(!is_api_key_login_provider(
        "blocked-custom",
        &ids(&[]),
        Some(&blocked)
    ));
    let subscription_only = ids(&["custom-subscription"]);
    assert!(!is_api_key_login_provider(
        "custom-subscription",
        &subscription_only,
        Some(&ids(&[])),
    ));
    let boundary = ids(&[LABEL_BOUNDARY]);
    for (label, expected) in [("", false), ("Visible name", true)] {
        BUILT_IN_PROVIDER_DISPLAY_NAMES
            .write()
            .unwrap()
            .insert(LABEL_BOUNDARY.into(), label.into());
        let admitted = is_api_key_login_provider(LABEL_BOUNDARY, &boundary, Some(&boundary));
        assert_eq!(admitted, expected, "display label {label:?}");
    }
    BUILT_IN_PROVIDER_DISPLAY_NAMES
        .write()
        .unwrap()
        .shift_remove(LABEL_BOUNDARY);
}
