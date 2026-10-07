// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("grok-2", grok_2()),
        ("grok-2-1212", grok_2_1212()),
        ("grok-2-latest", grok_2_latest()),
        ("grok-2-vision", grok_2_vision()),
        ("grok-2-vision-1212", grok_2_vision_1212()),
        ("grok-2-vision-latest", grok_2_vision_latest()),
        ("grok-3", grok_3()),
        ("grok-3-fast", grok_3_fast()),
        ("grok-3-fast-latest", grok_3_fast_latest()),
        ("grok-3-latest", grok_3_latest()),
    ]
}
pub(super) fn models_10() -> [(&'static str, Model); 10] {
    [
        ("grok-3-mini", grok_3_mini()),
        ("grok-3-mini-fast", grok_3_mini_fast()),
        ("grok-3-mini-fast-latest", grok_3_mini_fast_latest()),
        ("grok-3-mini-latest", grok_3_mini_latest()),
        ("grok-4", grok_4()),
        ("grok-4-1-fast", grok_4_1_fast()),
        ("grok-4-1-fast-non-reasoning", grok_4_1_fast_non_reasoning()),
        ("grok-4-fast", grok_4_fast()),
        ("grok-4-fast-non-reasoning", grok_4_fast_non_reasoning()),
        (
            "grok-4.20-0309-non-reasoning",
            grok_4_dot_20_0309_non_reasoning(),
        ),
    ]
}
fn grok_2() -> Model {
    Model {
        id: "grok-2".into(),
        name: "Grok 2".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 2.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_2_1212() -> Model {
    Model {
        id: "grok-2-1212".into(),
        name: "Grok 2 (1212)".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 2.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_2_latest() -> Model {
    Model {
        id: "grok-2-latest".into(),
        name: "Grok 2 Latest".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 2.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_2_vision() -> Model {
    Model {
        id: "grok-2-vision".into(),
        name: "Grok 2 Vision".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 2.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn grok_2_vision_1212() -> Model {
    Model {
        id: "grok-2-vision-1212".into(),
        name: "Grok 2 Vision (1212)".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 2.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn grok_2_vision_latest() -> Model {
    Model {
        id: "grok-2-vision-latest".into(),
        name: "Grok 2 Vision Latest".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 2.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn grok_3() -> Model {
    Model {
        id: "grok-3".into(),
        name: "Grok 3".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_fast() -> Model {
    Model {
        id: "grok-3-fast".into(),
        name: "Grok 3 Fast".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_fast_latest() -> Model {
    Model {
        id: "grok-3-fast-latest".into(),
        name: "Grok 3 Fast Latest".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_latest() -> Model {
    Model {
        id: "grok-3-latest".into(),
        name: "Grok 3 Latest".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_mini() -> Model {
    Model {
        id: "grok-3-mini".into(),
        name: "Grok 3 Mini".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_mini_fast() -> Model {
    Model {
        id: "grok-3-mini-fast".into(),
        name: "Grok 3 Mini Fast".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 4.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_mini_fast_latest() -> Model {
    Model {
        id: "grok-3-mini-fast-latest".into(),
        name: "Grok 3 Mini Fast Latest".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 4.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_3_mini_latest() -> Model {
    Model {
        id: "grok-3-mini-latest".into(),
        name: "Grok 3 Mini Latest".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn grok_4() -> Model {
    Model {
        id: "grok-4".into(),
        name: "Grok 4".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn grok_4_1_fast() -> Model {
    Model {
        id: "grok-4-1-fast".into(),
        name: "Grok 4.1 Fast".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.5,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn grok_4_1_fast_non_reasoning() -> Model {
    Model {
        id: "grok-4-1-fast-non-reasoning".into(),
        name: "Grok 4.1 Fast (Non-Reasoning)".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.5,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn grok_4_fast() -> Model {
    Model {
        id: "grok-4-fast".into(),
        name: "Grok 4 Fast".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.5,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn grok_4_fast_non_reasoning() -> Model {
    Model {
        id: "grok-4-fast-non-reasoning".into(),
        name: "Grok 4 Fast (Non-Reasoning)".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.5,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn grok_4_dot_20_0309_non_reasoning() -> Model {
    Model {
        id: "grok-4.20-0309-non-reasoning".into(),
        name: "Grok 4.20 (Non-Reasoning)".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}
