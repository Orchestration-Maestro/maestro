// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_72() -> [(&'static str, Model); 7] {
    [
        (
            "qwen.qwen3-235b-a22b-2507-v1:0",
            qwen_dot_qwen3_235b_a22b_2507_v1_0(),
        ),
        ("qwen.qwen3-32b-v1:0", qwen_dot_qwen3_32b_v1_0()),
        (
            "qwen.qwen3-coder-30b-a3b-v1:0",
            qwen_dot_qwen3_coder_30b_a3b_v1_0(),
        ),
        (
            "qwen.qwen3-coder-480b-a35b-v1:0",
            qwen_dot_qwen3_coder_480b_a35b_v1_0(),
        ),
        ("qwen.qwen3-coder-next", qwen_dot_qwen3_coder_next()),
        ("qwen.qwen3-next-80b-a3b", qwen_dot_qwen3_next_80b_a3b()),
        ("qwen.qwen3-vl-235b-a22b", qwen_dot_qwen3_vl_235b_a22b()),
    ]
}
fn qwen_dot_qwen3_235b_a22b_2507_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-235b-a22b-2507-v1:0".into(),
        name: "Qwen3 235B A22B 2507".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.88,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn qwen_dot_qwen3_32b_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-32b-v1:0".into(),
        name: "Qwen3 32B (dense)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_384.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn qwen_dot_qwen3_coder_30b_a3b_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-coder-30b-a3b-v1:0".into(),
        name: "Qwen3 Coder 30B A3B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn qwen_dot_qwen3_coder_480b_a35b_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-coder-480b-a35b-v1:0".into(),
        name: "Qwen3 Coder 480B A35B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 1.8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_dot_qwen3_coder_next() -> Model {
    Model {
        id: "qwen.qwen3-coder-next".into(),
        name: "Qwen3 Coder Next".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 1.8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_dot_qwen3_next_80b_a3b() -> Model {
    Model {
        id: "qwen.qwen3-next-80b-a3b".into(),
        name: "Qwen/Qwen3-Next-80B-A3B-Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.14,
            output: 1.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}

fn qwen_dot_qwen3_vl_235b_a22b() -> Model {
    Model {
        id: "qwen.qwen3-vl-235b-a22b".into(),
        name: "Qwen/Qwen3-VL-235B-A22B-Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}
