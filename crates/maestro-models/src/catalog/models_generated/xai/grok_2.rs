// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
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
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
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
        context_window: 8_192.0,
        max_tokens: 4_096.0,
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
        context_window: 8_192.0,
        max_tokens: 4_096.0,
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
        context_window: 8_192.0,
        max_tokens: 4_096.0,
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
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}
