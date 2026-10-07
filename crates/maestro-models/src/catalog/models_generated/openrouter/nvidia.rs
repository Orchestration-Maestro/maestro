// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_108() -> [(&'static str, Model); 10] {
    [
        (
            "nvidia/llama-3.1-nemotron-70b-instruct",
            nvidia_llama_3_dot_1_nemotron_70b_instruct(),
        ),
        (
            "nvidia/llama-3.3-nemotron-super-49b-v1.5",
            nvidia_llama_3_dot_3_nemotron_super_49b_v1_dot_5(),
        ),
        (
            "nvidia/nemotron-3-nano-30b-a3b",
            nvidia_nemotron_3_nano_30b_a3b(),
        ),
        (
            "nvidia/nemotron-3-nano-30b-a3b:free",
            nvidia_nemotron_3_nano_30b_a3b_free(),
        ),
        (
            "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
            nvidia_nemotron_3_nano_omni_30b_a3b_reasoning_free(),
        ),
        (
            "nvidia/nemotron-3-super-120b-a12b",
            nvidia_nemotron_3_super_120b_a12b(),
        ),
        (
            "nvidia/nemotron-3-super-120b-a12b:free",
            nvidia_nemotron_3_super_120b_a12b_free(),
        ),
        (
            "nvidia/nemotron-nano-12b-v2-vl:free",
            nvidia_nemotron_nano_12b_v2_vl_free(),
        ),
        ("nvidia/nemotron-nano-9b-v2", nvidia_nemotron_nano_9b_v2()),
        (
            "nvidia/nemotron-nano-9b-v2:free",
            nvidia_nemotron_nano_9b_v2_free(),
        ),
    ]
}
fn nvidia_llama_3_dot_1_nemotron_70b_instruct() -> Model {
    Model {
        id: "nvidia/llama-3.1-nemotron-70b-instruct".into(),
        name: "NVIDIA: Llama 3.1 Nemotron 70B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_llama_3_dot_3_nemotron_super_49b_v1_dot_5() -> Model {
    Model {
        id: "nvidia/llama-3.3-nemotron-super-49b-v1.5".into(),
        name: "NVIDIA: Llama 3.3 Nemotron Super 49B V1.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_3_nano_30b_a3b() -> Model {
    Model {
        id: "nvidia/nemotron-3-nano-30b-a3b".into(),
        name: "NVIDIA: Nemotron 3 Nano 30B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.199_999_999_999_999_98,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 228_000.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_3_nano_30b_a3b_free() -> Model {
    Model {
        id: "nvidia/nemotron-3-nano-30b-a3b:free".into(),
        name: "NVIDIA: Nemotron 3 Nano 30B A3B (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_3_nano_omni_30b_a3b_reasoning_free() -> Model {
    Model {
        id: "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free".into(),
        name: "NVIDIA: Nemotron 3 Nano Omni (free)".into(),
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
        context_window: 256_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_3_super_120b_a12b() -> Model {
    Model {
        id: "nvidia/nemotron-3-super-120b-a12b".into(),
        name: "NVIDIA: Nemotron 3 Super".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_3_super_120b_a12b_free() -> Model {
    Model {
        id: "nvidia/nemotron-3-super-120b-a12b:free".into(),
        name: "NVIDIA: Nemotron 3 Super (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_nano_12b_v2_vl_free() -> Model {
    Model {
        id: "nvidia/nemotron-nano-12b-v2-vl:free".into(),
        name: "NVIDIA: Nemotron Nano 12B 2 VL (free)".into(),
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
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn nvidia_nemotron_nano_9b_v2() -> Model {
    Model {
        id: "nvidia/nemotron-nano-9b-v2".into(),
        name: "NVIDIA: Nemotron Nano 9B V2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
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

fn nvidia_nemotron_nano_9b_v2_free() -> Model {
    Model {
        id: "nvidia/nemotron-nano-9b-v2:free".into(),
        name: "NVIDIA: Nemotron Nano 9B V2 (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
