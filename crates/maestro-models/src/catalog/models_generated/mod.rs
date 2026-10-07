// Generated model descriptor data.

mod amazon_bedrock;
mod anthropic;
mod azure_openai_responses;
mod cerebras;
mod cloudflare_ai_gateway;
mod cloudflare_workers_ai;
mod deepseek;
mod fireworks;
mod github_copilot;
mod google;
mod google_vertex;
mod groq;
mod huggingface;
mod kimi_coding;
mod minimax;
mod minimax_cn;
mod mistral;
mod moonshotai;
mod moonshotai_cn;
mod openai;
mod openai_codex;
mod opencode;
mod opencode_go;
mod openrouter;
mod vercel_ai_gateway;
mod xai;
mod xiaomi;
mod xiaomi_token_plan_ams;
mod xiaomi_token_plan_cn;
mod xiaomi_token_plan_sgp;
mod zai;
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
