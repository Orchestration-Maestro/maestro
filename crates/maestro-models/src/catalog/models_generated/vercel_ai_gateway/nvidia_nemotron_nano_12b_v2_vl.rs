// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        (
            "nvidia/nemotron-nano-12b-v2-vl",
            nvidia_nemotron_nano_12b_v2_vl(),
        ),
        ("nvidia/nemotron-nano-9b-v2", nvidia_nemotron_nano_9b_v2()),
    ]
}
fn nvidia_nemotron_nano_12b_v2_vl() -> Model {
    Model {
        id: "nvidia/nemotron-nano-12b-v2-vl".into(),
        name: "Nvidia Nemotron Nano 12B V2 VL".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_nano_9b_v2() -> Model {
    Model {
        id: "nvidia/nemotron-nano-9b-v2".into(),
        name: "Nvidia Nemotron Nano 9B V2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.229_999_999_999_999_98,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
