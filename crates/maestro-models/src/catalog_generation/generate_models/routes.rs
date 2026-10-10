//! Authored protocol routes and compatibility data.
use super::input;
use crate::providers::chat::cloudflare::{
    CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL, CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL,
    CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL, CLOUDFLARE_WORKERS_AI_BASE_URL,
};
use crate::providers::json_text::member;
use crate::{Model, ModelCompat, OpenAICompletionsCompat};
use serde_json::value::RawValue;

/// Source key, output provider, protocol and endpoint in authored encounter order.
pub(super) const PROVIDERS: &[(&str, &str, &str, &str)] = &[
    (
        "amazon-bedrock",
        "amazon-bedrock",
        "bedrock-converse-stream",
        "https://bedrock-runtime.us-east-1.amazonaws.com",
    ),
    (
        "anthropic",
        "anthropic",
        "anthropic-messages",
        "https://api.anthropic.com",
    ),
    (
        "google",
        "google",
        "google-generative-ai",
        "https://generativelanguage.googleapis.com/v1beta",
    ),
    (
        "openai",
        "openai",
        "openai-responses",
        "https://api.openai.com/v1",
    ),
    (
        "groq",
        "groq",
        "openai-completions",
        "https://api.groq.com/openai/v1",
    ),
    (
        "cerebras",
        "cerebras",
        "openai-completions",
        "https://api.cerebras.ai/v1",
    ),
    (
        "cloudflare-workers-ai",
        "cloudflare-workers-ai",
        "openai-completions",
        CLOUDFLARE_WORKERS_AI_BASE_URL,
    ),
    (
        "cloudflare-ai-gateway",
        "cloudflare-ai-gateway",
        "openai-completions",
        CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL,
    ),
    ("xai", "xai", "openai-completions", "https://api.x.ai/v1"),
    (
        "zai-coding-plan",
        "zai",
        "openai-completions",
        "https://api.z.ai/api/coding/paas/v4",
    ),
    (
        "mistral",
        "mistral",
        "mistral-conversations",
        "https://api.mistral.ai",
    ),
    (
        "huggingface",
        "huggingface",
        "openai-completions",
        "https://router.huggingface.co/v1",
    ),
    (
        "fireworks-ai",
        "fireworks",
        "anthropic-messages",
        "https://api.fireworks.ai/inference",
    ),
    (
        "opencode",
        "opencode",
        "openai-completions",
        "https://opencode.ai/zen",
    ),
    (
        "opencode-go",
        "opencode-go",
        "openai-completions",
        "https://opencode.ai/zen/go",
    ),
    (
        "github-copilot",
        "github-copilot",
        "openai-completions",
        "https://api.individual.githubcopilot.com",
    ),
    (
        "minimax",
        "minimax",
        "anthropic-messages",
        "https://api.minimax.io/anthropic",
    ),
    (
        "minimax-cn",
        "minimax-cn",
        "anthropic-messages",
        "https://api.minimaxi.com/anthropic",
    ),
    (
        "kimi-for-coding",
        "kimi-coding",
        "anthropic-messages",
        "https://api.kimi.com/coding",
    ),
    (
        "moonshotai",
        "moonshotai",
        "openai-completions",
        "https://api.moonshot.ai/v1",
    ),
    (
        "moonshotai-cn",
        "moonshotai-cn",
        "openai-completions",
        "https://api.moonshot.cn/v1",
    ),
    (
        "xiaomi",
        "xiaomi",
        "anthropic-messages",
        "https://api.xiaomimimo.com/anthropic",
    ),
    (
        "xiaomi",
        "xiaomi-token-plan-cn",
        "anthropic-messages",
        "https://token-plan-cn.xiaomimimo.com/anthropic",
    ),
    (
        "xiaomi",
        "xiaomi-token-plan-ams",
        "anthropic-messages",
        "https://token-plan-ams.xiaomimimo.com/anthropic",
    ),
    (
        "xiaomi",
        "xiaomi-token-plan-sgp",
        "anthropic-messages",
        "https://token-plan-sgp.xiaomimimo.com/anthropic",
    ),
];
/// Select a route before inspecting optional model metadata.
pub(super) fn route(mut model: Model, raw: &RawValue) -> Option<Model> {
    match model.provider.as_str() {
        "amazon-bedrock" => {
            if model.id.starts_with("ai21.jamba")
                || model.id.starts_with("mistral.mistral-7b-instruct-v0")
            {
                return None;
            }
            if model.id.starts_with("eu.") {
                model.base_url = "https://bedrock-runtime.eu-central-1.amazonaws.com".into();
            }
        }
        "cloudflare-workers-ai" => completion_compat(
            &mut model,
            OpenAICompletionsCompat {
                send_session_affinity_headers: Some(true),
                ..Default::default()
            },
        ),
        "cloudflare-ai-gateway" => cloudflare(&mut model)?,
        "zai" => zai(&mut model),
        "huggingface" => completion_compat(
            &mut model,
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ),
        "opencode" | "opencode-go" => {
            if deprecated(raw) {
                return None;
            }
            opencode(&mut model, raw);
        }
        "github-copilot" => {
            if deprecated(raw) {
                return None;
            }
            copilot(&mut model);
        }
        "kimi-coding" => model.headers = Some([("User-Agent".into(), "KimiCLI/1.5".into())].into()),
        "moonshotai" | "moonshotai-cn" => completion_compat(
            &mut model,
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(crate::MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ),
        _ => {}
    }
    Some(model)
}
/// Attach typed completion compatibility.
fn completion_compat(model: &mut Model, compat: OpenAICompletionsCompat) {
    model.compat = Some(ModelCompat::from(compat));
}
/// Exact deprecated flag used by the two authored provider groups.
fn deprecated(raw: &RawValue) -> bool {
    input::text(member(raw, "status")).is_ok_and(|status| status == "deprecated")
}
/// Route gateway upstreams while retaining Workers AI's full identity.
fn cloudflare(model: &mut Model) -> Option<()> {
    let (upstream, native) = model.id.split_once('/')?;
    let (api, url) = match upstream {
        "openai" => ("openai-responses", CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL),
        "anthropic" => (
            "anthropic-messages",
            CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL,
        ),
        "workers-ai" => ("openai-completions", CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL),
        _ => return None,
    };
    if upstream == "workers-ai" {
        completion_compat(
            model,
            OpenAICompletionsCompat {
                send_session_affinity_headers: Some(true),
                ..Default::default()
            },
        );
    } else {
        model.id = native.into();
        model.name = model.id.clone();
    }
    model.api = api.into();
    model.base_url = url.into();
    Some(())
}
/// Select the SDK route and repair the three named Go model routes.
fn opencode(model: &mut Model, raw: &RawValue) {
    let npm = input::text(input::field(raw, "provider", "npm")).unwrap_or_default();
    model.api = match npm.as_str() {
        "@ai-sdk/openai" => "openai-responses",
        "@ai-sdk/anthropic" => "anthropic-messages",
        "@ai-sdk/google" => "google-generative-ai",
        _ => "openai-completions",
    }
    .into();
    let mut compat = OpenAICompletionsCompat::default();
    let alibaba = npm == "@ai-sdk/alibaba";
    if alibaba {
        compat.cache_control_format = Some(crate::CacheControlFormat::Anthropic);
    }
    let qwen = model.provider == "opencode-go"
        && ["qwen3.5-plus", "qwen3.6-plus"].contains(&model.id.as_str());
    if model.provider == "opencode-go" && (qwen || model.id == "minimax-m2.7") {
        model.api = "openai-completions".into();
    }
    if qwen {
        compat.thinking_format = Some(crate::ThinkingFormat::Qwen);
    }
    if model.api != "anthropic-messages" {
        model.base_url.push_str("/v1");
    }
    if alibaba || qwen {
        completion_compat(model, compat);
    }
}
/// Select the provider-specific protocol, headers and compatibility.
fn copilot(model: &mut Model) {
    let claude = ["claude-haiku-4", "claude-sonnet-4", "claude-opus-4"]
        .iter()
        .any(|prefix| {
            model
                .id
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', '-']))
        });
    model.api = if claude {
        "anthropic-messages"
    } else if model.id.starts_with("gpt-5") || model.id.starts_with("oswe") {
        "openai-responses"
    } else {
        "openai-completions"
    }
    .into();
    model.context_window = 128_000.0;
    model.max_tokens = 8192.0;
    model.headers = Some(
        [
            ("User-Agent", "GitHubCopilotChat/0.35.0"),
            ("Editor-Version", "vscode/1.107.0"),
            ("Editor-Plugin-Version", "copilot-chat/0.35.0"),
            ("Copilot-Integration-Id", "vscode-chat"),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect(),
    );
    if model.api == "openai-completions" {
        completion_compat(
            model,
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        );
    } else if claude
        && ["claude-haiku-4.5", "claude-sonnet-4", "claude-sonnet-4.5"].contains(&model.id.as_str())
    {
        model.compat = Some(ModelCompat::from(crate::AnthropicMessagesCompat {
            supports_eager_tool_input_streaming: Some(false),
            ..Default::default()
        }));
    }
}

/// Retain zAI's thinking convention and exact legacy streaming exclusions.
fn zai(model: &mut Model) {
    let legacy =
        ["glm-4.5", "glm-4.5-air", "glm-4.5-flash", "glm-4.5v"].contains(&model.id.as_str());
    completion_compat(
        model,
        OpenAICompletionsCompat {
            supports_developer_role: Some(false),
            thinking_format: Some(crate::ThinkingFormat::Zai),
            zai_tool_stream: (!legacy).then_some(true),
            ..Default::default()
        },
    );
}
