use maestro_models::{
    OPENAI_CODEX_OAUTH_PROVIDER, complete, get_model, login_openai_codex,
    refresh_openai_codex_token,
};

fn main() {
    let model = get_model("google", "gemini-2.5-flash");
    std::hint::black_box((
        model,
        complete,
        login_openai_codex,
        OPENAI_CODEX_OAUTH_PROVIDER,
    ));
    drop(std::hint::black_box(refresh_openai_codex_token(
        "compile-only".into(),
        None,
    )));
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
