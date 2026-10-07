// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 7] {
    [
        ("minimax/minimax-m2", minimax_minimax_m2()),
        ("minimax/minimax-m2.1", minimax_minimax_m2_dot_1()),
        (
            "minimax/minimax-m2.1-lightning",
            minimax_minimax_m2_dot_1_lightning(),
        ),
        ("minimax/minimax-m2.5", minimax_minimax_m2_dot_5()),
        (
            "minimax/minimax-m2.5-highspeed",
            minimax_minimax_m2_dot_5_highspeed(),
        ),
        ("minimax/minimax-m2.7", minimax_minimax_m2_dot_7()),
        (
            "minimax/minimax-m2.7-highspeed",
            minimax_minimax_m2_dot_7_highspeed(),
        ),
    ]
}
fn minimax_minimax_m2() -> Model {
    Model {
        id: "minimax/minimax-m2".into(),
        name: "MiniMax M2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 205_000.0,
        max_tokens: 205_000.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_1() -> Model {
    Model {
        id: "minimax/minimax-m2.1".into(),
        name: "MiniMax M2.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_1_lightning() -> Model {
    Model {
        id: "minimax/minimax-m2.1-lightning".into(),
        name: "MiniMax M2.1 Lightning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 2.4,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax/minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_5_highspeed() -> Model {
    Model {
        id: "minimax/minimax-m2.5-highspeed".into(),
        name: "MiniMax M2.5 High Speed".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax/minimax-m2.7".into(),
        name: "Minimax M2.7".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

fn minimax_minimax_m2_dot_7_highspeed() -> Model {
    Model {
        id: "minimax/minimax-m2.7-highspeed".into(),
        name: "MiniMax M2.7 High Speed".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}
