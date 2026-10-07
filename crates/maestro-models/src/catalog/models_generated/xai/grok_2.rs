// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_20() -> [(&'static str, Model); 5] {
    [
        ("grok-4.20-0309-reasoning", grok_4_dot_20_0309_reasoning()),
        ("grok-4.3", grok_4_dot_3()),
        ("grok-beta", grok_beta()),
        ("grok-code-fast-1", grok_code_fast_1()),
        ("grok-vision-beta", grok_vision_beta()),
    ]
}
fn grok_4_dot_20_0309_reasoning() -> Model {
    Model {
        id: "grok-4.20-0309-reasoning".into(),
        name: "Grok 4.20 (Reasoning)".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
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

fn grok_4_dot_3() -> Model {
    Model {
        id: "grok-4.3".into(),
        name: "Grok 4.3".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn grok_beta() -> Model {
    Model {
        id: "grok-beta".into(),
        name: "Grok Beta".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 15.0,
            cache_read: 5.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn grok_code_fast_1() -> Model {
    Model {
        id: "grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
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

fn grok_vision_beta() -> Model {
    Model {
        id: "grok-vision-beta".into(),
        name: "Grok Vision Beta".into(),
        api: "openai-completions".into(),
        provider: "xai".into(),
        base_url: "https://api.x.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 15.0,
            cache_read: 5.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
