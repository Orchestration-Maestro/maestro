use maestro_models::{
    OPENAI_CODEX_OAUTH_PROVIDER, ProviderObjects, complete, get_model, login_openai_codex,
    refresh_openai_codex_token, register_built_in_api_providers, reset_api_providers,
    stream_anthropic, stream_azure_openai_responses, stream_mistral, stream_openai_completions,
    stream_openai_responses, stream_simple_anthropic, stream_simple_azure_openai_responses,
    stream_simple_mistral, stream_simple_openai_completions, stream_simple_openai_responses,
};

/// A client holding single-threaded state, with that state's call counter.
#[cfg(target_arch = "wasm32")]
fn local_client() -> (
    maestro_models::AnthropicClient,
    std::rc::Rc<std::cell::Cell<u8>>,
) {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0_u8));
    let seen = std::rc::Rc::clone(&calls);
    let client: maestro_models::AnthropicClient = std::sync::Arc::new(move |_, _| {
        seen.set(seen.get() + 1);
        Box::pin(std::future::ready(Err(
            maestro_models::DiagnosticErrorInfo {
                name: None,
                message: String::new(),
                stack: None,
                code: None,
            },
        )))
    });
    (client, calls)
}

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
        let (client, calls) = local_client();
        objects.insert(client);
        assert!(objects.get::<maestro_models::AnthropicClient>().is_some());
        assert_eq!(calls.get(), 0);
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
