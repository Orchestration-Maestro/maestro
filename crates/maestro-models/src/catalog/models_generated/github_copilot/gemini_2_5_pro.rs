// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models() -> [(&'static str, Model); 4] {
    [
        ("gemini-2.5-pro", gemini_2_dot_5_pro()),
        ("gemini-3-flash-preview", gemini_3_flash_preview()),
        ("gemini-3-pro-preview", gemini_3_pro_preview()),
        ("gemini-3.1-pro-preview", gemini_3_dot_1_pro_preview()),
    ]
}
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
