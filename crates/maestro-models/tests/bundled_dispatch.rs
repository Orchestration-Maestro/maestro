//! Bundled protocols reached through the public registry, generic dispatch and root exports.
use crate::bundled;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::key_selection;

#[cfg(not(target_arch = "wasm32"))]
use bundled::Protocol;
use bundled::{PROTOCOLS, registered};
#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::clear_api_providers;
use maestro_models::records::api_registry::get_api_provider;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{
    AnthropicOptions, AssistantMessageEvent, AssistantMessageEventStream,
    AzureOpenAIResponsesOptions, MistralOptions, OnPayload, OnResponse,
    OpenAICodexResponsesOptions, OpenAICompletionsOptions, OpenAIResponsesOptions,
    ProviderStreamOptions, SharedAssistantMessage, SimpleStreamOptions, StopReason, StreamOptions,
    complete, complete_simple, stream,
};
use maestro_models::{ApiProvider, DiagnosticErrorInfo, SharedHandle, stream_simple};
use maestro_models::{register_api_provider, reset_api_providers, unregister_api_providers};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::json;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex, PoisonError};

#[cfg(not(target_arch = "wasm32"))]
/// Expected endpoint paths, owned by the native request observations.
const EXPECTED_PATHS: [(&str, &str); 6] = [
    (PROTOCOLS[0].api, "/v1/messages"),
    (PROTOCOLS[1].api, "/v1/chat/completions"),
    (PROTOCOLS[2].api, "/v1/chat/completions"),
    (PROTOCOLS[3].api, "/v1/responses"),
    (PROTOCOLS[4].api, "/v1/responses"),
    (PROTOCOLS[5].api, "/codex/responses"),
];

#[cfg(not(target_arch = "wasm32"))]
/// How a public operation reaches the protocol.
#[derive(Clone, Copy, Debug)]
enum Route {
    GenericStream,
    GenericComplete,
    GenericSimpleStream,
    GenericSimpleComplete,
    RootRaw,
    RootSimple,
}

#[cfg(not(target_arch = "wasm32"))]
const ROUTES: [Route; 6] = [
    Route::GenericStream,
    Route::GenericComplete,
    Route::GenericSimpleStream,
    Route::GenericSimpleComplete,
    Route::RootRaw,
    Route::RootSimple,
];

#[cfg(not(target_arch = "wasm32"))]
/// What one invocation observed.
struct Observed {
    result: SharedAssistantMessage,
    requests: Vec<bundled::Seen>,
    hooks: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
/// Common options with a key, the recording transport and logging hooks.
fn common(
    text: &str,
    protocol: &Protocol,
) -> (StreamOptions, bundled::Requests, Arc<Mutex<Vec<String>>>) {
    let (fetch, requests) = bundled::fetch(bundled::text_body(protocol.api, text));
    let hooks = Arc::new(Mutex::new(Vec::new()));
    let (payloads, responses) = (Arc::clone(&hooks), Arc::clone(&hooks));
    let on_payload: OnPayload = Arc::new(move |payload, _| {
        payloads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push("payload".into());
        Box::pin(async move { Ok(payload) })
    });
    let on_response: OnResponse = Arc::new(move |response, _| {
        responses
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!("response {}", response.status));
        Box::pin(async { Ok(()) })
    });
    let options = StreamOptions {
        api_key: Some(bundled::api_key(protocol.api)),
        transport: Some(maestro_models::Transport::Sse),
        fetch: Some(fetch),
        on_payload: Some(on_payload),
        on_response: Some(on_response),
        ..StreamOptions::default()
    };
    (options, requests, hooks)
}

#[cfg(not(target_arch = "wasm32"))]
/// Start the typed root raw entry of `protocol`.
fn root_raw(protocol: &Protocol, common: StreamOptions) -> TestResult<AssistantMessageEventStream> {
    let model = bundled::protocol_model(protocol)?;
    let context = bundled::conversation()?;
    Ok(match protocol.api {
        "anthropic-messages" => maestro_models::stream_anthropic(
            model,
            context,
            Some(AnthropicOptions {
                common,
                ..AnthropicOptions::default()
            }),
        ),
        "openai-completions" => maestro_models::stream_openai_completions(
            model,
            context,
            Some(OpenAICompletionsOptions {
                common,
                ..OpenAICompletionsOptions::default()
            }),
        ),
        "mistral-conversations" => maestro_models::stream_mistral(
            model,
            context,
            Some(MistralOptions {
                common,
                ..MistralOptions::default()
            }),
        ),
        "openai-responses" => maestro_models::stream_openai_responses(
            model,
            context,
            Some(OpenAIResponsesOptions {
                common,
                ..OpenAIResponsesOptions::default()
            }),
        ),
        "azure-openai-responses" => maestro_models::stream_azure_openai_responses(
            model,
            context,
            Some(AzureOpenAIResponsesOptions {
                common,
                ..AzureOpenAIResponsesOptions::default()
            }),
        ),
        "openai-codex-responses" => maestro_models::stream_openai_codex_responses(
            model,
            context,
            Some(OpenAICodexResponsesOptions {
                common,
                ..Default::default()
            }),
        ),
        _ => return Err("unknown protocol".into()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Start the typed root simple entry of `protocol`.
fn root_simple(
    protocol: &Protocol,
    options: Option<SimpleStreamOptions>,
) -> TestResult<AssistantMessageEventStream> {
    let (model, context) = (bundled::protocol_model(protocol)?, bundled::conversation()?);
    Ok(match protocol.api {
        "anthropic-messages" => maestro_models::stream_simple_anthropic(model, context, options),
        "openai-completions" => {
            maestro_models::stream_simple_openai_completions(model, context, options)
        }
        "mistral-conversations" => maestro_models::stream_simple_mistral(model, context, options),
        "openai-responses" => {
            maestro_models::stream_simple_openai_responses(model, context, options)
        }
        "azure-openai-responses" => {
            maestro_models::stream_simple_azure_openai_responses(model, context, options)
        }
        "openai-codex-responses" => {
            maestro_models::stream_simple_openai_codex_responses(model, context, options)
        }
        _ => return Err("unknown protocol".into()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Invoke `protocol` through `route` with controlled wire text.
async fn invoke(protocol: &Protocol, route: Route, text: &str) -> TestResult<Observed> {
    let (common, requests, hooks) = common(text, protocol);
    let (model, context) = (bundled::protocol_model(protocol)?, bundled::conversation()?);
    let simple = Some(SimpleStreamOptions {
        common: common.clone(),
        ..SimpleStreamOptions::default()
    });
    let raw = Some(ProviderStreamOptions {
        common: common.clone(),
        ..ProviderStreamOptions::default()
    });
    let result = match route {
        Route::GenericStream => {
            let stream = stream(model, context, raw)?;
            bundled::drain(&stream).await;
            stream.result().await
        }
        Route::GenericSimpleStream => {
            let stream = stream_simple(model, context, simple)?;
            bundled::drain(&stream).await;
            stream.result().await
        }
        Route::GenericComplete => complete(model, context, raw).await?,
        Route::GenericSimpleComplete => complete_simple(model, context, simple).await?,
        Route::RootRaw => root_raw(protocol, common)?.result().await,
        Route::RootSimple => root_simple(protocol, simple)?.result().await,
    };
    let hooks = hooks.lock().unwrap_or_else(PoisonError::into_inner).clone();
    Ok(Observed {
        result,
        requests: bundled::recorded(&requests),
        hooks,
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Hooks a successful call runs under the protocol's response-hook policy.
fn expected_hooks(protocol: &Protocol) -> &'static [&'static str] {
    if bundled::policy(protocol.api).response_hook {
        &["payload", "response 200"]
    } else {
        &["payload"]
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_public_routes_reach_real_adapters() -> TestResult {
    let _registry = bundled::registry();
    maestro_models::reset_api_providers();
    crate::runtime::run(async {
        for protocol in &PROTOCOLS {
            let path = EXPECTED_PATHS
                .iter()
                .find(|(api, _)| *api == protocol.api)
                .ok_or("missing protocol path expectation")?
                .1;
            for route in ROUTES {
                let text = format!("{} via {route:?}", protocol.api);
                let seen = invoke(protocol, route, &text).await?;
                let label = format!("{} {route:?}", protocol.api);
                assert_eq!(bundled::text_of(&seen.result), text, "{label}");
                assert_eq!(bundled::stop_of(&seen.result), StopReason::Stop, "{label}");
                assert_eq!(seen.requests.len(), 1, "{label}");
                let url = seen.requests[0].url.split('?').next().unwrap_or_default();
                assert!(url.ends_with(path), "{label}: {url}");
                assert_eq!(seen.hooks, expected_hooks(protocol), "{label}");
                let usage = seen.result.read().map_err(|_| "poisoned")?.usage.clone();
                assert_eq!(
                    (
                        usage.input,
                        usage.output,
                        usage.total_tokens.present().copied()
                    ),
                    (3.0, 2.0, Some(5.0)),
                    "{label}"
                );
            }
        }
        Ok(())
    })
}

/// An error carrying only `message`.
fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: message.into(),
        stack: None,
        code: None,
    }
}

/// A provider whose callbacks fail immediately with `label`.
fn custom(api: &str, label: &'static str) -> ApiProvider {
    ApiProvider {
        api: api.into(),
        stream: SharedHandle::new(SetupFailure(label)),
        stream_simple: SharedHandle::new(SetupFailure(label)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_initial_registry_is_ready() -> TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output =
                maestro_test_process::TestProcess::current("maestro_initial_registry_is_ready")?
                    .nocapture()
                    .env_clear()
                    .case("fresh")
                    .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    assert_eq!(registered(), bundled::registered_apis());
    Ok(())
}

#[test]
fn maestro_registration_keeps_custom_order() {
    let _registry = bundled::registry();
    reset_api_providers();
    register_api_provider(custom("custom-a", "a"), Some("extension".into()));
    register_api_provider(
        custom("openai-codex-responses", "replaced"),
        Some("extension".into()),
    );
    register_api_provider(custom("custom-b", "b"), None);
    let replaced = get_api_provider("openai-codex-responses");
    maestro_models::register_built_in_api_providers();
    let expected: Vec<String> = bundled::registered_apis()
        .into_iter()
        .map(str::to_owned)
        .chain(["custom-a".to_owned(), "custom-b".to_owned()])
        .collect();
    assert_eq!(registered(), expected);
    unregister_api_providers("extension");
    let after: Vec<String> = bundled::registered_apis()
        .into_iter()
        .map(str::to_owned)
        .chain(["custom-b".to_owned()])
        .collect();
    assert_eq!(registered(), after);
    let restored = stream_simple(
        bundled::protocol_model(&PROTOCOLS[5]).unwrap(),
        bundled::conversation().unwrap(),
        None,
    );
    assert_eq!(
        restored.err().map(|error| error.message).as_deref(),
        Some("No API key for provider: fixture"),
        "the built-in replaced the custom callback"
    );
    assert!(replaced.is_some_and(|handle| {
        handle
            .stream
            .call(
                bundled::protocol_model(&PROTOCOLS[5]).unwrap(),
                bundled::conversation().unwrap(),
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .err()
            .is_some_and(|error| error.message == "replaced")
    }));
    reset_api_providers();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_reset_restores_builtin_adapters() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    for protocol in &PROTOCOLS {
        register_api_provider(custom(protocol.api, "custom"), None);
    }
    register_api_provider(custom("custom-x", "custom"), None);
    let retired = get_api_provider(PROTOCOLS[0].api).ok_or("custom entry")?;
    reset_api_providers();
    assert_eq!(registered(), bundled::registered_apis());
    assert!(get_api_provider("custom-x").is_none());
    let model = bundled::protocol_model(&PROTOCOLS[0])?;
    let old = retired.stream_simple.call(
        model,
        bundled::conversation()?,
        maestro_models::ProviderInvocation::Threaded(None, None),
    );
    assert_eq!(
        old.err().map(|error| error.message),
        Some("custom".to_owned())
    );
    crate::runtime::run(async {
        for protocol in &PROTOCOLS {
            let seen = invoke(protocol, Route::GenericStream, "restored").await?;
            assert_eq!(
                bundled::text_of(&seen.result),
                "restored",
                "{}",
                protocol.api
            );
            let simple = stream_simple(
                bundled::protocol_model(protocol)?,
                bundled::conversation()?,
                None,
            );
            assert_eq!(
                simple.err().map(|error| error.message).as_deref(),
                Some("No API key for provider: fixture"),
                "{}",
                protocol.api
            );
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_clear_does_not_reactivate() -> TestResult {
    let _registry = bundled::registry();
    clear_api_providers();
    crate::runtime::run::<crate::TestResult<_>>(async {
        for protocol in &PROTOCOLS {
            assert!(registered().is_empty());
            let missing = format!("No API provider registered for api: {}", protocol.api);
            let model = bundled::protocol_model(protocol)?;
            let generic = stream(model.clone(), bundled::conversation()?, None);
            assert_eq!(
                generic.err().map(|error| error.message),
                Some(missing.clone())
            );
            let completed = complete_simple(model, bundled::conversation()?, None).await;
            assert_eq!(completed.err().map(|error| error.message), Some(missing));
            for route in [Route::RootRaw, Route::RootSimple] {
                let seen = invoke(protocol, route, "unregistered").await?;
                assert_eq!(bundled::text_of(&seen.result), "unregistered");
                assert!(registered().is_empty());
            }
        }
        Ok(())
    })?;
    reset_api_providers();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_custom_setup_errors_stay_immediate() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    register_api_provider(custom("custom-api", "custom setup"), None);
    let model = bundled::descriptor("custom-api", "https://controlled.invalid")?;
    let message = |result: Result<AssistantMessageEventStream, DiagnosticErrorInfo>| {
        result.err().map(|error| error.message)
    };
    assert_eq!(
        message(stream(model.clone(), bundled::conversation()?, None)).as_deref(),
        Some("custom setup")
    );
    assert_eq!(
        message(stream_simple(model.clone(), bundled::conversation()?, None)).as_deref(),
        Some("custom setup")
    );
    let completed = crate::runtime::run::<crate::TestResult<_>>(async {
        Ok(complete(model, bundled::conversation()?, None)
            .await
            .err()
            .map(|error| error.message))
    })?;
    assert_eq!(completed.as_deref(), Some("custom setup"));
    reset_api_providers();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// A controlled stream that ends without publishing its result.
fn ending_provider() -> ApiProvider {
    ApiProvider {
        api: "ending-api".into(),
        stream: Arc::new(EndingProvider),
        stream_simple: Arc::new(SetupFailure("unused")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_explicit_end_keeps_result_pending() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    register_api_provider(ending_provider(), None);
    let model = bundled::descriptor("ending-api", "https://controlled.invalid")?;
    let started = stream(model.clone(), bundled::conversation()?, None)?;
    let original = crate::runtime::run::<crate::TestResult<_>>(async {
        let events = bundled::drain(&started).await;
        assert!(matches!(
            events.as_slice(),
            [AssistantMessageEvent::Start { .. }]
        ));
        assert!(futures_util::FutureExt::now_or_never(started.result()).is_none());
        let AssistantMessageEvent::Start { partial } = &events[0] else {
            return Err("missing partial".into());
        };
        Ok(Arc::clone(partial))
    })?;
    let output = Arc::new(RetainedOutput(started.clone()));
    register_api_provider(
        ApiProvider {
            api: "ending-api".into(),
            stream: output.clone(),
            stream_simple: output,
        },
        None,
    );
    let mut raw = complete(model.clone(), bundled::conversation()?, None);
    let mut simple = complete_simple(model, bundled::conversation()?, None);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(raw.as_mut().poll(&mut cx).is_pending());
    assert!(simple.as_mut().poll(&mut cx).is_pending());
    register_api_provider(
        ApiProvider {
            api: "ending-api".into(),
            stream: Arc::new(SetupFailure("replacement")),
            stream_simple: Arc::new(SetupFailure("replacement")),
        },
        None,
    );
    started.end(Some(Arc::clone(&original)));
    crate::runtime::run::<crate::TestResult<_>>(async {
        assert!(Arc::ptr_eq(&raw.await?, &original));
        assert!(Arc::ptr_eq(&simple.await?, &original));
        Ok(())
    })?;
    reset_api_providers();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// The dedicated simple entry of `protocol`, which reports setup failure as `Err`.
fn dedicated_simple(
    protocol: &Protocol,
    options: Option<SimpleStreamOptions>,
) -> TestResult<Result<AssistantMessageEventStream, DiagnosticErrorInfo>> {
    use maestro_models::providers::{
        chat::openai_completions as chat,
        messages::anthropic,
        reasoning::mistral,
        responses::{azure_openai_responses as cloud, openai_responses},
    };
    let (model, context) = (bundled::protocol_model(protocol)?, bundled::conversation()?);
    Ok(match protocol.api {
        "anthropic-messages" => anthropic::stream_simple_anthropic(model, context, options),
        "openai-completions" => chat::stream_simple_openai_completions(model, context, options),
        "mistral-conversations" => mistral::stream_simple_mistral(model, context, options),
        "openai-responses" => {
            openai_responses::stream_simple_openai_responses(model, context, options)
        }
        "azure-openai-responses" => cloud::stream_simple_azure_openai_responses(model, context, options),
        "openai-codex-responses" => maestro_models::providers::response_sessions::openai_codex_responses::stream_simple_openai_codex_responses(model, context, options),
        _ => return Err("unknown protocol".into()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Wire facts every protocol must report for a missing key.
fn expected_setup_error(protocol: &Protocol) -> serde_json::Value {
    json!({
        "role": "assistant", "content": [], "api": protocol.api, "provider": "fixture",
        "model": format!("{}-model", protocol.api),
        "usage": {"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"totalTokens":0.0,
            "cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},
        "stopReason": "error", "errorMessage": "No API key for provider: fixture"
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Simple options carrying an optional cancellation signal and API key.
fn signalled(
    signal: Option<&maestro_models::Cancellation>,
    key: Option<String>,
) -> SimpleStreamOptions {
    SimpleStreamOptions {
        common: StreamOptions {
            signal: signal.cloned(),
            api_key: key,
            ..StreamOptions::default()
        },
        ..SimpleStreamOptions::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// One terminal error refers to the same shared result handle.
fn assert_error_identity(
    events: &[AssistantMessageEvent],
    result: &SharedAssistantMessage,
) -> TestResult {
    let [AssistantMessageEvent::Error { error, reason }] = events else {
        return Err("expected one terminal error".into());
    };
    assert!(Arc::ptr_eq(error, result));
    assert!(matches!(
        (reason, bundled::stop_of(result)),
        (maestro_models::ErrorReason::Aborted, StopReason::Aborted)
            | (maestro_models::ErrorReason::Error, StopReason::Error)
    ));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Registry setup failures precede streams; convenience roots retain their settled result.
async fn assert_setup_error(
    protocol: &Protocol,
    signal: Option<maestro_models::Cancellation>,
    key: Option<String>,
) -> TestResult {
    let model = bundled::protocol_model(protocol)?;
    let generic = stream_simple(
        model.clone(),
        bundled::conversation()?,
        Some(signalled(signal.as_ref(), key.clone())),
    );
    assert_eq!(
        generic.err().map(|error| error.message).as_deref(),
        Some("No API key for provider: fixture")
    );
    let completion = complete_simple(
        model,
        bundled::conversation()?,
        Some(signalled(signal.as_ref(), key.clone())),
    )
    .await;
    assert_eq!(
        completion.err().map(|error| error.message).as_deref(),
        Some("No API key for provider: fixture")
    );
    let root = root_simple(protocol, Some(signalled(signal.as_ref(), key)))?;
    let root_events = bundled::drain(&root).await;
    let root_result = root.result().await;
    assert_error_identity(&root_events, &root_result)?;
    let mut seen = serde_json::to_value(&*root_result.read().map_err(|_| "poisoned")?)?;
    assert!(seen["timestamp"].is_number(), "{} timestamp", protocol.api);
    seen.as_object_mut()
        .ok_or("object")?
        .shift_remove("timestamp");
    assert_eq!(seen, expected_setup_error(protocol), "{}", protocol.api);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_builtin_setup_errors_settle() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    let aborted = maestro_models::Cancellation::new();
    aborted.abort();
    crate::runtime::run(async {
        for protocol in &PROTOCOLS {
            let dedicated = dedicated_simple(protocol, None)?;
            assert_eq!(
                dedicated.err().map(|error| error.message).as_deref(),
                Some("No API key for provider: fixture"),
                "{} dedicated",
                protocol.api
            );
            for (key, signal) in [
                (None, None),
                (None, Some(aborted.clone())),
                (Some(String::new()), None),
                (Some(String::new()), Some(aborted.clone())),
            ] {
                let dedicated =
                    dedicated_simple(protocol, Some(signalled(signal.as_ref(), key.clone())))?;
                assert_eq!(
                    dedicated.err().map(|error| error.message).as_deref(),
                    Some("No API key for provider: fixture")
                );
                assert_setup_error(protocol, signal, key).await?;
            }
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// The chat protocol's answer "ab" in separate body reads.
fn chat_chunks() -> Vec<Vec<u8>> {
    let frame = |delta: serde_json::Value, finish: serde_json::Value| {
        let chunk = json!({"id":"r","model":"server","choices":[{"index":0,"finish_reason":finish,"delta":delta}]});
        format!("data: {chunk}\n\n").into_bytes()
    };
    vec![
        frame(json!({"content":"a"}), json!(null)),
        frame(json!({"content":"b"}), json!(null)),
        frame(json!({}), json!("stop")),
        b"data: [DONE]\n\n".to_vec(),
    ]
}

#[cfg(not(target_arch = "wasm32"))]
/// The shared partial handle of a stream's first event.
fn first_handle(events: &[AssistantMessageEvent]) -> TestResult<SharedAssistantMessage> {
    match events.first() {
        Some(AssistantMessageEvent::Start { partial }) => Ok(Arc::clone(partial)),
        _ => Err("the first event is the start".into()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Every delta shares the start handle; returns their concatenation.
fn shared_deltas(events: &[AssistantMessageEvent], handle: &SharedAssistantMessage) -> String {
    let mut deltas = String::new();
    for event in events {
        match event {
            AssistantMessageEvent::TextDelta { partial, delta, .. } => {
                assert!(Arc::ptr_eq(partial, handle));
                deltas.push_str(delta);
            }
            AssistantMessageEvent::Done { message, .. } => {
                assert!(Arc::ptr_eq(message, handle));
            }
            _ => {}
        }
    }
    deltas
}

#[cfg(not(target_arch = "wasm32"))]
/// Payload edits and one shared handle across start, deltas, terminal and results.
async fn assert_shared_results(protocol: &Protocol, root: bool) -> TestResult {
    let chunks = if protocol.api == "openai-completions" {
        chat_chunks()
    } else {
        vec![bundled::text_body(protocol.api, "ab")]
    };
    let (fetch, requests) = bundled::fetch_chunks(chunks);
    let edited: OnPayload = Arc::new(
        |maestro_models::PayloadHookInput {
             mut payload,
             signal: hook_signal,
         },
         _| {
            payload["marker"] = json!({"nested": ["edited", "edited"]});
            Box::pin(async move {
                Ok(maestro_models::PayloadHookInput {
                    payload,
                    signal: hook_signal,
                })
            })
        },
    );
    let common = StreamOptions {
        api_key: Some(bundled::api_key(protocol.api)),
        transport: Some(maestro_models::Transport::Sse),
        fetch: Some(fetch),
        on_payload: Some(edited),
        ..StreamOptions::default()
    };
    let started = if root {
        root_raw(protocol, common)?
    } else {
        stream(
            bundled::protocol_model(protocol)?,
            bundled::conversation()?,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )?
    };
    let independent = started.result();
    let events = bundled::drain(&started).await;
    let handle = first_handle(&events)?;
    assert_eq!(shared_deltas(&events, &handle), "ab");
    assert!(Arc::ptr_eq(&independent.await, &handle));
    assert!(Arc::ptr_eq(&started.result().await, &handle));
    assert_eq!(bundled::text_of(&handle), "ab");
    assert_eq!(
        bundled::recorded(&requests)[0].body["marker"],
        json!({"nested": ["edited", "edited"]})
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Common options whose payload or response hook fails.
fn failing_hook(protocol: &Protocol, fail_payload: bool) -> (StreamOptions, bundled::Requests) {
    let (mut common, requests, _) = common("unused", protocol);
    if fail_payload {
        common.on_payload = Some(Arc::new(|_, _| {
            Box::pin(async { Err(failure("payload failure")) })
        }));
    } else {
        common.on_response = Some(Arc::new(|_, _| {
            Box::pin(async { Err(failure("response failure")) })
        }));
    }
    (common, requests)
}

#[cfg(not(target_arch = "wasm32"))]
/// A failing hook ends `protocol` through its own error path.
async fn assert_hook_failure(protocol: &Protocol, fail_payload: bool) -> TestResult {
    let (common, requests) = failing_hook(protocol, fail_payload);
    let result = root_raw(protocol, common)?.result().await;
    let (text, sent) = if fail_payload {
        ("payload failure", 0)
    } else {
        ("response failure", 1)
    };
    assert_eq!(
        bundled::error_of(&result).as_deref(),
        Some(text),
        "{}",
        protocol.api
    );
    assert_eq!(
        bundled::stop_of(&result),
        StopReason::Error,
        "{}",
        protocol.api
    );
    assert_eq!(
        bundled::recorded(&requests).len(),
        sent,
        "{} {text}",
        protocol.api
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_root_hooks_keep_shared_results() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    crate::runtime::run_paused(async {
        assert_shared_results(&PROTOCOLS[1], false).await?;
        for root in [false, true] {
            assert_shared_results(&PROTOCOLS[5], root).await?;
        }
        assert_session_response_retry().await?;
        for protocol in &PROTOCOLS {
            assert_hook_failure(protocol, true).await?;
            if !matches!(
                protocol.api,
                "mistral-conversations" | "openai-codex-responses"
            ) {
                assert_hook_failure(protocol, false).await?;
            }
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// A cloud body that publishes one text delta, then breaks.
fn broken_body() -> TestResult<maestro_models::HttpBody> {
    let frames = String::from_utf8(bundled::text_body("azure-openai-responses", "partial"))?;
    let delta_end = frames
        .match_indices("\n\n")
        .nth(2)
        .map_or(frames.len(), |(at, _)| at + 2);
    let first = frames.as_bytes()[..delta_end].to_vec();
    let broken = maestro_models::FetchError::Connection(failure("transport reset"));
    Ok(Box::pin(futures_util::stream::iter([
        Ok(first),
        Err(broken),
    ])))
}

#[cfg(not(target_arch = "wasm32"))]
/// A transport that answers once with `body`.
fn single_body_fetch(body: maestro_models::HttpBody) -> maestro_models::Fetch {
    let slot = Arc::new(Mutex::new(Some(body)));
    Arc::new(move |_| {
        let body = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
        Box::pin(std::future::ready(
            body.ok_or(maestro_models::FetchError::Aborted).map(|body| {
                maestro_models::HttpResponse {
                    status: 200,
                    status_text: String::new(),
                    headers: [("content-type".to_owned(), "text/event-stream".to_owned())].into(),
                    body,
                }
            }),
        ))
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_cloud_partial_failure_settles() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    crate::runtime::run(async {
        let common = StreamOptions {
            api_key: Some("controlled-key".into()),
            fetch: Some(single_body_fetch(broken_body()?)),
            ..StreamOptions::default()
        };
        let options = ProviderStreamOptions {
            common,
            ..ProviderStreamOptions::default()
        };
        let model = bundled::protocol_model(&PROTOCOLS[4])?;
        let started = stream(model, bundled::conversation()?, Some(options))?;
        let result = started.result();
        let mut last = None;
        let mut deltas = 0;
        while let Some(event) = started.next().await {
            deltas += usize::from(matches!(event, AssistantMessageEvent::TextDelta { .. }));
            last = Some(event);
        }
        assert_eq!(deltas, 1);
        assert!(matches!(last, Some(AssistantMessageEvent::Error { .. })));
        let settled = result.await;
        assert_eq!(bundled::stop_of(&settled), StopReason::Error);
        assert!(bundled::error_of(&settled).is_some_and(|text| text.contains("transport reset")));
        assert_eq!(bundled::text_of(&settled), "partial");
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Known extras of the wrong shape, by protocol.
fn rejected_extras() -> [(&'static str, serde_json::Value); 12] {
    [
        ("anthropic-messages", json!({"effort": 5})),
        ("anthropic-messages", json!({"effort": {"level": "high"}})),
        ("anthropic-messages", json!({"thinkingEnabled": null})),
        (
            "anthropic-messages",
            json!({"toolChoice": {"type": "tool"}}),
        ),
        ("openai-completions", json!({"reasoningEffort": 5})),
        (
            "openai-completions",
            json!({"toolChoice": {"type": "function", "function": {}}}),
        ),
        ("mistral-conversations", json!({"promptMode": "other"})),
        (
            "mistral-conversations",
            json!({"toolChoice": {"type": "function"}}),
        ),
        ("openai-responses", json!({"reasoningEffort": null})),
        ("openai-responses", json!({"serviceTier": "other"})),
        ("azure-openai-responses", json!({"azureBaseUrl": null})),
        ("azure-openai-responses", json!({"azureDeploymentName": 7})),
    ]
}

#[cfg(not(target_arch = "wasm32"))]
/// Known extras written as maps or arrays where a string or an object is required.
fn rejected_shapes() -> [(&'static str, serde_json::Value); 13] {
    [
        ("openai-responses", json!({"serviceTier": {"auto": null}})),
        ("anthropic-messages", json!({"effort": {"high": null}})),
        (
            "anthropic-messages",
            json!({"thinkingDisplay": {"omitted": null}}),
        ),
        ("anthropic-messages", json!({"toolChoice": {"auto": null}})),
        (
            "anthropic-messages",
            json!({"toolChoice": {"type": {"tool": null}, "name": "lookup"}}),
        ),
        (
            "openai-completions",
            json!({"reasoningEffort": {"high": null}}),
        ),
        ("openai-completions", json!({"toolChoice": {"auto": null}})),
        (
            "openai-completions",
            json!({"toolChoice": {"type": "function", "function": ["lookup"]}}),
        ),
        (
            "openai-completions",
            json!({"toolChoice": {"type": {"function": null}, "function": {"name": "f"}}}),
        ),
        (
            "mistral-conversations",
            json!({"promptMode": {"reasoning": null}}),
        ),
        (
            "mistral-conversations",
            json!({"reasoningEffort": {"high": null}}),
        ),
        (
            "mistral-conversations",
            json!({"toolChoice": {"type": "function", "function": ["lookup"]}}),
        ),
        (
            "openai-responses",
            json!({"reasoningSummary": {"auto": null}}),
        ),
    ]
}

#[cfg(not(target_arch = "wasm32"))]
/// Invalid selected fields, including strings encoded as tagged objects or arrays.
fn rejected_session_extras() -> Vec<(&'static str, serde_json::Value)> {
    let mut cases = Vec::new();
    for (field, literal) in [
        ("reasoningEffort", "high"),
        ("reasoningSummary", "auto"),
        ("serviceTier", "auto"),
        ("textVerbosity", "low"),
    ] {
        for value in [
            json!("other"),
            json!(7),
            json!(false),
            json!([]),
            json!([literal]),
            json!({literal: null}),
        ] {
            cases.push(("openai-codex-responses", json!({field: value})));
        }
    }
    for extra in [
        json!({"reasoningEffort": null}),
        json!({"reasoningEffort": "off"}),
        json!({"textVerbosity": null}),
    ] {
        cases.push(("openai-codex-responses", extra));
    }
    cases
}

#[cfg(not(target_arch = "wasm32"))]
/// Hook log shared with the options that fill it.
type Hooks = Arc<Mutex<Vec<String>>>;
#[cfg(not(target_arch = "wasm32"))]
/// Raw options, the requests they send and the hooks they run.
type ExtraSetup = (ProviderStreamOptions, bundled::Requests, Hooks);

#[cfg(not(target_arch = "wasm32"))]
/// Raw options sending `extra` to `protocol`, with its recorders.
fn extra_options(
    protocol: &Protocol,
    text: &str,
    extra: &serde_json::Value,
) -> TestResult<ExtraSetup> {
    let (common, requests, hooks) = common(text, protocol);
    let extra = serde_json::from_value(extra.clone())?;
    Ok((
        ProviderStreamOptions {
            common,
            extra,
            ..ProviderStreamOptions::default()
        },
        requests,
        hooks,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_invalid_extra_reports_decode_failure() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    crate::runtime::run(async {
        for (index, (api, extra)) in rejected_extras()
            .iter()
            .chain(&rejected_shapes())
            .chain(&rejected_session_extras())
            .enumerate()
        {
            let protocol = PROTOCOLS
                .iter()
                .find(|protocol| protocol.api == *api)
                .ok_or("protocol")?;
            let (options, requests, hooks) = extra_options(protocol, "unused", extra)?;
            let model = bundled::protocol_model(protocol)?;
            let started = stream(model, bundled::conversation()?, Some(options))?;
            let events = bundled::drain(&started).await;
            let result = started.result().await;
            assert!(
                matches!(events.as_slice(), [AssistantMessageEvent::Error { error, .. }] if Arc::ptr_eq(error, &result)),
                "case {index} terminal identity"
            );
            assert_eq!(bundled::stop_of(&result), StopReason::Error, "case {index}");
            let decode_failure = bundled::error_of(&result)
                .is_some_and(|text| !text.is_empty() && !text.starts_with("No API"));
            assert!(decode_failure, "case {index}");
            assert!(
                bundled::recorded(&requests).is_empty(),
                "case {index} reached the transport"
            );
            assert!(
                hooks.lock().map_err(|_| "poisoned")?.is_empty(),
                "case {index} reached a hook"
            );
        }
        for protocol in [&PROTOCOLS[0], &PROTOCOLS[5]] {
            let extra = json!({"effort": "high", "unknown": {"deep": [1, 1, {"x": [true]}]}});
            let (options, requests, _) = extra_options(protocol, "fine", &extra)?;
            let model = bundled::protocol_model(protocol)?;
            let result = stream(model, bundled::conversation()?, Some(options))?
                .result()
                .await;
            assert_eq!(bundled::text_of(&result), "fine");
            assert_eq!(bundled::recorded(&requests).len(), 1);
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Counts drops of the last handle to a stored object.
struct Released(Arc<std::sync::atomic::AtomicUsize>);

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Released {
    fn drop(&mut self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// The `u32` and `String` objects each call received.
type ObjectsSeen = Arc<Mutex<Vec<(Option<u32>, Option<String>)>>>;

#[cfg(not(target_arch = "wasm32"))]
/// A provider recording the `u32` and `String` objects each raw call received.
fn recording_provider(seen: &ObjectsSeen) -> ApiProvider {
    let recorder = Arc::clone(seen);
    ApiProvider {
        api: "object-api".into(),
        stream: Arc::new(ObjectRecorder(recorder)),
        stream_simple: Arc::new(SetupFailure("unused")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_provider_objects_keep_typed_aliases() -> TestResult {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<maestro_models::ProviderObjects>();
    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut objects = maestro_models::ProviderObjects::default();
    objects.insert(7_u32);
    objects.insert(String::from("private-value"));
    objects.insert(Released(Arc::clone(&drops)));
    let mut replaced = objects.clone();
    replaced.insert(8_u32);
    assert_eq!(
        (objects.get::<u32>(), replaced.get::<u32>()),
        (Some(&7), Some(&8))
    );
    assert_eq!(
        replaced.get::<String>().map(String::as_str),
        Some("private-value")
    );
    assert_eq!(objects.get::<u8>(), None);
    let debug = format!("{objects:?}");
    assert!(
        debug.contains('3') && !debug.contains("private-value"),
        "{debug}"
    );

    let _registry = bundled::registry();
    reset_api_providers();
    let seen = Arc::default();
    register_api_provider(recording_provider(&seen), None);
    let model = bundled::descriptor("object-api", "https://controlled.invalid")?;
    let options = |objects: &maestro_models::ProviderObjects| {
        Some(ProviderStreamOptions {
            objects: objects.clone(),
            ..ProviderStreamOptions::default()
        })
    };
    assert!(stream(model.clone(), bundled::conversation()?, options(&objects)).is_err());
    let completed = crate::runtime::run::<crate::TestResult<_>>(async {
        Ok(
            complete(model, bundled::conversation()?, options(&replaced))
                .await
                .is_err(),
        )
    })?;
    assert!(completed);
    let both = |number| (Some(number), Some("private-value".to_owned()));
    assert_eq!(*seen.lock().map_err(|_| "poisoned")?, [both(7), both(8)]);
    drop(objects);
    assert_eq!(
        drops.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a clone still holds the object"
    );
    drop(replaced);
    assert_eq!(
        drops.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the last alias released it"
    );
    reset_api_providers();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Payloads and retry counts an injected client received.
type ClientCalls = Arc<Mutex<Vec<(serde_json::Value, Option<f64>)>>>;

#[cfg(not(target_arch = "wasm32"))]
/// An injected message client answering with `text` and recording what it was sent.
fn injected_client(sent: &ClientCalls) -> maestro_models::AnthropicClient {
    let recorder = Arc::clone(sent);
    Arc::new(move |payload, options| {
        recorder
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((payload, options.max_retries));
        let body = bundled::text_body("anthropic-messages", "injected");
        let chunk: maestro_models::HttpBody = Box::pin(futures_util::stream::iter([Ok(body)]));
        Box::pin(std::future::ready(Ok(maestro_models::HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: [("content-type".to_owned(), "text/event-stream".to_owned())].into(),
            body: chunk,
        })))
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_injected_client_bypasses_default_setup() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    let sent = Arc::default();
    let client = injected_client(&sent);
    let (fetch, requests) = bundled::fetch(Vec::new());
    let common = StreamOptions {
        api_key: Some(format!("{}{}", "sk-ant-oat", "01-controlled")),
        fetch: Some(fetch),
        max_retries: Some(3.0),
        ..StreamOptions::default()
    };
    let mut model = bundled::protocol_model(&PROTOCOLS[0])?;
    model.base_url = "not a url".into();
    crate::runtime::run::<crate::TestResult<_>>(async {
        let mut objects = maestro_models::ProviderObjects::default();
        objects.insert(Arc::clone(&client));
        let raw = ProviderStreamOptions {
            common: common.clone(),
            objects,
            ..ProviderStreamOptions::default()
        };
        let generic = stream(model.clone(), bundled::conversation()?, Some(raw))?;
        assert_eq!(bundled::text_of(&generic.result().await), "injected");
        let mut objects = maestro_models::ProviderObjects::default();
        objects.insert(Arc::clone(&client));
        let completed = maestro_models::complete(
            model.clone(),
            bundled::conversation()?,
            Some(ProviderStreamOptions {
                common: common.clone(),
                objects,
                ..ProviderStreamOptions::default()
            }),
        )
        .await?;
        assert_eq!(bundled::text_of(&completed), "injected");
        let typed = AnthropicOptions {
            common,
            client: Some(client),
            ..AnthropicOptions::default()
        };
        let typed = maestro_models::stream_anthropic(model, bundled::conversation()?, Some(typed));
        assert_eq!(bundled::text_of(&typed.result().await), "injected");
        Ok(())
    })?;
    let sent = sent.lock().map_err(|_| "poisoned")?;
    assert_eq!(sent.len(), 3);
    for (payload, retries) in sent.iter() {
        assert!(
            !payload.to_string().contains("Claude Code"),
            "the key shape selected the subscription payload"
        );
        assert_eq!(*retries, Some(3.0));
    }
    assert!(
        bundled::recorded(&requests).is_empty(),
        "the default transport was used"
    );
    reset_api_providers();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_first_access_is_safe_across_threads() -> TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "maestro_first_access_is_safe_across_threads",
            )?
            .nocapture()
            .env_clear()
            .case("threads")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    let readers = 8;
    let start = Arc::new(std::sync::Barrier::new(readers + 1));
    let handles: Vec<_> = (0..readers)
        .map(|_| {
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                let apis = registered();
                apis.len() >= 6
                    && bundled::apis()
                        .iter()
                        .zip(&apis)
                        .all(|(want, got)| want == got)
            })
        })
        .collect();
    start.wait();
    register_api_provider(custom("custom-api", "custom"), None);
    for handle in handles {
        assert!(
            handle.join().map_err(|_| "reader panicked")?,
            "a reader missed the built-in prefix"
        );
    }
    assert_eq!(registered().last().map(String::as_str), Some("custom-api"));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// A response hook fails once, then the existing retry completes the same invocation.
async fn assert_session_response_retry() -> TestResult {
    let (mut common, requests, _) = common("retried", &PROTOCOLS[5]);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    common.on_response = Some(Arc::new(move |_, _| {
        let first = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0;
        Box::pin(async move {
            if first {
                Err(failure("response failure"))
            } else {
                Ok(())
            }
        })
    }));
    let result = root_raw(&PROTOCOLS[5], common)?.result().await;
    assert_eq!(bundled::text_of(&result), "retried");
    assert_eq!(bundled::stop_of(&result), StopReason::Stop);
    assert_eq!(bundled::recorded(&requests).len(), 2);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let usage = &result.read().map_err(|_| "poisoned")?.usage;
    assert_eq!(
        (
            usage.input,
            usage.output,
            usage.total_tokens.present().copied()
        ),
        (3.0, 2.0, Some(5.0))
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_response_session_cache_affinity_completes() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    crate::runtime::run(async {
        for session in [None, Some(""), Some("affinity")] {
            let (mut common, requests, _) = common("cache affinity e2e success", &PROTOCOLS[5]);
            common.session_id = session.map(str::to_owned);
            let result = complete(
                bundled::protocol_model(&PROTOCOLS[5])?,
                bundled::conversation()?,
                Some(ProviderStreamOptions {
                    common,
                    ..Default::default()
                }),
            )
            .await?;
            assert_eq!(bundled::text_of(&result), "cache affinity e2e success");
            assert_eq!(bundled::error_of(&result), None);
            assert_eq!(bundled::stop_of(&result), StopReason::Stop);
            let requests = bundled::recorded(&requests);
            assert_eq!(requests.len(), 1);
            let request = &requests[0];
            assert_eq!(
                request.body.get("prompt_cache_key"),
                session.map(|value| json!(value)).as_ref()
            );
            for name in ["session_id", "x-client-request-id"] {
                let header = request
                    .headers
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.as_str());
                assert_eq!(header, session.filter(|value| !value.is_empty()));
            }
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_response_session_raw_key_errors_settle() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    crate::runtime::run(async {
        for (key, aborted) in [
            (None, false),
            (None, true),
            (Some(String::new()), false),
            (Some(String::new()), true),
        ] {
            for route in [Route::RootRaw, Route::GenericStream, Route::GenericComplete] {
                assert_raw_key_error(key.clone(), aborted, route).await?;
            }
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Raw key failure preserves cancellation, terminal identity and the absence of effects.
async fn assert_raw_key_error(key: Option<String>, aborted: bool, route: Route) -> TestResult {
    let (mut common, requests, hooks) = common("unused", &PROTOCOLS[5]);
    common.api_key = key;
    let signal = maestro_models::Cancellation::new();
    if aborted {
        signal.abort();
    }
    common.signal = Some(signal);
    let model = bundled::protocol_model(&PROTOCOLS[5])?;
    let options = Some(ProviderStreamOptions {
        common: common.clone(),
        ..Default::default()
    });
    let result = if matches!(route, Route::GenericComplete) {
        complete(model, bundled::conversation()?, options).await?
    } else {
        let started = if matches!(route, Route::RootRaw) {
            root_raw(&PROTOCOLS[5], common)?
        } else {
            stream(model, bundled::conversation()?, options)?
        };
        let events = bundled::drain(&started).await;
        let result = started.result().await;
        assert_error_identity(&events, &result)?;
        result
    };
    let mut seen = serde_json::to_value(&*result.read().map_err(|_| "poisoned")?)?;
    assert!(seen["timestamp"].is_number());
    seen.as_object_mut()
        .ok_or("object")?
        .shift_remove("timestamp");
    let mut expected = expected_setup_error(&PROTOCOLS[5]);
    if aborted {
        expected["stopReason"] = json!("aborted");
    }
    assert_eq!(seen, expected);
    assert!(bundled::recorded(&requests).is_empty());
    assert!(hooks.lock().map_err(|_| "poisoned")?.is_empty());
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// A dedicated simple entry fails synchronously with the caller's own diagnostic fields.
fn assert_missing_key(row: &key_selection::Row) -> TestResult {
    let protocol = PROTOCOLS
        .iter()
        .find(|protocol| protocol.api == row.input.api)
        .ok_or("protocol")?;
    let (fetch, requests) = bundled::fetch(bundled::text_body(protocol.api, "ok"));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = Arc::clone(&calls);
    let on_payload: OnPayload = Arc::new(move |payload, _| {
        hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(payload)))
    });
    let mut options = signalled(None, row.input.options.api_key.clone());
    options.common.fetch = Some(fetch);
    options.common.on_payload = Some(on_payload);
    let Err(error) = dedicated_simple(protocol, Some(options))? else {
        return Err("a keyless setup produced a stream".into());
    };
    assert_eq!(Some(&error.message), row.expected.error.as_ref());
    let name = (protocol.api != "mistral-conversations").then_some("Error");
    assert_eq!(error.name.as_deref(), name, "{}", protocol.api);
    assert!(error.code.is_none() && error.stack.is_none());
    assert!(bundled::recorded(&requests).is_empty());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_missing_key_keeps_each_callers_diagnostic() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    key_selection::assert_rows(
        &[
            "simple_key_selection_stays_shared_across_existing_providers",
            "simple_missing_key_keeps_each_callers_diagnostic",
        ],
        "simple_missing_key_keeps_each_callers_diagnostic",
        assert_missing_key,
    )
}

/// Retained invocation owner for the controlled provider.
struct SetupFailure(&'static str);
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for SetupFailure
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        Err(failure(self.0))
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Retained invocation owner for the controlled provider.
struct EndingProvider;
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for EndingProvider
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        let model = call.model;

        let stream = AssistantMessageEventStream::new();
        let partial = Arc::new(std::sync::RwLock::new(
            serde_json::from_value(json!({
                "role": "assistant", "content": [], "api": model.api, "provider": model.provider,
                "model": model.id, "stopReason": "stop", "timestamp": 1,
                "usage": {"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                    "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
            }))
            .map_err(|error| failure(&error.to_string()))?,
        ));
        stream.push(AssistantMessageEvent::Start { partial });
        stream.end(None);
        Ok(stream)
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Retained invocation owner for the controlled provider.
struct ObjectRecorder(ObjectsSeen);
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::ProviderStreamOptions<E>>,
    > for ObjectRecorder
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::ProviderStreamOptions<E>>,
    ) -> Self::Return {
        let options = call.options;

        let objects = options.map(|options| options.objects).unwrap_or_default();
        let entry = (
            objects.get::<u32>().copied(),
            objects.get::<String>().cloned(),
        );
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(entry);
        Err(failure("recorded"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Keep an explicitly ended stream's original result observation across replacement.
struct RetainedOutput(AssistantMessageEventStream);
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for RetainedOutput
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, _: maestro_models::ProviderCall<E, O>) -> Self::Return {
        Ok(self.0.clone())
    }
}
