// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("mistral/codestral", mistral_codestral()),
        ("mistral/devstral-2", mistral_devstral_2()),
        ("mistral/devstral-small", mistral_devstral_small()),
        ("mistral/devstral-small-2", mistral_devstral_small_2()),
        ("mistral/ministral-3b", mistral_ministral_3b()),
        ("mistral/ministral-8b", mistral_ministral_8b()),
        ("mistral/mistral-medium", mistral_mistral_medium()),
        ("mistral/mistral-small", mistral_mistral_small()),
        ("mistral/pixtral-12b", mistral_pixtral_12b()),
        ("mistral/pixtral-large", mistral_pixtral_large()),
    ]
}
fn mistral_codestral() -> Model {
    Model {
        id: "mistral/codestral".into(),
        name: "Mistral Codestral".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_devstral_2() -> Model {
    Model {
        id: "mistral/devstral-2".into(),
        name: "Devstral 2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_devstral_small() -> Model {
    Model {
        id: "mistral/devstral-small".into(),
        name: "Devstral Small 1.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_devstral_small_2() -> Model {
    Model {
        id: "mistral/devstral-small-2".into(),
        name: "Devstral Small 2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_ministral_3b() -> Model {
    Model {
        id: "mistral/ministral-3b".into(),
        name: "Ministral 3B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_ministral_8b() -> Model {
    Model {
        id: "mistral/ministral-8b".into(),
        name: "Ministral 8B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_mistral_medium() -> Model {
    Model {
        id: "mistral/mistral-medium".into(),
        name: "Mistral Medium 3.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_mistral_small() -> Model {
    Model {
        id: "mistral/mistral-small".into(),
        name: "Mistral Small".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 4_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_pixtral_12b() -> Model {
    Model {
        id: "mistral/pixtral-12b".into(),
        name: "Pixtral 12B 2409".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_pixtral_large() -> Model {
    Model {
        id: "mistral/pixtral-large".into(),
        name: "Pixtral Large".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_000.0,
        headers: None,
        compat: None,
    }
}
