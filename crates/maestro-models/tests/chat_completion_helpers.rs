//! Gateway and account helpers shared by chat-completion adapters.

use crate::chat::TestResult;
use indexmap::IndexMap;
use maestro_models::Message;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::Model;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::chat::cloudflare::{
    CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL, CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL,
    CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL, CLOUDFLARE_WORKERS_AI_BASE_URL, is_cloudflare_provider,
    resolve_cloudflare_base_url,
};
use maestro_models::providers::chat::github_copilot_headers::{
    build_copilot_dynamic_headers, has_copilot_vision_input, infer_copilot_initiator,
};
use serde_json::json;

#[cfg(not(target_arch = "wasm32"))]
fn model_at(base_url: &str) -> TestResult<Model> {
    Ok(serde_json::from_value(json!({
        "id": "model", "name": "Model", "api": "openai-completions",
        "provider": "cloudflare-ai-gateway", "baseUrl": base_url,
        "reasoning": true, "input": ["text", "image"],
        "cost": {"input": 1, "output": 2, "cacheRead": 0.1, "cacheWrite": 1.25},
        "contextWindow": 128_000, "maxTokens": 4096
    }))?)
}

#[cfg(not(target_arch = "wasm32"))]
/// A base URL, the variables set in the child, and the resolution it must produce.
type Substitution = (
    &'static str,
    &'static [(&'static str, &'static str)],
    Result<&'static str, &'static str>,
);

#[cfg(not(target_arch = "wasm32"))]
const SUBSTITUTIONS: &[Substitution] = &[
    ("https://plain", &[], Ok("https://plain")),
    (
        "https://x/{ACCOUNT}/{GATEWAY}/{ACCOUNT}",
        &[("ACCOUNT", "a"), ("GATEWAY", "g")],
        Ok("https://x/a/g/a"),
    ),
    (
        "https://x/{ACCOUNT}",
        &[],
        Err("ACCOUNT is required for provider cloudflare-ai-gateway but is not set."),
    ),
    (
        "https://x/{ACCOUNT}",
        &[("ACCOUNT", "")],
        Err("ACCOUNT is required for provider cloudflare-ai-gateway but is not set."),
    ),
    (
        "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
        &[("CLOUDFLARE_ACCOUNT_ID", "account")],
        Err("CLOUDFLARE_GATEWAY_ID is required for provider cloudflare-ai-gateway but is not set."),
    ),
    (
        "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
        &[
            ("CLOUDFLARE_ACCOUNT_ID", "account"),
            ("CLOUDFLARE_GATEWAY_ID", "gateway"),
        ],
        Ok("https://gateway.ai.cloudflare.com/v1/account/gateway/compat"),
    ),
    (
        "https://x/{lower}/{9BAD}/{_GOOD}/{A2}",
        &[("_GOOD", "ok"), ("A2", "next")],
        Ok("https://x/{lower}/{9BAD}/ok/next"),
    ),
    (
        "https://x/{ACCOUNT}",
        &[("ACCOUNT", "a/b $& {GATEWAY}")],
        Ok("https://x/a/b $& {GATEWAY}"),
    ),
];

#[cfg(not(target_arch = "wasm32"))]
const TEST: &str = "maestro_cloudflare_resolves_account_urls";

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_cloudflare_resolves_account_urls() -> TestResult {
    let case = maestro_test_process::selection();
    if case.is_child() {
        let index = (0..SUBSTITUTIONS.len())
            .find(|index| case.runs_case(&index.to_string()))
            .ok_or("unknown child case")?;
        let (url, _, expected) = SUBSTITUTIONS[index];
        let actual = resolve_cloudflare_base_url(&model_at(url)?).map_err(|error| error.message);
        assert_eq!(actual, expected.map(str::to_owned).map_err(str::to_owned));
        return Ok(());
    }
    assert_eq!(
        [
            CLOUDFLARE_WORKERS_AI_BASE_URL,
            CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL,
            CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL,
            CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL,
        ],
        [
            "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
            "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
            "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
            "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        ]
    );
    for (provider, expected) in [
        ("cloudflare-workers-ai", true),
        ("cloudflare-ai-gateway", true),
        ("cloudflare", false),
        ("fixture", false),
    ] {
        assert_eq!(is_cloudflare_provider(provider), expected, "{provider}");
    }
    for (index, (_, variables, _)) in SUBSTITUTIONS.iter().enumerate() {
        {
            let output = maestro_test_process::TestProcess::current(TEST)?
                .nocapture()
                .env_clear()
                .envs(variables.iter().copied())
                .case(index.to_string())
                .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    Ok(())
}

fn messages(value: serde_json::Value) -> TestResult<Vec<Message>> {
    Ok(serde_json::from_value(value)?)
}

fn user(content: &serde_json::Value) -> serde_json::Value {
    json!({"role": "user", "content": content, "timestamp": 1})
}

fn assistant() -> serde_json::Value {
    json!({
        "role": "assistant", "content": [{"type": "text", "text": "x"}],
        "api": "openai-completions", "provider": "fixture", "model": "model",
        "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
        "stopReason": "stop", "timestamp": 2
    })
}

fn tool_result(content: &serde_json::Value) -> serde_json::Value {
    json!({
        "role": "toolResult", "toolCallId": "call", "toolName": "lookup",
        "content": content, "isError": false, "timestamp": 3
    })
}

fn image() -> serde_json::Value {
    json!({"type": "image", "mimeType": "image/png", "data": "aGVsbG8="})
}

#[test]
fn maestro_copilot_marks_request_initiation() -> TestResult {
    for (history, initiator) in [
        (json!([]), "user"),
        (json!([user(&json!("x"))]), "user"),
        (json!([assistant()]), "agent"),
        (json!([tool_result(&json!([]))]), "agent"),
        (json!([user(&json!("x")), assistant()]), "agent"),
        (json!([assistant(), user(&json!("x"))]), "user"),
    ] {
        assert_eq!(
            infer_copilot_initiator(&messages(history.clone())?),
            initiator,
            "{history}"
        );
    }
    Ok(())
}

#[test]
fn maestro_copilot_detects_vision_history() -> TestResult {
    let text = json!([{"type": "text", "text": "found"}]);
    for (history, initiator, vision) in [
        (json!([]), "user", false),
        (json!([user(&json!("x"))]), "user", false),
        (json!([assistant()]), "agent", false),
        (json!([tool_result(&text)]), "agent", false),
        (json!([user(&json!([image()]))]), "user", true),
        (json!([tool_result(&json!([image()]))]), "agent", true),
        (
            json!([user(&json!([image()])), assistant(), user(&json!("later"))]),
            "user",
            true,
        ),
    ] {
        let history = messages(history)?;
        assert_eq!(has_copilot_vision_input(&history), vision);
        let mut expected = IndexMap::from([
            ("X-Initiator".to_owned(), initiator.to_owned()),
            ("Openai-Intent".to_owned(), "conversation-edits".to_owned()),
        ]);
        if vision {
            expected.insert("Copilot-Vision-Request".to_owned(), "true".to_owned());
        }
        assert_eq!(build_copilot_dynamic_headers(&history, vision), expected);
    }
    let history = messages(json!([user(&json!("x"))]))?;
    assert_eq!(
        build_copilot_dynamic_headers(&history, true).get("Copilot-Vision-Request"),
        Some(&"true".to_owned())
    );
    assert_eq!(
        build_copilot_dynamic_headers(&history, false)
            .keys()
            .collect::<Vec<_>>(),
        ["X-Initiator", "Openai-Intent"]
    );
    Ok(())
}
