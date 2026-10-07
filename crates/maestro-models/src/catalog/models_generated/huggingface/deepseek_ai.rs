// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_10() -> [(&'static str, Model); 3] {
    [
        (
            "deepseek-ai/DeepSeek-R1-0528",
            deepseek_ai_deepseek_r1_0528(),
        ),
        ("deepseek-ai/DeepSeek-V3.2", deepseek_ai_deepseek_v3_dot_2()),
        ("deepseek-ai/DeepSeek-V4-Pro", deepseek_ai_deepseek_v4_pro()),
    ]
}
fn deepseek_ai_deepseek_r1_0528() -> Model {
    Model {
        id: "deepseek-ai/DeepSeek-R1-0528".into(),
        name: "DeepSeek-R1-0528".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 5.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn deepseek_ai_deepseek_v3_dot_2() -> Model {
    Model {
        id: "deepseek-ai/DeepSeek-V3.2".into(),
        name: "DeepSeek-V3.2".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.28,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn deepseek_ai_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek-ai/DeepSeek-V4-Pro".into(),
        name: "DeepSeek V4 Pro".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.74,
            output: 3.48,
            cache_read: 0.145,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 393_216.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}
