// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_20() -> [(&'static str, Model); 10] {
    [
        ("gpt-5.1-codex-mini", gpt_5_dot_1_codex_mini()),
        ("gpt-5.2", gpt_5_dot_2()),
        ("gpt-5.2-chat-latest", gpt_5_dot_2_chat_latest()),
        ("gpt-5.2-codex", gpt_5_dot_2_codex()),
        ("gpt-5.2-pro", gpt_5_dot_2_pro()),
        ("gpt-5.3-chat-latest", gpt_5_dot_3_chat_latest()),
        ("gpt-5.3-codex", gpt_5_dot_3_codex()),
        ("gpt-5.3-codex-spark", gpt_5_dot_3_codex_spark()),
        ("gpt-5.4", gpt_5_dot_4()),
        ("gpt-5.4-mini", gpt_5_dot_4_mini()),
    ]
}
pub(super) fn models_30() -> [(&'static str, Model); 4] {
    [
        ("gpt-5.4-nano", gpt_5_dot_4_nano()),
        ("gpt-5.4-pro", gpt_5_dot_4_pro()),
        ("gpt-5.5", gpt_5_dot_5()),
        ("gpt-5.5-pro", gpt_5_dot_5_pro()),
    ]
}
fn gpt_5_dot_1_codex_mini() -> Model {
    Model {
        id: "gpt-5.1-codex-mini".into(),
        name: "GPT-5.1 Codex mini".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_2() -> Model {
    Model {
        id: "gpt-5.2".into(),
        name: "GPT-5.2".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_2_chat_latest() -> Model {
    Model {
        id: "gpt-5.2-chat-latest".into(),
        name: "GPT-5.2 Chat".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_2_codex() -> Model {
    Model {
        id: "gpt-5.2-codex".into(),
        name: "GPT-5.2 Codex".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_2_pro() -> Model {
    Model {
        id: "gpt-5.2-pro".into(),
        name: "GPT-5.2 Pro".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 21.0,
            output: 168.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_3_chat_latest() -> Model {
    Model {
        id: "gpt-5.3-chat-latest".into(),
        name: "GPT-5.3 Chat (latest)".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_3_codex() -> Model {
    Model {
        id: "gpt-5.3-codex".into(),
        name: "GPT-5.3 Codex".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_3_codex_spark() -> Model {
    Model {
        id: "gpt-5.3-codex-spark".into(),
        name: "GPT-5.3 Codex Spark".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_4() -> Model {
    Model {
        id: "gpt-5.4".into(),
        name: "GPT-5.4".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_4_mini() -> Model {
    Model {
        id: "gpt-5.4-mini".into(),
        name: "GPT-5.4 mini".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.75,
            output: 4.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_4_nano() -> Model {
    Model {
        id: "gpt-5.4-nano".into(),
        name: "GPT-5.4 nano".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 1.25,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_4_pro() -> Model {
    Model {
        id: "gpt-5.4-pro".into(),
        name: "GPT-5.4 Pro".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_5() -> Model {
    Model {
        id: "gpt-5.5".into(),
        name: "GPT-5.5".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_5_pro() -> Model {
    Model {
        id: "gpt-5.5-pro".into(),
        name: "GPT-5.5 Pro".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
