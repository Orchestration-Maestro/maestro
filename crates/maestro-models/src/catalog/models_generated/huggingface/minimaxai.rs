// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_0() -> [(&'static str, Model); 3] {
    [
        ("MiniMaxAI/MiniMax-M2.1", minimaxai_minimax_m2_dot_1()),
        ("MiniMaxAI/MiniMax-M2.5", minimaxai_minimax_m2_dot_5()),
        ("MiniMaxAI/MiniMax-M2.7", minimaxai_minimax_m2_dot_7()),
    ]
}
fn minimaxai_minimax_m2_dot_1() -> Model {
    Model {
        id: "MiniMaxAI/MiniMax-M2.1".into(),
        name: "MiniMax-M2.1".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn minimaxai_minimax_m2_dot_5() -> Model {
    Model {
        id: "MiniMaxAI/MiniMax-M2.5".into(),
        name: "MiniMax-M2.5".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn minimaxai_minimax_m2_dot_7() -> Model {
    Model {
        id: "MiniMaxAI/MiniMax-M2.7".into(),
        name: "MiniMax-M2.7".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}
