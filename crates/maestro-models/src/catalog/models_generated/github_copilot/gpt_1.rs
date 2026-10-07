// Generated model descriptor data.
use crate::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat,
};
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
pub(super) fn models_21() -> [(&'static str, Model); 3] {
    [
        ("gpt-5.3-codex", gpt_5_dot_3_codex()),
        ("gpt-5.4", gpt_5_dot_4()),
        ("gpt-5.4-mini", gpt_5_dot_4_mini()),
    ]
}
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
