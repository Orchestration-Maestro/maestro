// Generated model descriptor data.
use crate::{MaxTokensField, Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models() -> [(&'static str, Model); 7] {
    [
        ("kimi-k2-0711-preview", kimi_k2_0711_preview()),
        ("kimi-k2-0905-preview", kimi_k2_0905_preview()),
        ("kimi-k2-thinking", kimi_k2_thinking()),
        ("kimi-k2-thinking-turbo", kimi_k2_thinking_turbo()),
        ("kimi-k2-turbo-preview", kimi_k2_turbo_preview()),
        ("kimi-k2.5", kimi_k2_dot_5()),
        ("kimi-k2.6", kimi_k2_dot_6()),
    ]
}
fn kimi_k2_0711_preview() -> Model {
    Model {
        id: "kimi-k2-0711-preview".into(),
        name: "Kimi K2 0711".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn kimi_k2_0905_preview() -> Model {
    Model {
        id: "kimi-k2-0905-preview".into(),
        name: "Kimi K2 0905".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn kimi_k2_thinking() -> Model {
    Model {
        id: "kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn kimi_k2_thinking_turbo() -> Model {
    Model {
        id: "kimi-k2-thinking-turbo".into(),
        name: "Kimi K2 Thinking Turbo".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.15,
            output: 8.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn kimi_k2_turbo_preview() -> Model {
    Model {
        id: "kimi-k2-turbo-preview".into(),
        name: "Kimi K2 Turbo".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.4,
            output: 10.0,
            cache_read: 0.6,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn kimi_k2_dot_5() -> Model {
    Model {
        id: "kimi-k2.5".into(),
        name: "Kimi K2.5".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn kimi_k2_dot_6() -> Model {
    Model {
        id: "kimi-k2.6".into(),
        name: "Kimi K2.6".into(),
        api: "openai-completions".into(),
        provider: "moonshotai-cn".into(),
        base_url: "https://api.moonshot.cn/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(false),
                supports_reasoning_effort: Some(false),
                max_tokens_field: Some(MaxTokensField::MaxTokens),
                supports_strict_mode: Some(false),
                ..Default::default()
            },
        ))),
    }
}
