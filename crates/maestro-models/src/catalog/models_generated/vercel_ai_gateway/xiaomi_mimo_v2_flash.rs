// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 4] {
    [
        ("xiaomi/mimo-v2-flash", xiaomi_mimo_v2_flash()),
        ("xiaomi/mimo-v2-pro", xiaomi_mimo_v2_pro()),
        ("xiaomi/mimo-v2.5", xiaomi_mimo_v2_dot_5()),
        ("xiaomi/mimo-v2.5-pro", xiaomi_mimo_v2_dot_5_pro()),
    ]
}
fn xiaomi_mimo_v2_flash() -> Model {
    Model {
        id: "xiaomi/mimo-v2-flash".into(),
        name: "MiMo V2 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2-pro".into(),
        name: "MiMo V2 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_dot_5() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5".into(),
        name: "MiMo M2.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}

fn xiaomi_mimo_v2_dot_5_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5-pro".into(),
        name: "MiMo V2.5 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}
