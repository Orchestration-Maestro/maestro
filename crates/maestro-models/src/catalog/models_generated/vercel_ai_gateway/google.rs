// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_42() -> [(&'static str, Model); 10] {
    [
        ("google/gemini-2.0-flash", google_gemini_2_dot_0_flash()),
        (
            "google/gemini-2.0-flash-lite",
            google_gemini_2_dot_0_flash_lite(),
        ),
        ("google/gemini-2.5-flash", google_gemini_2_dot_5_flash()),
        (
            "google/gemini-2.5-flash-lite",
            google_gemini_2_dot_5_flash_lite(),
        ),
        ("google/gemini-2.5-pro", google_gemini_2_dot_5_pro()),
        ("google/gemini-3-flash", google_gemini_3_flash()),
        ("google/gemini-3-pro-preview", google_gemini_3_pro_preview()),
        (
            "google/gemini-3.1-flash-lite-preview",
            google_gemini_3_dot_1_flash_lite_preview(),
        ),
        (
            "google/gemini-3.1-pro-preview",
            google_gemini_3_dot_1_pro_preview(),
        ),
        ("google/gemma-4-26b-a4b-it", google_gemma_4_26b_a4b_it()),
    ]
}
pub(super) fn models_52() -> [(&'static str, Model); 1] {
    [("google/gemma-4-31b-it", google_gemma_4_31b_it())]
}
fn google_gemini_2_dot_0_flash() -> Model {
    Model {
        id: "google/gemini-2.0-flash".into(),
        name: "Gemini 2.0 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_0_flash_lite() -> Model {
    Model {
        id: "google/gemini-2.0-flash-lite".into(),
        name: "Gemini 2.0 Flash Lite".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_flash() -> Model {
    Model {
        id: "google/gemini-2.5-flash".into(),
        name: "Gemini 2.5 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_flash_lite() -> Model {
    Model {
        id: "google/gemini-2.5-flash-lite".into(),
        name: "Gemini 2.5 Flash Lite".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_pro() -> Model {
    Model {
        id: "google/gemini-2.5-pro".into(),
        name: "Gemini 2.5 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_flash() -> Model {
    Model {
        id: "google/gemini-3-flash".into(),
        name: "Gemini 3 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_000.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_pro_preview() -> Model {
    Model {
        id: "google/gemini-3-pro-preview".into(),
        name: "Gemini 3 Pro Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_dot_1_flash_lite_preview() -> Model {
    Model {
        id: "google/gemini-3.1-flash-lite-preview".into(),
        name: "Gemini 3.1 Flash Lite Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_000.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview".into(),
        name: "Gemini 3.1 Pro Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_26b_a4b_it() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it".into(),
        name: "Gemma 4 26B A4B IT".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_31b_it() -> Model {
    Model {
        id: "google/gemma-4-31b-it".into(),
        name: "Gemma 4 31B IT".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.14,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
