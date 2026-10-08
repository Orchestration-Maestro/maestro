//! Generated provider registries in recorded order.

/// Recorded descriptors for amazon-bedrock.
mod amazon_bedrock;
/// Recorded descriptors for anthropic.
mod anthropic;
/// Recorded descriptors for azure-openai-responses.
mod azure_openai_responses;
/// Recorded descriptors for cerebras.
mod cerebras;
/// Recorded descriptors for cloudflare-ai-gateway.
mod cloudflare_ai_gateway;
/// Recorded descriptors for cloudflare-workers-ai.
mod cloudflare_workers_ai;
/// Recorded descriptors for deepseek.
mod deepseek;
/// Recorded descriptors for fireworks.
mod fireworks;
/// Recorded descriptors for github-copilot.
mod github_copilot;
/// Recorded descriptors for google.
mod google;
/// Recorded descriptors for google-vertex.
mod google_vertex;
/// Recorded descriptors for groq.
mod groq;
/// Recorded descriptors for huggingface.
mod huggingface;
/// Recorded descriptors for kimi-coding.
mod kimi_coding;
/// Recorded descriptors for minimax.
mod minimax;
/// Recorded descriptors for minimax-cn.
mod minimax_cn;
/// Recorded descriptors for mistral.
mod mistral;
/// Recorded descriptors for moonshotai.
mod moonshotai;
/// Recorded descriptors for moonshotai-cn.
mod moonshotai_cn;
/// Recorded descriptors for openai.
mod openai;
/// Recorded descriptors for openai-codex.
mod openai_codex;
/// Recorded descriptors for opencode.
mod opencode;
/// Recorded descriptors for opencode-go.
mod opencode_go;
/// Recorded descriptors for openrouter.
mod openrouter;
/// Recorded descriptors for vercel-ai-gateway.
mod vercel_ai_gateway;
/// Recorded descriptors for xai.
mod xai;
/// Recorded descriptors for xiaomi.
mod xiaomi;
/// Recorded descriptors for xiaomi-token-plan-ams.
mod xiaomi_token_plan_ams;
/// Recorded descriptors for xiaomi-token-plan-cn.
mod xiaomi_token_plan_cn;
/// Recorded descriptors for xiaomi-token-plan-sgp.
mod xiaomi_token_plan_sgp;
/// Recorded descriptors for zai.
mod zai;
/// Assemble all provider registries without changing their order.
pub(super) fn models() -> super::models::ModelRegistry {
    [
        ("amazon-bedrock", amazon_bedrock::models()),
        ("anthropic", anthropic::models()),
        ("azure-openai-responses", azure_openai_responses::models()),
        ("cerebras", cerebras::models()),
        ("cloudflare-ai-gateway", cloudflare_ai_gateway::models()),
        ("cloudflare-workers-ai", cloudflare_workers_ai::models()),
        ("deepseek", deepseek::models()),
        ("fireworks", fireworks::models()),
        ("github-copilot", github_copilot::models()),
        ("google", google::models()),
        ("google-vertex", google_vertex::models()),
        ("groq", groq::models()),
        ("huggingface", huggingface::models()),
        ("kimi-coding", kimi_coding::models()),
        ("minimax", minimax::models()),
        ("minimax-cn", minimax_cn::models()),
        ("mistral", mistral::models()),
        ("moonshotai", moonshotai::models()),
        ("moonshotai-cn", moonshotai_cn::models()),
        ("openai", openai::models()),
        ("openai-codex", openai_codex::models()),
        ("opencode", opencode::models()),
        ("opencode-go", opencode_go::models()),
        ("openrouter", openrouter::models()),
        ("vercel-ai-gateway", vercel_ai_gateway::models()),
        ("xai", xai::models()),
        ("xiaomi", xiaomi::models()),
        ("xiaomi-token-plan-ams", xiaomi_token_plan_ams::models()),
        ("xiaomi-token-plan-cn", xiaomi_token_plan_cn::models()),
        ("xiaomi-token-plan-sgp", xiaomi_token_plan_sgp::models()),
        ("zai", zai::models()),
    ]
    .into()
}
