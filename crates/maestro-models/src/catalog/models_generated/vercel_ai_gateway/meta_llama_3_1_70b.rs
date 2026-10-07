// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 7] {
    [
        ("meta/llama-3.1-70b", meta_llama_3_dot_1_70b()),
        ("meta/llama-3.1-8b", meta_llama_3_dot_1_8b()),
        ("meta/llama-3.2-11b", meta_llama_3_dot_2_11b()),
        ("meta/llama-3.2-90b", meta_llama_3_dot_2_90b()),
        ("meta/llama-3.3-70b", meta_llama_3_dot_3_70b()),
        ("meta/llama-4-maverick", meta_llama_4_maverick()),
        ("meta/llama-4-scout", meta_llama_4_scout()),
    ]
}
fn meta_llama_3_dot_1_70b() -> Model {
    Model {
        id: "meta/llama-3.1-70b".into(),
        name: "Llama 3.1 70B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_3_dot_1_8b() -> Model {
    Model {
        id: "meta/llama-3.1-8b".into(),
        name: "Llama 3.1 8B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.22,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_3_dot_2_11b() -> Model {
    Model {
        id: "meta/llama-3.2-11b".into(),
        name: "Llama 3.2 11B Vision Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.16,
            output: 0.16,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_3_dot_2_90b() -> Model {
    Model {
        id: "meta/llama-3.2-90b".into(),
        name: "Llama 3.2 90B Vision Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_3_dot_3_70b() -> Model {
    Model {
        id: "meta/llama-3.3-70b".into(),
        name: "Llama 3.3 70B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_4_maverick() -> Model {
    Model {
        id: "meta/llama-4-maverick".into(),
        name: "Llama 4 Maverick 17B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.24,
            output: 0.970_000_000_000_000_1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_4_scout() -> Model {
    Model {
        id: "meta/llama-4-scout".into(),
        name: "Llama 4 Scout 17B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.169_999_999_999_999_98,
            output: 0.66,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}
