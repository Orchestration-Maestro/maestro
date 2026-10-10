//! Authored model additions with shared provider defaults.
use crate::{Model, ModelCost, ModelInput};
/// Construct shared provider defaults.
pub(super) fn amazon_bedrock_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "bedrock-converse-stream",
            "amazon-bedrock",
            "https://bedrock-runtime.eu-central-1.amazonaws.com",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn anthropic_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "anthropic-messages",
            "anthropic",
            "https://api.anthropic.com",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn google_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "google-generative-ai",
            "google",
            "https://generativelanguage.googleapis.com/v1beta",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn openai_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "openai-responses",
            "openai",
            "https://api.openai.com/v1",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn deepseek_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 384_000.0,
        input: vec![ModelInput::Text],
        ..super::super::input::descriptor(
            String::new(),
            "openai-completions",
            "deepseek",
            "https://api.deepseek.com",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn openai_codex_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
        max_tokens: 128_000.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "openai-codex-responses",
            "openai-codex",
            "https://chatgpt.com/backend-api",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn xai_base() -> Model {
    Model {
        reasoning: false,
        cost: ModelCost {
            input: 0.2,
            output: 1.5,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 8192.0,
        input: vec![ModelInput::Text],
        ..super::super::input::descriptor(
            String::new(),
            "openai-completions",
            "xai",
            "https://api.x.ai/v1",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn mistral_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 1.5,
            output: 7.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "mistral-conversations",
            "mistral",
            "https://api.mistral.ai",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn openrouter_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "openai-completions",
            "openrouter",
            "https://openrouter.ai/api/v1",
        )
    }
}
/// Construct shared provider defaults.
pub(super) fn google_vertex_base() -> Model {
    Model {
        reasoning: true,
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        input: vec![ModelInput::Text, ModelInput::Image],
        ..super::super::input::descriptor(
            String::new(),
            "google-vertex",
            "google-vertex",
            "https://{location}-aiplatform.googleapis.com",
        )
    }
}
