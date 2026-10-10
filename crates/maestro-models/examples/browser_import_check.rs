use maestro_models::{
    OPENAI_CODEX_OAUTH_PROVIDER, ProviderObjects, complete, get_model, login_openai_codex,
    refresh_openai_codex_token, register_built_in_api_providers, reset_api_providers,
    stream_anthropic, stream_azure_openai_responses, stream_mistral, stream_openai_completions,
    stream_openai_responses, stream_simple_anthropic, stream_simple_azure_openai_responses,
    stream_simple_mistral, stream_simple_openai_completions, stream_simple_openai_responses,
};

fn main() {
    let model = get_model("google", "gemini-2.5-flash");
    std::hint::black_box((
        model,
        complete,
        (
            stream_anthropic,
            stream_simple_anthropic,
            stream_openai_completions,
            stream_simple_openai_completions,
            stream_mistral,
            stream_simple_mistral,
        ),
        stream_azure_openai_responses,
        stream_simple_azure_openai_responses,
        maestro_models::AzureOpenAIResponsesOptions::default(),
        stream_openai_responses,
        stream_simple_openai_responses,
        register_built_in_api_providers,
        reset_api_providers,
        maestro_models::OpenAIResponsesOptions::default(),
        login_openai_codex,
        OPENAI_CODEX_OAUTH_PROVIDER,
    ));
    drop(std::hint::black_box(refresh_openai_codex_token(
        "compile-only".into(),
        None,
    )));
    #[cfg(target_arch = "wasm32")]
    {
        let mut objects = ProviderObjects::default();
        objects.insert(std::rc::Rc::new(1_u8));
        assert!(objects.get::<std::rc::Rc<u8>>().is_some());
    }
    #[cfg(not(target_arch = "wasm32"))]
    drop(ProviderObjects::default());
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
