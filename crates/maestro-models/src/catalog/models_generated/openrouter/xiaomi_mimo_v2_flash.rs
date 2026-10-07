// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 5] {
    [
        ("xiaomi/mimo-v2-flash", xiaomi_mimo_v2_flash()),
        ("xiaomi/mimo-v2-omni", xiaomi_mimo_v2_omni()),
        ("xiaomi/mimo-v2-pro", xiaomi_mimo_v2_pro()),
        ("xiaomi/mimo-v2.5", xiaomi_mimo_v2_dot_5()),
        ("xiaomi/mimo-v2.5-pro", xiaomi_mimo_v2_dot_5_pro()),
    ]
}
fn xiaomi_mimo_v2_flash() -> Model {
    Model {
        id: "xiaomi/mimo-v2-flash".into(),
        name: "Xiaomi: MiMo-V2-Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.29,
            cache_read: 0.045,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_omni() -> Model {
    Model {
        id: "xiaomi/mimo-v2-omni".into(),
        name: "Xiaomi: MiMo-V2-Omni".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2-pro".into(),
        name: "Xiaomi: MiMo-V2-Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_dot_5() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5".into(),
        name: "Xiaomi: MiMo-V2.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_dot_5_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5-pro".into(),
        name: "Xiaomi: MiMo-V2.5-Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
