// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 8] {
    [
        (
            "google/gemini-3.1-pro-preview",
            google_gemini_3_dot_1_pro_preview(),
        ),
        (
            "google/gemini-3.1-pro-preview-customtools",
            google_gemini_3_dot_1_pro_preview_customtools(),
        ),
        ("google/gemma-3-12b-it", google_gemma_3_12b_it()),
        ("google/gemma-3-27b-it", google_gemma_3_27b_it()),
        ("google/gemma-4-26b-a4b-it", google_gemma_4_26b_a4b_it()),
        (
            "google/gemma-4-26b-a4b-it:free",
            google_gemma_4_26b_a4b_it_free(),
        ),
        ("google/gemma-4-31b-it", google_gemma_4_31b_it()),
        ("google/gemma-4-31b-it:free", google_gemma_4_31b_it_free()),
    ]
}
fn google_gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview".into(),
        name: "Google: Gemini 3.1 Pro Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_dot_1_pro_preview_customtools() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview-customtools".into(),
        name: "Google: Gemini 3.1 Pro Preview Custom Tools".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_3_12b_it() -> Model {
    Model {
        id: "google/gemma-3-12b-it".into(),
        name: "Google: Gemma 3 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.04,
            output: 0.13,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_3_27b_it() -> Model {
    Model {
        id: "google/gemma-3-27b-it".into(),
        name: "Google: Gemma 3 27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.16,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_26b_a4b_it() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it".into(),
        name: "Google: Gemma 4 26B A4B ".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.33,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_26b_a4b_it_free() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it:free".into(),
        name: "Google: Gemma 4 26B A4B  (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_31b_it() -> Model {
    Model {
        id: "google/gemma-4-31b-it".into(),
        name: "Google: Gemma 4 31B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.38,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_31b_it_free() -> Model {
    Model {
        id: "google/gemma-4-31b-it:free".into(),
        name: "Google: Gemma 4 31B (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
