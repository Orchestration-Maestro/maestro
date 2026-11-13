//! Shared mutable provider display labels.
use indexmap::IndexMap;
use std::sync::{LazyLock, RwLock};

/// Shipped labels shared by registry lookups and callers.
pub static BUILT_IN_PROVIDER_DISPLAY_NAMES: LazyLock<RwLock<IndexMap<String, String>>> =
    LazyLock::new(|| {
        RwLock::new(
            [
                ("anthropic", "Anthropic"),
                ("amazon-bedrock", "Amazon Bedrock"),
                ("azure-openai-responses", "Azure OpenAI Responses"),
                ("cerebras", "Cerebras"),
                ("cloudflare-ai-gateway", "Cloudflare AI Gateway"),
                ("cloudflare-workers-ai", "Cloudflare Workers AI"),
                ("deepseek", "DeepSeek"),
                ("fireworks", "Fireworks"),
                ("google", "Google Gemini"),
                ("google-vertex", "Google Vertex AI"),
                ("groq", "Groq"),
                ("huggingface", "Hugging Face"),
                ("kimi-coding", "Kimi For Coding"),
                ("mistral", "Mistral"),
                ("minimax", "MiniMax"),
                ("minimax-cn", "MiniMax (China)"),
                ("moonshotai", "Moonshot AI"),
                ("moonshotai-cn", "Moonshot AI (China)"),
                ("opencode", "OpenCode Zen"),
                ("opencode-go", "OpenCode Go"),
                ("openai", "OpenAI"),
                ("openrouter", "OpenRouter"),
                ("vercel-ai-gateway", "Vercel AI Gateway"),
                ("xai", "xAI"),
                ("zai", "ZAI"),
                ("xiaomi", "Xiaomi MiMo"),
                ("xiaomi-token-plan-cn", "Xiaomi MiMo Token Plan (China)"),
                (
                    "xiaomi-token-plan-ams",
                    "Xiaomi MiMo Token Plan (Amsterdam)",
                ),
                (
                    "xiaomi-token-plan-sgp",
                    "Xiaomi MiMo Token Plan (Singapore)",
                ),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        )
    });
