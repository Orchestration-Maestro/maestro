// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_68() -> [(&'static str, Model); 4] {
    [
        ("openai.gpt-oss-120b-1:0", openai_dot_gpt_oss_120b_1_0()),
        ("openai.gpt-oss-20b-1:0", openai_dot_gpt_oss_20b_1_0()),
        (
            "openai.gpt-oss-safeguard-120b",
            openai_dot_gpt_oss_safeguard_120b(),
        ),
        (
            "openai.gpt-oss-safeguard-20b",
            openai_dot_gpt_oss_safeguard_20b(),
        ),
    ]
}
fn openai_dot_gpt_oss_120b_1_0() -> Model {
    Model {
        id: "openai.gpt-oss-120b-1:0".into(),
        name: "gpt-oss-120b".into(),
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
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openai_dot_gpt_oss_20b_1_0() -> Model {
    Model {
        id: "openai.gpt-oss-20b-1:0".into(),
        name: "gpt-oss-20b".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openai_dot_gpt_oss_safeguard_120b() -> Model {
    Model {
        id: "openai.gpt-oss-safeguard-120b".into(),
        name: "GPT OSS Safeguard 120B".into(),
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
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openai_dot_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai.gpt-oss-safeguard-20b".into(),
        name: "GPT OSS Safeguard 20B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
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
