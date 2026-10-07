// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_126() -> [(&'static str, Model); 2] {
    [
        ("perplexity/sonar", perplexity_sonar()),
        ("perplexity/sonar-pro", perplexity_sonar_pro()),
    ]
}
fn perplexity_sonar() -> Model {
    Model {
        id: "perplexity/sonar".into(),
        name: "Sonar".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 127_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

fn perplexity_sonar_pro() -> Model {
    Model {
        id: "perplexity/sonar-pro".into(),
        name: "Sonar Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}
