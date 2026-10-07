// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_22() -> [(&'static str, Model); 4] {
    [
        (
            "arcee-ai/trinity-large-preview",
            arcee_ai_trinity_large_preview(),
        ),
        (
            "arcee-ai/trinity-large-thinking",
            arcee_ai_trinity_large_thinking(),
        ),
        ("arcee-ai/trinity-mini", arcee_ai_trinity_mini()),
        ("arcee-ai/virtuoso-large", arcee_ai_virtuoso_large()),
    ]
}
fn arcee_ai_trinity_large_preview() -> Model {
    Model {
        id: "arcee-ai/trinity-large-preview".into(),
        name: "Arcee AI: Trinity Large Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn arcee_ai_trinity_large_thinking() -> Model {
    Model {
        id: "arcee-ai/trinity-large-thinking".into(),
        name: "Arcee AI: Trinity Large Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.85,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn arcee_ai_trinity_mini() -> Model {
    Model {
        id: "arcee-ai/trinity-mini".into(),
        name: "Arcee AI: Trinity Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.045,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn arcee_ai_virtuoso_large() -> Model {
    Model {
        id: "arcee-ai/virtuoso-large".into(),
        name: "Arcee AI: Virtuoso Large".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.75,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}
