//! Generated model descriptors for github-copilot.

use crate::{
    AnthropicMessagesCompat, Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel,
    OpenAICompletionsCompat,
};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_7());
    models.extend(models_11());
    models.extend(models_21());
    models.extend(models_24());
    models.extend(models_25());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 7] {
    [
        ("claude-haiku-4.5", claude_haiku_4_dot_5()),
        ("claude-opus-4.5", claude_opus_4_dot_5()),
        ("claude-opus-4.6", claude_opus_4_dot_6()),
        ("claude-opus-4.7", claude_opus_4_dot_7()),
        ("claude-sonnet-4", claude_sonnet_4()),
        ("claude-sonnet-4.5", claude_sonnet_4_dot_5()),
        ("claude-sonnet-4.6", claude_sonnet_4_dot_6()),
    ]
}
/// Construct the recorded descriptor for this model.
fn claude_haiku_4_dot_5() -> Model {
    Model {
        id: "claude-haiku-4.5".into(),
        name: "Claude Haiku 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 144_000.0,
        max_tokens: 32_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            supports_eager_tool_input_streaming: Some(false),
            ..Default::default()
        })),
    }
}

/// Construct the recorded descriptor for this model.
fn claude_opus_4_dot_5() -> Model {
    Model {
        id: "claude-opus-4.5".into(),
        name: "Claude Opus 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 160_000.0,
        max_tokens: 32_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn claude_opus_4_dot_6() -> Model {
    Model {
        id: "claude-opus-4.6".into(),
        name: "Claude Opus 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn claude_opus_4_dot_7() -> Model {
    Model {
        id: "claude-opus-4.7".into(),
        name: "Claude Opus 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 144_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn claude_sonnet_4() -> Model {
    Model {
        id: "claude-sonnet-4".into(),
        name: "Claude Sonnet 4".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 216_000.0,
        max_tokens: 16_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            supports_eager_tool_input_streaming: Some(false),
            ..Default::default()
        })),
    }
}

/// Construct the recorded descriptor for this model.
fn claude_sonnet_4_dot_5() -> Model {
    Model {
        id: "claude-sonnet-4.5".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 144_000.0,
        max_tokens: 32_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            supports_eager_tool_input_streaming: Some(false),
            ..Default::default()
        })),
    }
}

/// Construct the recorded descriptor for this model.
fn claude_sonnet_4_dot_6() -> Model {
    Model {
        id: "claude-sonnet-4.6".into(),
        name: "Claude Sonnet 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_7() -> [(&'static str, Model); 4] {
    [
        ("gemini-2.5-pro", gemini_2_dot_5_pro()),
        ("gemini-3-flash-preview", gemini_3_flash_preview()),
        ("gemini-3-pro-preview", gemini_3_pro_preview()),
        ("gemini-3.1-pro-preview", gemini_3_dot_1_pro_preview()),
    ]
}
/// Construct the recorded descriptor for this model.
fn gemini_2_dot_5_pro() -> Model {
    Model {
        id: "gemini-2.5-pro".into(),
        name: "Gemini 2.5 Pro".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn gemini_3_flash_preview() -> Model {
    Model {
        id: "gemini-3-flash-preview".into(),
        name: "Gemini 3 Flash".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn gemini_3_pro_preview() -> Model {
    Model {
        id: "gemini-3-pro-preview".into(),
        name: "Gemini 3 Pro Preview".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "gemini-3.1-pro-preview".into(),
        name: "Gemini 3.1 Pro Preview".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_11() -> [(&'static str, Model); 10] {
    [
        ("gpt-4.1", gpt_4_dot_1()),
        ("gpt-4o", gpt_4o()),
        ("gpt-5", gpt_5()),
        ("gpt-5-mini", gpt_5_mini()),
        ("gpt-5.1", gpt_5_dot_1()),
        ("gpt-5.1-codex", gpt_5_dot_1_codex()),
        ("gpt-5.1-codex-max", gpt_5_dot_1_codex_max()),
        ("gpt-5.1-codex-mini", gpt_5_dot_1_codex_mini()),
        ("gpt-5.2", gpt_5_dot_2()),
        ("gpt-5.2-codex", gpt_5_dot_2_codex()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_21() -> [(&'static str, Model); 3] {
    [
        ("gpt-5.3-codex", gpt_5_dot_3_codex()),
        ("gpt-5.4", gpt_5_dot_4()),
        ("gpt-5.4-mini", gpt_5_dot_4_mini()),
    ]
}
/// Construct the recorded descriptor for this model.
fn gpt_4_dot_1() -> Model {
    Model {
        id: "gpt-4.1".into(),
        name: "GPT-4.1".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_4o() -> Model {
    Model {
        id: "gpt-4o".into(),
        name: "GPT-4o".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5() -> Model {
    Model {
        id: "gpt-5".into(),
        name: "GPT-5".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_mini() -> Model {
    Model {
        id: "gpt-5-mini".into(),
        name: "GPT-5-mini".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 264_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1() -> Model {
    Model {
        id: "gpt-5.1".into(),
        name: "GPT-5.1".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 264_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1_codex() -> Model {
    Model {
        id: "gpt-5.1-codex".into(),
        name: "GPT-5.1-Codex".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1_codex_max() -> Model {
    Model {
        id: "gpt-5.1-codex-max".into(),
        name: "GPT-5.1-Codex-max".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1_codex_mini() -> Model {
    Model {
        id: "gpt-5.1-codex-mini".into(),
        name: "GPT-5.1-Codex-mini".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_2() -> Model {
    Model {
        id: "gpt-5.2".into(),
        name: "GPT-5.2".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 264_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_2_codex() -> Model {
    Model {
        id: "gpt-5.2-codex".into(),
        name: "GPT-5.2-Codex".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_3_codex() -> Model {
    Model {
        id: "gpt-5.3-codex".into(),
        name: "GPT-5.3-Codex".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_4() -> Model {
    Model {
        id: "gpt-5.4".into(),
        name: "GPT-5.4".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_4_mini() -> Model {
    Model {
        id: "gpt-5.4-mini".into(),
        name: "GPT-5.4 Mini".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_24() -> [(&'static str, Model); 1] {
    [("gpt-5.5", gpt_5_dot_5())]
}
/// Construct the recorded descriptor for this model.
fn gpt_5_dot_5() -> Model {
    Model {
        id: "gpt-5.5".into(),
        name: "GPT-5.5".into(),
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_25() -> [(&'static str, Model); 1] {
    [("grok-code-fast-1", grok_code_fast_1())]
}
/// Construct the recorded descriptor for this model.
fn grok_code_fast_1() -> Model {
    Model {
        id: "grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        api: "openai-completions".into(),
        provider: "github-copilot".into(),
        base_url: "https://api.individual.githubcopilot.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: Some(
            [
                ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
                ("Editor-Version".into(), "vscode/1.107.0".into()),
                ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
                ("Copilot-Integration-Id".into(), "vscode-chat".into()),
            ]
            .into(),
        ),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                ..Default::default()
            },
        ))),
    }
}
