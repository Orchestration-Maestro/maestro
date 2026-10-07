// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-3.5-turbo", openai_gpt_3_dot_5_turbo()),
        ("openai/gpt-3.5-turbo-0613", openai_gpt_3_dot_5_turbo_0613()),
        ("openai/gpt-3.5-turbo-16k", openai_gpt_3_dot_5_turbo_16k()),
        ("openai/gpt-4", openai_gpt_4()),
        ("openai/gpt-4-0314", openai_gpt_4_0314()),
        ("openai/gpt-4-1106-preview", openai_gpt_4_1106_preview()),
        ("openai/gpt-4-turbo", openai_gpt_4_turbo()),
        ("openai/gpt-4-turbo-preview", openai_gpt_4_turbo_preview()),
        ("openai/gpt-4.1", openai_gpt_4_dot_1()),
        ("openai/gpt-4.1-mini", openai_gpt_4_dot_1_mini()),
    ]
}
fn openai_gpt_3_dot_5_turbo() -> Model {
    Model {
        id: "openai/gpt-3.5-turbo".into(),
        name: "OpenAI: GPT-3.5 Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_385.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_3_dot_5_turbo_0613() -> Model {
    Model {
        id: "openai/gpt-3.5-turbo-0613".into(),
        name: "OpenAI: GPT-3.5 Turbo (older v0613)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 4_095.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_3_dot_5_turbo_16k() -> Model {
    Model {
        id: "openai/gpt-3.5-turbo-16k".into(),
        name: "OpenAI: GPT-3.5 Turbo 16k".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 4.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_385.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4() -> Model {
    Model {
        id: "openai/gpt-4".into(),
        name: "OpenAI: GPT-4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_191.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_0314() -> Model {
    Model {
        id: "openai/gpt-4-0314".into(),
        name: "OpenAI: GPT-4 (older v0314)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_191.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_1106_preview() -> Model {
    Model {
        id: "openai/gpt-4-1106-preview".into(),
        name: "OpenAI: GPT-4 Turbo (older v1106)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_turbo() -> Model {
    Model {
        id: "openai/gpt-4-turbo".into(),
        name: "OpenAI: GPT-4 Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_turbo_preview() -> Model {
    Model {
        id: "openai/gpt-4-turbo-preview".into(),
        name: "OpenAI: GPT-4 Turbo Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_dot_1() -> Model {
    Model {
        id: "openai/gpt-4.1".into(),
        name: "OpenAI: GPT-4.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_dot_1_mini() -> Model {
    Model {
        id: "openai/gpt-4.1-mini".into(),
        name: "OpenAI: GPT-4.1 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 1.599_999_999_999_999_9,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
