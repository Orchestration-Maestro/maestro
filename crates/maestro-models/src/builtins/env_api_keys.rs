use std::path::PathBuf;
use std::sync::OnceLock;

static VERTEX_ADC_EXISTS: OnceLock<bool> = OnceLock::new();

/// Report nonempty environment key names in provider precedence order.
///
/// Ambient credentials are excluded. Values are not trimmed or cached;
/// unknown providers and environments without variables return `None`.
pub fn find_env_keys(provider: &str) -> Option<Vec<String>> {
    let found: Vec<String> = get_api_key_env_vars(provider)
        .iter()
        .filter(|key| env_value(key).is_some())
        .map(|key| (*key).to_owned())
        .collect();
    (!found.is_empty()).then_some(found)
}

/// Return the first configured environment value, or `<authenticated>`.
///
/// Vertex requires an existing credential path, a project and a location.
/// Bedrock accepts its ambient credential signals without loading credentials.
/// The Vertex path existence result persists for the process lifetime.
/// Unknown providers and environments without credentials return `None`.
pub fn get_env_api_key(provider: &str) -> Option<String> {
    if let Some(keys) = find_env_keys(provider) {
        return env_value(&keys[0]);
    }
    let authenticated = match provider {
        "google-vertex" => {
            has_vertex_adc_credentials()
                && (env_value("GOOGLE_CLOUD_PROJECT").is_some()
                    || env_value("GCLOUD_PROJECT").is_some())
                && env_value("GOOGLE_CLOUD_LOCATION").is_some()
        }
        "amazon-bedrock" => {
            env_value("AWS_PROFILE").is_some()
                || (env_value("AWS_ACCESS_KEY_ID").is_some()
                    && env_value("AWS_SECRET_ACCESS_KEY").is_some())
                || [
                    "AWS_BEARER_TOKEN_BEDROCK",
                    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
                    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
                    "AWS_WEB_IDENTITY_TOKEN_FILE",
                ]
                .iter()
                .any(|key| env_value(key).is_some())
        }
        _ => false,
    };
    authenticated.then(|| "<authenticated>".to_owned())
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

#[allow(deprecated)]
fn has_vertex_adc_credentials() -> bool {
    *VERTEX_ADC_EXISTS.get_or_init(|| {
        let path = std::env::var_os("GOOGLE_APPLICATION_CREDENTIALS")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::home_dir().map(|home| {
                    home.join(".config")
                        .join("gcloud")
                        .join("application_default_credentials.json")
                })
            });
        path.is_some_and(|path| path.exists())
    })
}

fn get_api_key_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "github-copilot" => &["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"],
        "anthropic" => &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "azure-openai-responses" => &["AZURE_OPENAI_API_KEY"],
        "deepseek" => &["DEEPSEEK_API_KEY"],
        "google" => &["GEMINI_API_KEY"],
        "google-vertex" => &["GOOGLE_CLOUD_API_KEY"],
        "groq" => &["GROQ_API_KEY"],
        "cerebras" => &["CEREBRAS_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "vercel-ai-gateway" => &["AI_GATEWAY_API_KEY"],
        "zai" => &["ZAI_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "minimax" => &["MINIMAX_API_KEY"],
        "minimax-cn" => &["MINIMAX_CN_API_KEY"],
        "moonshotai" => &["MOONSHOT_API_KEY"],
        "moonshotai-cn" => &["MOONSHOT_API_KEY"],
        "huggingface" => &["HF_TOKEN"],
        "fireworks" => &["FIREWORKS_API_KEY"],
        "opencode" => &["OPENCODE_API_KEY"],
        "opencode-go" => &["OPENCODE_API_KEY"],
        "kimi-coding" => &["KIMI_API_KEY"],
        "cloudflare-workers-ai" => &["CLOUDFLARE_API_KEY"],
        "cloudflare-ai-gateway" => &["CLOUDFLARE_API_KEY"],
        "xiaomi" => &["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => &["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        _ => &[],
    }
}
