use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::providers::response_sessions::openai_codex_responses::{
    Nullable, OpenAICodexReasoningSummary, OpenAICodexTextVerbosity,
    close_openai_codex_web_socket_sessions, get_openai_codex_web_socket_debug_stats,
    reset_openai_codex_web_socket_debug_stats, stream_openai_codex_responses,
    stream_simple_openai_codex_responses,
};
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
    std::sync::Arc<std::sync::atomic::AtomicU8>,
) {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
    let seen = std::sync::Arc::clone(&calls);
    let client: maestro_models::AnthropicClient = std::sync::Arc::new(move |_, _| {
        seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    response_session_surface();
    bedrock_surface();
    std::hint::black_box((
        stream_bedrock,
        BedrockOptions::<maestro_models::Threaded>::default(),
    ));
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
        maestro_models::AzureOpenAIResponsesOptions::<maestro_models::Threaded>::default(),
        stream_openai_responses,
        stream_simple_openai_responses,
        register_built_in_api_providers,
        reset_api_providers,
        maestro_models::OpenAIResponsesOptions::<maestro_models::Threaded>::default(),
        login_openai_codex,
        OPENAI_CODEX_OAUTH_PROVIDER,
    ));
    drop(std::hint::black_box(refresh_openai_codex_token(
        "compile-only".into(),
        None,
    )));
    #[cfg(target_arch = "wasm32")]
    {
        response_session_callbacks();
        guide_surface();
        let mut objects = ProviderObjects::default();
        let (client, calls) = local_client();
        objects.insert(client);
        assert!(objects.get::<maestro_models::AnthropicClient>().is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
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

/// Compile provider records and dedicated/root function paths on either target.
fn response_session_surface() {
    cloud_content_surface();
    std::hint::black_box((
        maestro_models::OpenAICodexResponsesOptions::<maestro_models::Threaded>::default(),
        maestro_models::OpenAICodexWebSocketDebugStats::default(),
        Nullable::<OpenAICodexReasoningSummary>::Null,
        OpenAICodexTextVerbosity::Low,
        stream_openai_codex_responses,
        stream_simple_openai_codex_responses,
        maestro_models::stream_openai_codex_responses
            as fn(
                maestro_models::Model,
                maestro_models::Context,
                Option<maestro_models::OpenAICodexResponsesOptions>,
            ) -> maestro_models::AssistantMessageEventStream,
        maestro_models::stream_simple_openai_codex_responses
            as fn(
                maestro_models::Model,
                maestro_models::Context,
                Option<maestro_models::SimpleStreamOptions>,
            ) -> maestro_models::AssistantMessageEventStream,
        close_openai_codex_web_socket_sessions,
        get_openai_codex_web_socket_debug_stats,
        reset_openai_codex_web_socket_debug_stats,
    ));
}

/// Compile browser-local callbacks through both dedicated invocation signatures.
#[cfg(target_arch = "wasm32")]
fn response_session_callbacks() {
    let Some(model) = get_model("openai-codex", "gpt-5.5") else {
        return;
    };
    let state = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = std::rc::Rc::clone(&state);
    let common = maestro_models::StreamOptions::<maestro_models::Threaded> {
        on_payload: Some(maestro_models::SharedHandle::new(move |body, _| {
            seen.set(seen.get() + 1);
            Box::pin(async { Ok(body) })
        })),
        ..Default::default()
    };
    let context = maestro_models::Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    };
    drop(stream_openai_codex_responses(
        model.clone(),
        context.clone(),
        Some(maestro_models::OpenAICodexResponsesOptions {
            common: common.clone(),
            ..Default::default()
        }),
    ));
    drop(stream_simple_openai_codex_responses(
        model,
        context,
        Some(maestro_models::SimpleStreamOptions {
            common,
            ..Default::default()
        }),
    ));
}

/// Compile cloud-conversation exports and browser-local replacement captures.
fn bedrock_surface() {
    std::hint::black_box((
        maestro_models::set_bedrock_provider_module,
        maestro_models::bedrock_provider::bedrock_provider_module(),
    ));
    #[cfg(target_arch = "wasm32")]
    {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0_u8));
        let seen = std::rc::Rc::clone(&calls);
        let mut module = maestro_models::bedrock_provider::bedrock_provider_module();
        let owner = maestro_models::SharedHandle::new(BrowserProvider(seen));
        module.stream_bedrock = owner.clone();
        module.stream_simple_bedrock = owner;
        maestro_models::set_bedrock_provider_module(module);
        assert_eq!(calls.get(), 0);
    }
}

/// Compile guide records and an explicit-key, injectable browser invocation without running it.
#[cfg(target_arch = "wasm32")]
fn guide_surface() {
    use maestro_models::{
        AnthropicOptions, Context, Message, StreamOptions, Tool, UserContent, UserMessage,
    };
    factory_surface();
    let Some(model) = get_model("anthropic", "claude-sonnet-4-5") else {
        return;
    };
    let context = Context {
        system_prompt: Some("Be concise".into()),
        messages: vec![Message::User(UserMessage {
            content: UserContent::Text("Hello".into()),
            timestamp: 0.0,
        })],
        tools: Some(vec![Tool {
            name: "echo".into(),
            description: "Echo supplied text".into(),
            parameters: serde_json::json!({
                "type":"object", "properties":{"text":{"type":"string"}}, "required":["text"]
            })
            .into(),
        }]),
    };
    let (client, _) = local_client();
    let options = AnthropicOptions {
        common: StreamOptions {
            api_key: Some("explicit-key".into()),
            ..Default::default()
        },
        client: Some(client),
        ..Default::default()
    };
    let _invocation = std::hint::black_box(move || stream_anthropic(model, context, Some(options)));
}

/// Browser factory owners may retain single-thread state.
#[cfg(target_arch = "wasm32")]
struct BrowserFactory(std::rc::Rc<std::cell::Cell<u8>>);
#[cfg(target_arch = "wasm32")]
impl BrowserFactory {
    /// Retain the caller-local state in the original factory future.
    fn respond<E: maestro_models::ProviderExecution>(
        &self,
        input: maestro_models::FauxResponseInput<E>,
    ) -> impl std::future::Future<
        Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
    >
    + 'static
    + use<E> {
        let calls = std::rc::Rc::clone(&self.0);
        async move {
            std::future::ready(()).await;
            calls.set(calls.get() + 1);
            Ok(maestro_models::faux_assistant_message(
                input.state.call_count().to_string(),
                maestro_models::FauxAssistantMessageOptions::default(),
            ))
        }
    }
}
/// Erase the same factory future for the two browser invocation families.
#[cfg(target_arch = "wasm32")]
macro_rules! browser_factory {
    ($mode:ty) => {
        impl maestro_models::Invoke<$mode, maestro_models::FauxResponseInput<$mode>>
            for BrowserFactory
        {
            type Return = <$mode as maestro_models::Execution>::Future<
                'static,
                Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: maestro_models::FauxResponseInput<$mode>) -> Self::Return {
                Box::pin(self.respond(input))
            }
        }
    };
}
#[cfg(target_arch = "wasm32")]
browser_factory!(maestro_models::Local);
#[cfg(target_arch = "wasm32")]
browser_factory!(maestro_models::Threaded);

/// Admit an `Rc`-capturing factory through the public retained-factory type.
#[cfg(target_arch = "wasm32")]
fn factory_surface() {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let factory: maestro_models::FauxResponseFactory =
        maestro_models::SharedHandle::new(BrowserFactory(std::rc::Rc::clone(&calls)));
    std::hint::black_box(factory);
    assert_eq!(calls.get(), 0);
}

/// Browser registered owner may capture single-threaded state in either invocation family.
#[cfg(target_arch = "wasm32")]
struct BrowserProvider(std::rc::Rc<std::cell::Cell<u8>>);
#[cfg(target_arch = "wasm32")]
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for BrowserProvider
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _: maestro_models::ProviderCall<E, O>) -> Self::Return {
        self.0.set(self.0.get() + 1);
        Err(maestro_models::DiagnosticErrorInfo {
            name: None,
            message: String::new(),
            stack: None,
            code: None,
        })
    }
}

/// Compile cloud options and all root entry points on either target.
fn cloud_content_surface() {
    std::hint::black_box((
        maestro_models::GoogleVertexOptions::<maestro_models::Threaded>::default(),
        maestro_models::stream_google_vertex,
        maestro_models::stream_simple_google_vertex,
        maestro_models::stream_google_vertex_for::<maestro_models::Local>,
        maestro_models::stream_simple_google_vertex_for::<maestro_models::Local>,
    ));
}
