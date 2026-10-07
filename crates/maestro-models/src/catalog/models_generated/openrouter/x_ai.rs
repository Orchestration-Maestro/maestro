// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_238() -> [(&'static str, Model); 10] {
    [
        ("x-ai/grok-3", x_ai_grok_3()),
        ("x-ai/grok-3-beta", x_ai_grok_3_beta()),
        ("x-ai/grok-3-mini", x_ai_grok_3_mini()),
        ("x-ai/grok-3-mini-beta", x_ai_grok_3_mini_beta()),
        ("x-ai/grok-4", x_ai_grok_4()),
        ("x-ai/grok-4-fast", x_ai_grok_4_fast()),
        ("x-ai/grok-4.1-fast", x_ai_grok_4_dot_1_fast()),
        ("x-ai/grok-4.20", x_ai_grok_4_dot_20()),
        ("x-ai/grok-4.3", x_ai_grok_4_dot_3()),
        ("x-ai/grok-code-fast-1", x_ai_grok_code_fast_1()),
    ]
}
fn x_ai_grok_3() -> Model {
    Model {
        id: "x-ai/grok-3".into(),
        name: "xAI: Grok 3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_3_beta() -> Model {
    Model {
        id: "x-ai/grok-3-beta".into(),
        name: "xAI: Grok 3 Beta".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_3_mini() -> Model {
    Model {
        id: "x-ai/grok-3-mini".into(),
        name: "xAI: Grok 3 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_3_mini_beta() -> Model {
    Model {
        id: "x-ai/grok-3-mini-beta".into(),
        name: "xAI: Grok 3 Mini Beta".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_4() -> Model {
    Model {
        id: "x-ai/grok-4".into(),
        name: "xAI: Grok 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_4_fast() -> Model {
    Model {
        id: "x-ai/grok-4-fast".into(),
        name: "xAI: Grok 4 Fast".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_4_dot_1_fast() -> Model {
    Model {
        id: "x-ai/grok-4.1-fast".into(),
        name: "xAI: Grok 4.1 Fast".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_4_dot_20() -> Model {
    Model {
        id: "x-ai/grok-4.20".into(),
        name: "xAI: Grok 4.20".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_4_dot_3() -> Model {
    Model {
        id: "x-ai/grok-4.3".into(),
        name: "xAI: Grok 4.3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn x_ai_grok_code_fast_1() -> Model {
    Model {
        id: "x-ai/grok-code-fast-1".into(),
        name: "xAI: Grok Code Fast 1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.5,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 10_000.0,
        headers: None,
        compat: None,
    }
}
