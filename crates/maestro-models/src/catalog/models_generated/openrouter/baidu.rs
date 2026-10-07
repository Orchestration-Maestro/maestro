// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_27() -> [(&'static str, Model); 2] {
    [
        ("baidu/ernie-4.5-21b-a3b", baidu_ernie_4_dot_5_21b_a3b()),
        (
            "baidu/ernie-4.5-vl-28b-a3b",
            baidu_ernie_4_dot_5_vl_28b_a3b(),
        ),
    ]
}
fn baidu_ernie_4_dot_5_21b_a3b() -> Model {
    Model {
        id: "baidu/ernie-4.5-21b-a3b".into(),
        name: "Baidu: ERNIE 4.5 21B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.28,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 120_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

fn baidu_ernie_4_dot_5_vl_28b_a3b() -> Model {
    Model {
        id: "baidu/ernie-4.5-vl-28b-a3b".into(),
        name: "Baidu: ERNIE 4.5 VL 28B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.14,
            output: 0.56,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 30_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}
