//! Authored model additions with shared provider defaults.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat, ThinkingFormat};
mod defaults;
use defaults::{
    amazon_bedrock_base, anthropic_base, deepseek_base, google_base, google_vertex_base,
    mistral_base, openai_base, openai_codex_base, openrouter_base, xai_base,
};
/// Authored constructor order.
const INITIAL: &[fn() -> Model] = &[
    amazon_bedrock_eu_anthropic_claude_opus_4_6_v1,
    anthropic_claude_opus_4_6,
    anthropic_claude_opus_4_7,
    anthropic_claude_sonnet_4_6,
    google_gemini_3_1_flash_lite_preview,
    openai_gpt_5_chat_latest,
    openai_gpt_5_1_codex,
    openai_gpt_5_1_codex_max,
    openai_gpt_5_3_codex_spark,
    openai_gpt_5_4,
];
/// Construct additions in their authored order.
pub(super) fn initial() -> Vec<Model> {
    INITIAL.iter().map(|constructor| constructor()).collect()
}

/// Authored constructor order.
const DEEPSEEK: &[fn() -> Model] = &[deepseek_deepseek_v4_flash, deepseek_deepseek_v4_pro];
/// Construct additions in their authored order.
pub(super) fn deepseek() -> Vec<Model> {
    DEEPSEEK.iter().map(|constructor| constructor()).collect()
}

/// Authored constructor order.
const CODEX: &[fn() -> Model] = &[
    openai_codex_gpt_5_1,
    openai_codex_gpt_5_1_codex_max,
    openai_codex_gpt_5_1_codex_mini,
    openai_codex_gpt_5_2,
    openai_codex_gpt_5_2_codex,
    openai_codex_gpt_5_3_codex,
    openai_codex_gpt_5_4,
    openai_codex_gpt_5_5,
    openai_codex_gpt_5_4_mini,
    openai_codex_gpt_5_3_codex_spark,
];
/// Construct additions in their authored order.
pub(super) fn codex() -> Vec<Model> {
    CODEX.iter().map(|constructor| constructor()).collect()
}

/// Authored constructor order.
const REMAINING: &[fn() -> Model] = &[
    xai_grok_code_fast_1,
    mistral_mistral_medium_3_5,
    openrouter_auto,
    google_vertex_gemini_3_pro_preview,
    google_vertex_gemini_3_1_pro_preview,
    google_vertex_gemini_3_1_pro_preview_customtools,
    google_vertex_gemini_3_flash_preview,
    google_vertex_gemini_2_0_flash,
    google_vertex_gemini_2_0_flash_lite,
    google_vertex_gemini_2_5_pro,
    google_vertex_gemini_2_5_flash,
    google_vertex_gemini_2_5_flash_lite_preview_09_2025,
    google_vertex_gemini_2_5_flash_lite,
    google_vertex_gemini_1_5_pro,
    google_vertex_gemini_1_5_flash,
    google_vertex_gemini_1_5_flash_8b,
];
/// Construct additions in their authored order.
pub(super) fn remaining() -> Vec<Model> {
    REMAINING.iter().map(|constructor| constructor()).collect()
}

/// Construct an authored model descriptor.
fn amazon_bedrock_eu_anthropic_claude_opus_4_6_v1() -> Model {
    Model {
        id: "eu.anthropic.claude-opus-4-6-v1".into(),
        name: "Claude Opus 4.6 (EU)".into(),
        ..amazon_bedrock_base()
    }
}

/// Construct an authored model descriptor.
fn anthropic_claude_opus_4_6() -> Model {
    Model {
        id: "claude-opus-4-6".into(),
        name: "Claude Opus 4.6".into(),
        ..anthropic_base()
    }
}

/// Construct an authored model descriptor.
fn anthropic_claude_opus_4_7() -> Model {
    Model {
        id: "claude-opus-4-7".into(),
        name: "Claude Opus 4.7".into(),
        ..anthropic_base()
    }
}

/// Construct an authored model descriptor.
fn anthropic_claude_sonnet_4_6() -> Model {
    Model {
        id: "claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6".into(),
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        max_tokens: 64_000.0,
        ..anthropic_base()
    }
}

/// Construct an authored model descriptor.
fn google_gemini_3_1_flash_lite_preview() -> Model {
    Model {
        id: "gemini-3.1-flash-lite-preview".into(),
        name: "Gemini 3.1 Flash Lite Preview".into(),
        ..google_base()
    }
}

/// Construct an authored model descriptor.
fn openai_gpt_5_chat_latest() -> Model {
    Model {
        id: "gpt-5-chat-latest".into(),
        name: "GPT-5 Chat Latest".into(),
        reasoning: false,
        max_tokens: 16_384.0,
        ..openai_base()
    }
}

/// Construct an authored model descriptor.
fn openai_gpt_5_1_codex() -> Model {
    Model {
        id: "gpt-5.1-codex".into(),
        name: "GPT-5.1 Codex".into(),
        cost: ModelCost {
            input: 1.25,
            output: 5.0,
            cache_read: 0.125,
            cache_write: 1.25,
        },
        context_window: 400_000.0,
        ..openai_base()
    }
}

/// Construct an authored model descriptor.
fn openai_gpt_5_1_codex_max() -> Model {
    Model {
        id: "gpt-5.1-codex-max".into(),
        name: "GPT-5.1 Codex Max".into(),
        context_window: 400_000.0,
        ..openai_base()
    }
}

/// Construct an authored model descriptor.
fn openai_gpt_5_3_codex_spark() -> Model {
    Model {
        id: "gpt-5.3-codex-spark".into(),
        name: "GPT-5.3 Codex Spark".into(),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        max_tokens: 16_384.0,
        ..openai_base()
    }
}

/// Construct an authored model descriptor.
fn openai_gpt_5_4() -> Model {
    Model {
        id: "gpt-5.4".into(),
        name: "GPT-5.4".into(),
        cost: ModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
        ..openai_base()
    }
}

/// Construct an authored model descriptor.
fn deepseek_deepseek_v4_flash() -> Model {
    Model {
        id: "deepseek-v4-flash".into(),
        name: "DeepSeek V4 Flash".into(),
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                requires_reasoning_content_on_assistant_messages: Some(true),
                thinking_format: Some(ThinkingFormat::Deepseek),
                ..OpenAICompletionsCompat::default()
            },
        ))),
        ..deepseek_base()
    }
}

/// Construct an authored model descriptor.
fn deepseek_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek-v4-pro".into(),
        name: "DeepSeek V4 Pro".into(),
        cost: ModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.003_625,
            cache_write: 0.0,
        },
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                requires_reasoning_content_on_assistant_messages: Some(true),
                thinking_format: Some(ThinkingFormat::Deepseek),
                ..OpenAICompletionsCompat::default()
            },
        ))),
        ..deepseek_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_1() -> Model {
    Model {
        id: "gpt-5.1".into(),
        name: "GPT-5.1".into(),
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_1_codex_max() -> Model {
    Model {
        id: "gpt-5.1-codex-max".into(),
        name: "GPT-5.1 Codex Max".into(),
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_1_codex_mini() -> Model {
    Model {
        id: "gpt-5.1-codex-mini".into(),
        name: "GPT-5.1 Codex Mini".into(),
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_2() -> Model {
    Model {
        id: "gpt-5.2".into(),
        name: "GPT-5.2".into(),
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_2_codex() -> Model {
    Model {
        id: "gpt-5.2-codex".into(),
        name: "GPT-5.2 Codex".into(),
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_3_codex() -> Model {
    Model {
        id: "gpt-5.3-codex".into(),
        name: "GPT-5.3 Codex".into(),
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_4() -> Model {
    Model {
        id: "gpt-5.4".into(),
        name: "GPT-5.4".into(),
        cost: ModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_5() -> Model {
    Model {
        id: "gpt-5.5".into(),
        name: "GPT-5.5".into(),
        cost: ModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_4_mini() -> Model {
    Model {
        id: "gpt-5.4-mini".into(),
        name: "GPT-5.4 Mini".into(),
        cost: ModelCost {
            input: 0.75,
            output: 4.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn openai_codex_gpt_5_3_codex_spark() -> Model {
    Model {
        id: "gpt-5.3-codex-spark".into(),
        name: "GPT-5.3 Codex Spark".into(),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        ..openai_codex_base()
    }
}

/// Construct an authored model descriptor.
fn xai_grok_code_fast_1() -> Model {
    Model {
        id: "grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        ..xai_base()
    }
}

/// Construct an authored model descriptor.
fn mistral_mistral_medium_3_5() -> Model {
    Model {
        id: "mistral-medium-3.5".into(),
        name: "Mistral Medium 3.5".into(),
        ..mistral_base()
    }
}

/// Construct an authored model descriptor.
fn openrouter_auto() -> Model {
    Model {
        id: "auto".into(),
        name: "Auto".into(),
        ..openrouter_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_3_pro_preview() -> Model {
    Model {
        id: "gemini-3-pro-preview".into(),
        name: "Gemini 3 Pro Preview (Vertex)".into(),
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_3_1_pro_preview() -> Model {
    Model {
        id: "gemini-3.1-pro-preview".into(),
        name: "Gemini 3.1 Pro Preview (Vertex)".into(),
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_3_1_pro_preview_customtools() -> Model {
    Model {
        id: "gemini-3.1-pro-preview-customtools".into(),
        name: "Gemini 3.1 Pro Preview Custom Tools (Vertex)".into(),
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_3_flash_preview() -> Model {
    Model {
        id: "gemini-3-flash-preview".into(),
        name: "Gemini 3 Flash Preview (Vertex)".into(),
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_2_0_flash() -> Model {
    Model {
        id: "gemini-2.0-flash".into(),
        name: "Gemini 2.0 Flash (Vertex)".into(),
        reasoning: false,
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0375,
            cache_write: 0.0,
        },
        max_tokens: 8192.0,
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_2_0_flash_lite() -> Model {
    Model {
        id: "gemini-2.0-flash-lite".into(),
        name: "Gemini 2.0 Flash Lite (Vertex)".into(),
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.01875,
            cache_write: 0.0,
        },
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_2_5_pro() -> Model {
    Model {
        id: "gemini-2.5-pro".into(),
        name: "Gemini 2.5 Pro (Vertex)".into(),
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_2_5_flash() -> Model {
    Model {
        id: "gemini-2.5-flash".into(),
        name: "Gemini 2.5 Flash (Vertex)".into(),
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_2_5_flash_lite_preview_09_2025() -> Model {
    Model {
        id: "gemini-2.5-flash-lite-preview-09-2025".into(),
        name: "Gemini 2.5 Flash Lite Preview 09-25 (Vertex)".into(),
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_2_5_flash_lite() -> Model {
    Model {
        id: "gemini-2.5-flash-lite".into(),
        name: "Gemini 2.5 Flash Lite (Vertex)".into(),
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_1_5_pro() -> Model {
    Model {
        id: "gemini-1.5-pro".into(),
        name: "Gemini 1.5 Pro (Vertex)".into(),
        reasoning: false,
        cost: ModelCost {
            input: 1.25,
            output: 5.0,
            cache_read: 0.3125,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8192.0,
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_1_5_flash() -> Model {
    Model {
        id: "gemini-1.5-flash".into(),
        name: "Gemini 1.5 Flash (Vertex)".into(),
        reasoning: false,
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.01875,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8192.0,
        ..google_vertex_base()
    }
}

/// Construct an authored model descriptor.
fn google_vertex_gemini_1_5_flash_8b() -> Model {
    Model {
        id: "gemini-1.5-flash-8b".into(),
        name: "Gemini 1.5 Flash-8B (Vertex)".into(),
        reasoning: false,
        cost: ModelCost {
            input: 0.0375,
            output: 0.15,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8192.0,
        ..google_vertex_base()
    }
}
