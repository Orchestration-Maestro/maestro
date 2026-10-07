// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 5] {
    [
        ("amazon.nova-2-lite-v1:0", amazon_dot_nova_2_lite_v1_0()),
        ("amazon.nova-lite-v1:0", amazon_dot_nova_lite_v1_0()),
        ("amazon.nova-micro-v1:0", amazon_dot_nova_micro_v1_0()),
        ("amazon.nova-premier-v1:0", amazon_dot_nova_premier_v1_0()),
        ("amazon.nova-pro-v1:0", amazon_dot_nova_pro_v1_0()),
    ]
}
fn amazon_dot_nova_2_lite_v1_0() -> Model {
    Model {
        id: "amazon.nova-2-lite-v1:0".into(),
        name: "Nova 2 Lite".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.33,
            output: 2.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn amazon_dot_nova_lite_v1_0() -> Model {
    Model {
        id: "amazon.nova-lite-v1:0".into(),
        name: "Nova Lite".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn amazon_dot_nova_micro_v1_0() -> Model {
    Model {
        id: "amazon.nova-micro-v1:0".into(),
        name: "Nova Micro".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.035,
            output: 0.14,
            cache_read: 0.008_75,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn amazon_dot_nova_premier_v1_0() -> Model {
    Model {
        id: "amazon.nova-premier-v1:0".into(),
        name: "Nova Premier".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 12.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn amazon_dot_nova_pro_v1_0() -> Model {
    Model {
        id: "amazon.nova-pro-v1:0".into(),
        name: "Nova Pro".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.8,
            output: 3.2,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
