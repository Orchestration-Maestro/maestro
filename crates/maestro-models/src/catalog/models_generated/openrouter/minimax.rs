// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_75() -> [(&'static str, Model); 6] {
    [
        ("minimax/minimax-m1", minimax_minimax_m1()),
        ("minimax/minimax-m2", minimax_minimax_m2()),
        ("minimax/minimax-m2.1", minimax_minimax_m2_dot_1()),
        ("minimax/minimax-m2.5", minimax_minimax_m2_dot_5()),
        ("minimax/minimax-m2.5:free", minimax_minimax_m2_dot_5_free()),
        ("minimax/minimax-m2.7", minimax_minimax_m2_dot_7()),
    ]
}
fn minimax_minimax_m1() -> Model {
    Model {
        id: "minimax/minimax-m1".into(),
        name: "MiniMax: MiniMax M1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2() -> Model {
    Model {
        id: "minimax/minimax-m2".into(),
        name: "MiniMax: MiniMax M2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.255,
            output: 1.0,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 196_608.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_1() -> Model {
    Model {
        id: "minimax/minimax-m2.1".into(),
        name: "MiniMax: MiniMax M2.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.29,
            output: 0.95,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 196_608.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax/minimax-m2.5".into(),
        name: "MiniMax: MiniMax M2.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 1.15,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_5_free() -> Model {
    Model {
        id: "minimax/minimax-m2.5:free".into(),
        name: "MiniMax: MiniMax M2.5 (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax/minimax-m2.7".into(),
        name: "MiniMax: MiniMax M2.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.059,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
