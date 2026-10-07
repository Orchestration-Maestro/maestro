// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_40() -> [(&'static str, Model); 10] {
    [
        (
            "meta.llama3-1-405b-instruct-v1:0",
            meta_dot_llama3_1_405b_instruct_v1_0(),
        ),
        (
            "meta.llama3-1-70b-instruct-v1:0",
            meta_dot_llama3_1_70b_instruct_v1_0(),
        ),
        (
            "meta.llama3-1-8b-instruct-v1:0",
            meta_dot_llama3_1_8b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-11b-instruct-v1:0",
            meta_dot_llama3_2_11b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-1b-instruct-v1:0",
            meta_dot_llama3_2_1b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-3b-instruct-v1:0",
            meta_dot_llama3_2_3b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-90b-instruct-v1:0",
            meta_dot_llama3_2_90b_instruct_v1_0(),
        ),
        (
            "meta.llama3-3-70b-instruct-v1:0",
            meta_dot_llama3_3_70b_instruct_v1_0(),
        ),
        (
            "meta.llama4-maverick-17b-instruct-v1:0",
            meta_dot_llama4_maverick_17b_instruct_v1_0(),
        ),
        (
            "meta.llama4-scout-17b-instruct-v1:0",
            meta_dot_llama4_scout_17b_instruct_v1_0(),
        ),
    ]
}
fn meta_dot_llama3_1_405b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-1-405b-instruct-v1:0".into(),
        name: "Llama 3.1 405B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.4,
            output: 2.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_1_70b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-1-70b-instruct-v1:0".into(),
        name: "Llama 3.1 70B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_1_8b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-1-8b-instruct-v1:0".into(),
        name: "Llama 3.1 8B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.22,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_2_11b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-11b-instruct-v1:0".into(),
        name: "Llama 3.2 11B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.16,
            output: 0.16,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_2_1b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-1b-instruct-v1:0".into(),
        name: "Llama 3.2 1B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_2_3b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-3b-instruct-v1:0".into(),
        name: "Llama 3.2 3B Instruct".into(),
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
        context_window: 131_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_2_90b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-90b-instruct-v1:0".into(),
        name: "Llama 3.2 90B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama3_3_70b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-3-70b-instruct-v1:0".into(),
        name: "Llama 3.3 70B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama4_maverick_17b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama4-maverick-17b-instruct-v1:0".into(),
        name: "Llama 4 Maverick 17B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.24,
            output: 0.97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn meta_dot_llama4_scout_17b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama4-scout-17b-instruct-v1:0".into(),
        name: "Llama 4 Scout 17B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.17,
            output: 0.66,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 3_500_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
