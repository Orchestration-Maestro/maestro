// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_3() -> [(&'static str, Model); 5] {
    [
        ("amazon/nova-2-lite-v1", amazon_nova_2_lite_v1()),
        ("amazon/nova-lite-v1", amazon_nova_lite_v1()),
        ("amazon/nova-micro-v1", amazon_nova_micro_v1()),
        ("amazon/nova-premier-v1", amazon_nova_premier_v1()),
        ("amazon/nova-pro-v1", amazon_nova_pro_v1()),
    ]
}
fn amazon_nova_2_lite_v1() -> Model {
    Model {
        id: "amazon/nova-2-lite-v1".into(),
        name: "Amazon: Nova 2 Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

fn amazon_nova_lite_v1() -> Model {
    Model {
        id: "amazon/nova-lite-v1".into(),
        name: "Amazon: Nova Lite 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 5120.0,
        headers: None,
        compat: None,
    }
}

fn amazon_nova_micro_v1() -> Model {
    Model {
        id: "amazon/nova-micro-v1".into(),
        name: "Amazon: Nova Micro 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.035,
            output: 0.14,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 5120.0,
        headers: None,
        compat: None,
    }
}

fn amazon_nova_premier_v1() -> Model {
    Model {
        id: "amazon/nova-premier-v1".into(),
        name: "Amazon: Nova Premier 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 12.5,
            cache_read: 0.625,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn amazon_nova_pro_v1() -> Model {
    Model {
        id: "amazon/nova-pro-v1".into(),
        name: "Amazon: Nova Pro 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.799_999_999_999_999_9,
            output: 3.199_999_999_999_999_7,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 5120.0,
        headers: None,
        compat: None,
    }
}
