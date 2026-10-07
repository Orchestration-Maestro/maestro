// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 6] {
    [
        ("moonshotai/kimi-k2", moonshotai_kimi_k2()),
        ("moonshotai/kimi-k2-thinking", moonshotai_kimi_k2_thinking()),
        (
            "moonshotai/kimi-k2-thinking-turbo",
            moonshotai_kimi_k2_thinking_turbo(),
        ),
        ("moonshotai/kimi-k2-turbo", moonshotai_kimi_k2_turbo()),
        ("moonshotai/kimi-k2.5", moonshotai_kimi_k2_dot_5()),
        ("moonshotai/kimi-k2.6", moonshotai_kimi_k2_dot_6()),
    ]
}
fn moonshotai_kimi_k2() -> Model {
    Model {
        id: "moonshotai/kimi-k2".into(),
        name: "Kimi K2 Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshotai/kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_114.0,
        max_tokens: 262_114.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_thinking_turbo() -> Model {
    Model {
        id: "moonshotai/kimi-k2-thinking-turbo".into(),
        name: "Kimi K2 Thinking Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.15,
            output: 8.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_114.0,
        max_tokens: 262_114.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_turbo() -> Model {
    Model {
        id: "moonshotai/kimi-k2-turbo".into(),
        name: "Kimi K2 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.15,
            output: 8.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_dot_5() -> Model {
    Model {
        id: "moonshotai/kimi-k2.5".into(),
        name: "Kimi K2.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.0,
        },
        context_window: 262_114.0,
        max_tokens: 262_114.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_dot_6() -> Model {
    Model {
        id: "moonshotai/kimi-k2.6".into(),
        name: "Kimi K2.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}
