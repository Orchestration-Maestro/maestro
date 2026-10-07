// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_64() -> [(&'static str, Model); 4] {
    [
        (
            "nvidia.nemotron-nano-12b-v2",
            nvidia_dot_nemotron_nano_12b_v2(),
        ),
        (
            "nvidia.nemotron-nano-3-30b",
            nvidia_dot_nemotron_nano_3_30b(),
        ),
        (
            "nvidia.nemotron-nano-9b-v2",
            nvidia_dot_nemotron_nano_9b_v2(),
        ),
        (
            "nvidia.nemotron-super-3-120b",
            nvidia_dot_nemotron_super_3_120b(),
        ),
    ]
}
fn nvidia_dot_nemotron_nano_12b_v2() -> Model {
    Model {
        id: "nvidia.nemotron-nano-12b-v2".into(),
        name: "NVIDIA Nemotron Nano 12B v2 VL BF16".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_dot_nemotron_nano_3_30b() -> Model {
    Model {
        id: "nvidia.nemotron-nano-3-30b".into(),
        name: "NVIDIA Nemotron Nano 3 30B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_dot_nemotron_nano_9b_v2() -> Model {
    Model {
        id: "nvidia.nemotron-nano-9b-v2".into(),
        name: "NVIDIA Nemotron Nano 9B v2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.23,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_dot_nemotron_super_3_120b() -> Model {
    Model {
        id: "nvidia.nemotron-super-3-120b".into(),
        name: "NVIDIA Nemotron 3 Super 120B A12B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.65,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
