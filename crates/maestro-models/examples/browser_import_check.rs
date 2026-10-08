use maestro_models::{complete, get_model};

fn main() {
    let model = get_model("google", "gemini-2.5-flash");
    std::hint::black_box((model, complete));
    #[cfg(target_arch = "wasm32")]
    for provider in [
        "openai",
        "github-copilot",
        "anthropic",
        "google-vertex",
        "amazon-bedrock",
        "unknown",
        "toString",
        "constructor",
        "__proto__",
    ] {
        assert_eq!(maestro_models::find_env_keys(provider), None);
        assert_eq!(maestro_models::get_env_api_key(provider), None);
    }
}
