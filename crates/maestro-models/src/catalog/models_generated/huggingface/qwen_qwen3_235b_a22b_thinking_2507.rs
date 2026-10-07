// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models() -> [(&'static str, Model); 6] {
    [
        (
            "Qwen/Qwen3-235B-A22B-Thinking-2507",
            qwen_qwen3_235b_a22b_thinking_2507(),
        ),
        (
            "Qwen/Qwen3-Coder-480B-A35B-Instruct",
            qwen_qwen3_coder_480b_a35b_instruct(),
        ),
        ("Qwen/Qwen3-Coder-Next", qwen_qwen3_coder_next()),
        (
            "Qwen/Qwen3-Next-80B-A3B-Instruct",
            qwen_qwen3_next_80b_a3b_instruct(),
        ),
        (
            "Qwen/Qwen3-Next-80B-A3B-Thinking",
            qwen_qwen3_next_80b_a3b_thinking(),
        ),
        ("Qwen/Qwen3.5-397B-A17B", qwen_qwen3_dot_5_397b_a17b()),
    ]
}
fn qwen_qwen3_235b_a22b_thinking_2507() -> Model {
    Model {
        id: "Qwen/Qwen3-235B-A22B-Thinking-2507".into(),
        name: "Qwen3-235B-A22B-Thinking-2507".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
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

fn qwen_qwen3_coder_480b_a35b_instruct() -> Model {
    Model {
        id: "Qwen/Qwen3-Coder-480B-A35B-Instruct".into(),
        name: "Qwen3-Coder-480B-A35B-Instruct".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 66_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn qwen_qwen3_coder_next() -> Model {
    Model {
        id: "Qwen/Qwen3-Coder-Next".into(),
        name: "Qwen3-Coder-Next".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
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

fn qwen_qwen3_next_80b_a3b_instruct() -> Model {
    Model {
        id: "Qwen/Qwen3-Next-80B-A3B-Instruct".into(),
        name: "Qwen3-Next-80B-A3B-Instruct".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 1.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 66_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

fn qwen_qwen3_next_80b_a3b_thinking() -> Model {
    Model {
        id: "Qwen/Qwen3-Next-80B-A3B-Thinking".into(),
        name: "Qwen3-Next-80B-A3B-Thinking".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
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

fn qwen_qwen3_dot_5_397b_a17b() -> Model {
    Model {
        id: "Qwen/Qwen3.5-397B-A17B".into(),
        name: "Qwen3.5-397B-A17B".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}
