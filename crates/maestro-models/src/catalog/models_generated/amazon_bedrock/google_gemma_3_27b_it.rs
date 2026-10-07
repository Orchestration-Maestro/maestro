// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        ("google.gemma-3-27b-it", google_dot_gemma_3_27b_it()),
        ("google.gemma-3-4b-it", google_dot_gemma_3_4b_it()),
    ]
}
fn google_dot_gemma_3_27b_it() -> Model {
    Model {
        id: "google.gemma-3-27b-it".into(),
        name: "Google Gemma 3 27B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.12,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn google_dot_gemma_3_4b_it() -> Model {
    Model {
        id: "google.gemma-3-4b-it".into(),
        name: "Gemma 3 4B IT".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.04,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}
