// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("claude-3-5-haiku-20241022", claude_3_5_haiku_20241022()),
        ("claude-3-5-haiku-latest", claude_3_5_haiku_latest()),
        ("claude-3-5-sonnet-20240620", claude_3_5_sonnet_20240620()),
        ("claude-3-5-sonnet-20241022", claude_3_5_sonnet_20241022()),
        ("claude-3-7-sonnet-20250219", claude_3_7_sonnet_20250219()),
        ("claude-3-haiku-20240307", claude_3_haiku_20240307()),
        ("claude-3-opus-20240229", claude_3_opus_20240229()),
        ("claude-3-sonnet-20240229", claude_3_sonnet_20240229()),
        ("claude-haiku-4-5", claude_haiku_4_5()),
        ("claude-haiku-4-5-20251001", claude_haiku_4_5_20251001()),
    ]
}
pub(super) fn models_10() -> [(&'static str, Model); 10] {
    [
        ("claude-opus-4-0", claude_opus_4_0()),
        ("claude-opus-4-1", claude_opus_4_1()),
        ("claude-opus-4-1-20250805", claude_opus_4_1_20250805()),
        ("claude-opus-4-20250514", claude_opus_4_20250514()),
        ("claude-opus-4-5", claude_opus_4_5()),
        ("claude-opus-4-5-20251101", claude_opus_4_5_20251101()),
        ("claude-opus-4-6", claude_opus_4_6()),
        ("claude-opus-4-7", claude_opus_4_7()),
        ("claude-sonnet-4-0", claude_sonnet_4_0()),
        ("claude-sonnet-4-20250514", claude_sonnet_4_20250514()),
    ]
}
fn claude_3_5_haiku_20241022() -> Model {
    Model {
        id: "claude-3-5-haiku-20241022".into(),
        name: "Claude Haiku 3.5".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.8,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_5_haiku_latest() -> Model {
    Model {
        id: "claude-3-5-haiku-latest".into(),
        name: "Claude Haiku 3.5 (latest)".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.8,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_5_sonnet_20240620() -> Model {
    Model {
        id: "claude-3-5-sonnet-20240620".into(),
        name: "Claude Sonnet 3.5".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_5_sonnet_20241022() -> Model {
    Model {
        id: "claude-3-5-sonnet-20241022".into(),
        name: "Claude Sonnet 3.5 v2".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_7_sonnet_20250219() -> Model {
    Model {
        id: "claude-3-7-sonnet-20250219".into(),
        name: "Claude Sonnet 3.7".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_haiku_20240307() -> Model {
    Model {
        id: "claude-3-haiku-20240307".into(),
        name: "Claude Haiku 3".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.3,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_opus_20240229() -> Model {
    Model {
        id: "claude-3-opus-20240229".into(),
        name: "Claude Opus 3".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn claude_3_sonnet_20240229() -> Model {
    Model {
        id: "claude-3-sonnet-20240229".into(),
        name: "Claude Sonnet 3".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 0.3,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn claude_haiku_4_5() -> Model {
    Model {
        id: "claude-haiku-4-5".into(),
        name: "Claude Haiku 4.5 (latest)".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_haiku_4_5_20251001() -> Model {
    Model {
        id: "claude-haiku-4-5-20251001".into(),
        name: "Claude Haiku 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_0() -> Model {
    Model {
        id: "claude-opus-4-0".into(),
        name: "Claude Opus 4 (latest)".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_1() -> Model {
    Model {
        id: "claude-opus-4-1".into(),
        name: "Claude Opus 4.1 (latest)".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_1_20250805() -> Model {
    Model {
        id: "claude-opus-4-1-20250805".into(),
        name: "Claude Opus 4.1".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_20250514() -> Model {
    Model {
        id: "claude-opus-4-20250514".into(),
        name: "Claude Opus 4".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_5() -> Model {
    Model {
        id: "claude-opus-4-5".into(),
        name: "Claude Opus 4.5 (latest)".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_5_20251101() -> Model {
    Model {
        id: "claude-opus-4-5-20251101".into(),
        name: "Claude Opus 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_6() -> Model {
    Model {
        id: "claude-opus-4-6".into(),
        name: "Claude Opus 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_7() -> Model {
    Model {
        id: "claude-opus-4-7".into(),
        name: "Claude Opus 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_sonnet_4_0() -> Model {
    Model {
        id: "claude-sonnet-4-0".into(),
        name: "Claude Sonnet 4 (latest)".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_sonnet_4_20250514() -> Model {
    Model {
        id: "claude-sonnet-4-20250514".into(),
        name: "Claude Sonnet 4".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}
