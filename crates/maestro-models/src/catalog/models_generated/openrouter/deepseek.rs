// Generated model descriptor data.
use crate::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat,
    ThinkingFormat,
};
pub(super) fn models_35() -> [(&'static str, Model); 10] {
    [
        ("deepseek/deepseek-chat", deepseek_deepseek_chat()),
        (
            "deepseek/deepseek-chat-v3-0324",
            deepseek_deepseek_chat_v3_0324(),
        ),
        (
            "deepseek/deepseek-chat-v3.1",
            deepseek_deepseek_chat_v3_dot_1(),
        ),
        ("deepseek/deepseek-r1", deepseek_deepseek_r1()),
        ("deepseek/deepseek-r1-0528", deepseek_deepseek_r1_0528()),
        (
            "deepseek/deepseek-v3.1-terminus",
            deepseek_deepseek_v3_dot_1_terminus(),
        ),
        ("deepseek/deepseek-v3.2", deepseek_deepseek_v3_dot_2()),
        (
            "deepseek/deepseek-v3.2-exp",
            deepseek_deepseek_v3_dot_2_exp(),
        ),
        ("deepseek/deepseek-v4-flash", deepseek_deepseek_v4_flash()),
        ("deepseek/deepseek-v4-pro", deepseek_deepseek_v4_pro()),
    ]
}
fn deepseek_deepseek_chat() -> Model {
    Model {
        id: "deepseek/deepseek-chat".into(),
        name: "DeepSeek: DeepSeek V3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.32,
            output: 0.889_999_999_999_999_9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_chat_v3_0324() -> Model {
    Model {
        id: "deepseek/deepseek-chat-v3-0324".into(),
        name: "DeepSeek: DeepSeek V3 0324".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.77,
            cache_read: 0.135,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_chat_v3_dot_1() -> Model {
    Model {
        id: "deepseek/deepseek-chat-v3.1".into(),
        name: "DeepSeek: DeepSeek V3.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 7168.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_r1() -> Model {
    Model {
        id: "deepseek/deepseek-r1".into(),
        name: "DeepSeek: R1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.7,
            output: 2.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 64_000.0,
        max_tokens: 16_000.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_r1_0528() -> Model {
    Model {
        id: "deepseek/deepseek-r1-0528".into(),
        name: "DeepSeek: R1 0528".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 2.150_000_000_000_000_4,
            cache_read: 0.35,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_1_terminus() -> Model {
    Model {
        id: "deepseek/deepseek-v3.1-terminus".into(),
        name: "DeepSeek: DeepSeek V3.1 Terminus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.27,
            output: 0.95,
            cache_read: 0.13,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_2() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2".into(),
        name: "DeepSeek: DeepSeek V3.2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.252,
            output: 0.378,
            cache_read: 0.0252,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_2_exp() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2-exp".into(),
        name: "DeepSeek: DeepSeek V3.2 Exp".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.27,
            output: 0.41,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v4_flash() -> Model {
    Model {
        id: "deepseek/deepseek-v4-flash".into(),
        name: "DeepSeek: DeepSeek V4 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, None),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("high".into())),
                (ModelThinkingLevel::Xhigh, Some("max".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                requires_reasoning_content_on_assistant_messages: Some(true),
                thinking_format: Some(ThinkingFormat::Deepseek),
                ..Default::default()
            },
        ))),
    }
}

fn deepseek_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek/deepseek-v4-pro".into(),
        name: "DeepSeek: DeepSeek V4 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, None),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("high".into())),
                (ModelThinkingLevel::Xhigh, Some("max".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                requires_reasoning_content_on_assistant_messages: Some(true),
                thinking_format: Some(ThinkingFormat::Deepseek),
                ..Default::default()
            },
        ))),
    }
}
