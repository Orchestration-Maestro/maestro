use crate::ThrownValue;
mod platform;
use platform::{Host, Platform};
use std::sync::Mutex;
#[derive(Default)]
struct State {
    adc: Mutex<Option<bool>>,
}
static STATE: State = State {
    adc: Mutex::new(None),
};

/// Report nonempty environment key names in provider precedence order.
///
/// Ambient credentials are excluded. Values are not trimmed or cached. Native
/// execution reads the process environment. Browser mapped providers fail with
/// a missing-process error; unknown providers return no keys.
pub fn find_env_keys(provider: &str) -> Result<Option<Vec<String>>, ThrownValue> {
    find_env_keys_with_platform(&Host, provider)
}
/// Return the first configured environment value, or an ambient auth marker.
///
/// Values are reread after filtering in precedence order, without trimming.
/// Browser mapped providers and Bedrock fail with a missing-process error.
/// Host failures are returned and ordinary absence is `None`. Native execution
/// uses process facilities; browser hosts cannot probe native credential files.
/// Completed Vertex existence probes persist for the process.
pub fn get_env_api_key(provider: &str) -> Result<Option<String>, ThrownValue> {
    get_env_api_key_with_platform(&Host, &STATE, provider)
}
fn get_api_key_env_vars(provider: &str) -> Vec<&str> {
    match provider {
        "github-copilot" => vec!["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"],
        "anthropic" => vec!["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"],
        "openai" => vec!["OPENAI_API_KEY"],
        "azure-openai-responses" => vec!["AZURE_OPENAI_API_KEY"],
        "deepseek" => vec!["DEEPSEEK_API_KEY"],
        "google" => vec!["GEMINI_API_KEY"],
        "google-vertex" => vec!["GOOGLE_CLOUD_API_KEY"],
        "groq" => vec!["GROQ_API_KEY"],
        "cerebras" => vec!["CEREBRAS_API_KEY"],
        "xai" => vec!["XAI_API_KEY"],
        "openrouter" => vec!["OPENROUTER_API_KEY"],
        "vercel-ai-gateway" => vec!["AI_GATEWAY_API_KEY"],
        "zai" => vec!["ZAI_API_KEY"],
        "mistral" => vec!["MISTRAL_API_KEY"],
        "minimax" => vec!["MINIMAX_API_KEY"],
        "minimax-cn" => vec!["MINIMAX_CN_API_KEY"],
        "moonshotai" => vec!["MOONSHOT_API_KEY"],
        "moonshotai-cn" => vec!["MOONSHOT_API_KEY"],
        "huggingface" => vec!["HF_TOKEN"],
        "fireworks" => vec!["FIREWORKS_API_KEY"],
        "opencode" => vec!["OPENCODE_API_KEY"],
        "opencode-go" => vec!["OPENCODE_API_KEY"],
        "kimi-coding" => vec!["KIMI_API_KEY"],
        "cloudflare-workers-ai" => vec!["CLOUDFLARE_API_KEY"],
        "cloudflare-ai-gateway" => vec!["CLOUDFLARE_API_KEY"],
        "xiaomi" => vec!["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => vec!["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => vec!["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => vec!["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        "constructor" => vec!["function Object() { [native code] }"],
        "__defineGetter__" => vec!["function __defineGetter__() { [native code] }"],
        "__defineSetter__" => vec!["function __defineSetter__() { [native code] }"],
        "hasOwnProperty" => vec!["function hasOwnProperty() { [native code] }"],
        "__lookupGetter__" => vec!["function __lookupGetter__() { [native code] }"],
        "__lookupSetter__" => vec!["function __lookupSetter__() { [native code] }"],
        "isPrototypeOf" => vec!["function isPrototypeOf() { [native code] }"],
        "propertyIsEnumerable" => vec!["function propertyIsEnumerable() { [native code] }"],
        "toString" => vec!["function toString() { [native code] }"],
        "valueOf" => vec!["function valueOf() { [native code] }"],
        "__proto__" => vec!["[object Object]"],
        "toLocaleString" => vec!["function toLocaleString() { [native code] }"],
        _ => vec![],
    }
}
fn find_env_keys_with_platform(
    p: &impl Platform,
    provider: &str,
) -> Result<Option<Vec<String>>, ThrownValue> {
    let found = find_keys(p, provider)?;
    Ok((!found.is_empty()).then(|| found.into_iter().map(str::to_owned).collect()))
}
fn find_keys<'a>(p: &impl Platform, provider: &'a str) -> Result<Vec<&'a str>, ThrownValue> {
    let mut found = Vec::new();
    for key in get_api_key_env_vars(provider) {
        if nonempty(p.env(key)?).is_some() {
            found.push(key);
        }
    }
    Ok(found)
}
fn get_env_api_key_with_platform(
    p: &impl Platform,
    state: &State,
    provider: &str,
) -> Result<Option<String>, ThrownValue> {
    if let Some(key) = find_keys(p, provider)?.first() {
        return Ok(nonempty(p.env(key)?));
    }
    if provider == "google-vertex" {
        let credentials = has_vertex_adc_credentials(p, state)?;
        let project = nonempty(p.env("GOOGLE_CLOUD_PROJECT")?).is_some()
            || nonempty(p.env("GCLOUD_PROJECT")?).is_some();
        let location = nonempty(p.env("GOOGLE_CLOUD_LOCATION")?);
        if credentials && project && location.is_some() {
            return Ok(Some("<authenticated>".into()));
        }
    }
    if provider == "amazon-bedrock" && bedrock(|key| p.env(key))? {
        return Ok(Some("<authenticated>".into()));
    }
    Ok(None)
}
fn bedrock(
    mut read: impl FnMut(&str) -> Result<Option<String>, ThrownValue>,
) -> Result<bool, ThrownValue> {
    Ok(nonempty(read("AWS_PROFILE")?).is_some()
        || (nonempty(read("AWS_ACCESS_KEY_ID")?).is_some()
            && nonempty(read("AWS_SECRET_ACCESS_KEY")?).is_some())
        || nonempty(read("AWS_BEARER_TOKEN_BEDROCK")?).is_some()
        || nonempty(read("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI")?).is_some()
        || nonempty(read("AWS_CONTAINER_CREDENTIALS_FULL_URI")?).is_some()
        || nonempty(read("AWS_WEB_IDENTITY_TOKEN_FILE")?).is_some())
}
fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}
fn has_vertex_adc_credentials(p: &impl Platform, state: &State) -> Result<bool, ThrownValue> {
    if let Some(value) = *state.adc.lock().unwrap() {
        return Ok(value);
    }
    if !p.native()? {
        *state.adc.lock().unwrap() = Some(false);
        return Ok(false);
    }
    let path = match nonempty(p.env("GOOGLE_APPLICATION_CREDENTIALS")?) {
        Some(path) => path.into(),
        None => p.join(&p.home()?)?,
    };
    let exists = p.exists(&path);
    Ok(*state.adc.lock().unwrap().get_or_insert(exists))
}
#[cfg(test)]
#[path = "../../tests/support/environment_platform_checks.rs"]
mod environment_platform_checks;
