// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_102() -> [(&'static str, Model); 5] {
    [
        ("moonshotai/kimi-k2", moonshotai_kimi_k2()),
        ("moonshotai/kimi-k2-0905", moonshotai_kimi_k2_0905()),
        ("moonshotai/kimi-k2-thinking", moonshotai_kimi_k2_thinking()),
        ("moonshotai/kimi-k2.5", moonshotai_kimi_k2_dot_5()),
        ("moonshotai/kimi-k2.6", moonshotai_kimi_k2_dot_6()),
    ]
}
pub(super) fn models_271() -> [(&'static str, Model); 1] {
    [("~moonshotai/kimi-latest", moonshotai_kimi_latest())]
}
fn moonshotai_kimi_k2() -> Model {
    Model {
        id: "moonshotai/kimi-k2".into(),
        name: "MoonshotAI: Kimi K2 0711".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.570_000_000_000_000_1,
            output: 2.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_0905() -> Model {
    Model {
        id: "moonshotai/kimi-k2-0905".into(),
        name: "MoonshotAI: Kimi K2 0905".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshotai/kimi-k2-thinking".into(),
        name: "MoonshotAI: Kimi K2 Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        compat: None,
    }
}

fn moonshotai_kimi_k2_dot_5() -> Model {
    Model {
        id: "moonshotai/kimi-k2.5".into(),
        name: "MoonshotAI: Kimi K2.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.41,
            output: 2.06,
            cache_read: 0.07,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_dot_6() -> Model {
    Model {
        id: "moonshotai/kimi-k2.6".into(),
        name: "MoonshotAI: Kimi K2.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.74,
            output: 3.49,
            cache_read: 0.14,
            cache_write: 0.0,
        },
        context_window: 262_142.0,
        max_tokens: 262_142.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_latest() -> Model {
    Model {
        id: "~moonshotai/kimi-latest".into(),
        name: "MoonshotAI Kimi Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.74,
            output: 3.49,
            cache_read: 0.14,
            cache_write: 0.0,
        },
        context_window: 262_142.0,
        max_tokens: 262_142.0,
        headers: None,
        compat: None,
    }
}
