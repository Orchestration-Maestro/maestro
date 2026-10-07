// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_53() -> [(&'static str, Model); 9] {
    [
        ("mistral.devstral-2-123b", mistral_dot_devstral_2_123b()),
        (
            "mistral.magistral-small-2509",
            mistral_dot_magistral_small_2509(),
        ),
        (
            "mistral.ministral-3-14b-instruct",
            mistral_dot_ministral_3_14b_instruct(),
        ),
        (
            "mistral.ministral-3-3b-instruct",
            mistral_dot_ministral_3_3b_instruct(),
        ),
        (
            "mistral.ministral-3-8b-instruct",
            mistral_dot_ministral_3_8b_instruct(),
        ),
        (
            "mistral.mistral-large-3-675b-instruct",
            mistral_dot_mistral_large_3_675b_instruct(),
        ),
        (
            "mistral.pixtral-large-2502-v1:0",
            mistral_dot_pixtral_large_2502_v1_0(),
        ),
        (
            "mistral.voxtral-mini-3b-2507",
            mistral_dot_voxtral_mini_3b_2507(),
        ),
        (
            "mistral.voxtral-small-24b-2507",
            mistral_dot_voxtral_small_24b_2507(),
        ),
    ]
}
fn mistral_dot_devstral_2_123b() -> Model {
    Model {
        id: "mistral.devstral-2-123b".into(),
        name: "Devstral 2 123B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_magistral_small_2509() -> Model {
    Model {
        id: "mistral.magistral-small-2509".into(),
        name: "Magistral Small 1.2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_ministral_3_14b_instruct() -> Model {
    Model {
        id: "mistral.ministral-3-14b-instruct".into(),
        name: "Ministral 14B 3.0".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_ministral_3_3b_instruct() -> Model {
    Model {
        id: "mistral.ministral-3-3b-instruct".into(),
        name: "Ministral 3 3B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_ministral_3_8b_instruct() -> Model {
    Model {
        id: "mistral.ministral-3-8b-instruct".into(),
        name: "Ministral 3 8B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_mistral_large_3_675b_instruct() -> Model {
    Model {
        id: "mistral.mistral-large-3-675b-instruct".into(),
        name: "Mistral Large 3".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_pixtral_large_2502_v1_0() -> Model {
    Model {
        id: "mistral.pixtral-large-2502-v1:0".into(),
        name: "Pixtral Large (25.02)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_voxtral_mini_3b_2507() -> Model {
    Model {
        id: "mistral.voxtral-mini-3b-2507".into(),
        name: "Voxtral Mini 3B 2507".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
            output: 0.04,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn mistral_dot_voxtral_small_24b_2507() -> Model {
    Model {
        id: "mistral.voxtral-small-24b-2507".into(),
        name: "Voxtral Small 24B 2507".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.35,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
