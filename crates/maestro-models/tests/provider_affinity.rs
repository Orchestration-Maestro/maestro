//! Provider invocation ownership through the public SDK.

#[cfg(not(target_arch = "wasm32"))]
use maestro_models::Model;
use maestro_models::{
    Local, OnPayload, OnResponse, SimpleStreamOptions, StreamOptions, build_base_options_for,
};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use std::{cell::Cell, rc::Rc};

mod bundled_cases;
#[cfg(not(target_arch = "wasm32"))]
mod controls;
mod corpus;
mod dispatch_corpus;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/original_options.rs"]
mod original_options;
mod registered;
mod registry_corpus;
mod response_policies;
mod scripted;
#[cfg(not(target_arch = "wasm32"))]
mod session_transport;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn raw_simple_and_catalog_keep_original_option_families() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    let owner = Arc::new(original_options::OptionsProvider::default());
    maestro_models::register_api_provider(
        maestro_models::ApiProvider {
            api: "controlled".into(),
            stream: owner.clone(),
            stream_simple: owner.clone(),
        },
        Some("original-options".into()),
    );
    assert!(owner.observed.lock().unwrap().is_empty());
    let provider = maestro_models::get_api_provider("controlled").unwrap();
    let model = crate::bundled::descriptor("controlled", "https://fixture.invalid")?;
    crate::runtime::run::<crate::TestResult<_>>(tokio::task::LocalSet::new().run_until(async {
        original_options::exercise(&provider, &model, &owner).await;
        Ok(())
    }))?;
    maestro_models::unregister_api_providers("original-options");
    Ok(())
}

#[test]
fn base_options_share_hooks_without_calling_them() {
    let model = crate::chat::model(&serde_json::json!({"maxTokens":40000})).unwrap();
    let context = crate::chat::context(&serde_json::json!({"messages": []})).unwrap();
    let calls = Rc::new(Cell::new(0));
    let payload_calls = Rc::clone(&calls);
    let payload: OnPayload<Local> = Rc::new(move |input, _| {
        payload_calls.set(payload_calls.get() + 1);
        Box::pin(async move { Ok(input) })
    });
    let response_calls = Rc::clone(&calls);
    let response: OnResponse<Local> = Rc::new(move |_, _| {
        response_calls.set(response_calls.get() + 1);
        Box::pin(async { Ok(()) })
    });
    for row in corpus::hooks()
        .rows
        .into_iter()
        .filter(|row| row.input.get("payload").is_some())
    {
        let input: corpus::HookInput = serde_json::from_value(row.input).unwrap();
        let (with_payload, with_response) = (input.payload, input.response);
        let options = SimpleStreamOptions::<Local> {
            common: StreamOptions {
                on_payload: with_payload.then(|| Rc::clone(&payload)),
                on_response: with_response.then(|| Rc::clone(&response)),
                api_key: Some("original".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let prepared = build_base_options_for(&model, &context, Some(&options), Some("selected"));
        assert_eq!(prepared.on_payload.is_some(), with_payload);
        assert_eq!(prepared.on_response.is_some(), with_response);
        assert_eq!(
            serde_json::json!({
                "payloadIdentity": prepared.on_payload.as_ref().zip(options.common.on_payload.as_ref())
                    .is_none_or(|(actual, original)| Rc::ptr_eq(actual, original)),
                "responseIdentity": prepared.on_response.as_ref().zip(options.common.on_response.as_ref())
                    .is_none_or(|(actual, original)| Rc::ptr_eq(actual, original)),
            }),
            row.expected,
            "{}",
            row.name
        );
        if let Some(actual) = prepared.on_payload {
            assert!(Rc::ptr_eq(&actual, &payload));
        }
        if let Some(actual) = prepared.on_response {
            assert!(Rc::ptr_eq(&actual, &response));
        }
        assert_eq!(prepared.api_key.as_deref(), Some("selected"));
        assert_eq!(prepared.max_tokens, Some(40000.0));
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(
        maestro_models::build_base_options(&model, &context, None, None).max_tokens,
        Some(40000.0)
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn default_sdk_futures_remain_thread_safe() {
    use maestro_models::{
        ApiProvider, Context, FauxResponseFactory, ProviderStreamOptions, SimpleStreamOptions,
        StreamOptionsInput,
    };
    use std::sync::Arc;
    fn shared<T: Send + Sync>() {}
    fn send<T: Send>(_: T) {}
    // A contextual default literal still coerces hook closures without a family annotation.
    fn default_options(options: StreamOptions) -> StreamOptions {
        options
    }
    shared::<StreamOptions>();
    shared::<ProviderStreamOptions>();
    shared::<SimpleStreamOptions>();
    shared::<StreamOptionsInput>();
    shared::<ApiProvider>();
    shared::<FauxResponseFactory>();
    let common = default_options(StreamOptions {
        on_payload: Some(Arc::new(|input, _| Box::pin(async move { Ok(input) }))),
        on_response: Some(Arc::new(|_, _| Box::pin(async { Ok(()) }))),
        ..Default::default()
    });
    assert!(common.on_payload.is_some());
    assert!(common.on_response.is_some());
    let inferred_with_expected_type: StreamOptions = StreamOptions {
        on_payload: Some(Arc::new(|input, _| Box::pin(async move { Ok(input) }))),
        ..Default::default()
    };
    assert!(inferred_with_expected_type.on_payload.is_some());
    send(async move {
        std::future::ready(()).await;
        let model: Model = serde_json::from_value(serde_json::json!({
            "id": "controlled", "name": "Controlled", "api": "controlled",
            "provider": "controlled", "baseUrl": "https://fixture.invalid",
            "reasoning": false, "input": ["text"],
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
            "contextWindow": 100, "maxTokens": 10
        }))
        .unwrap();
        let context = Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        };
        let _ = maestro_models::stream(model.clone(), context.clone(), None);
        let _ = maestro_models::stream_simple(model.clone(), context.clone(), None);
        let _ = maestro_models::complete(model.clone(), context.clone(), None).await;
        let _ = maestro_models::complete_simple(model, context, None).await;
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn direct_local_submission_preserves_hook_phases() -> crate::TestResult {
    let _registry = crate::bundled::registry();
    crate::runtime::run(tokio::task::LocalSet::new().run_until(async {
        for protocol in [
            "openai-completions",
            "anthropic-messages",
            "openai-responses",
            "azure-openai-responses",
            "mistral-conversations",
            "google-generative-ai",
            "openai-codex-responses",
        ] {
            for simple in [false, true] {
                direct_local_case(protocol, simple).await?;
            }
        }
        Ok(())
    }))
}

#[cfg(not(target_arch = "wasm32"))]
/// Keep authentication and transport selection on the real direct entry route.
fn local_direct_options(
    protocol: &str,
    phases: Rc<std::cell::RefCell<Vec<&'static str>>>,
    fetch: maestro_models::Fetch,
) -> StreamOptions<Local> {
    StreamOptions {
        api_key: Some(crate::bundled::api_key(protocol)),
        transport: Some(maestro_models::Transport::Sse),
        ..local_common_options(phases, fetch)
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Protocol-shaped empty success bodies remain independent of the renderer.
fn direct_response(protocol: &str) -> crate::transport::Attempt {
    use crate::transport;
    match protocol {
        "anthropic-messages" => {
            transport::Attempt::body(200, &[], crate::messages::STOP.as_bytes().to_vec())
        }
        "openai-responses" | "azure-openai-responses" | "openai-codex-responses" => transport::Attempt::body(
            200,
            &[],
            crate::response_endpoint::COMPLETE.as_bytes().to_vec(),
        ),
        "google-generative-ai" | "google-vertex" => transport::Attempt::body(200, &[], b"data: {\"candidates\":[{\"content\":{\"parts\":[]},\"finishReason\":\"STOP\"}]}\n\n".to_vec()),
        "mistral-conversations" => transport::Attempt::body(200, &[("content-type", "text/event-stream")], b"data: {\"id\":\"controlled\",\"model\":\"controlled\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n".to_vec()),
        _ => transport::Attempt::success(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Drive each invocation to completion before checking its closed phase transcript.
async fn direct_local_case(protocol: &str, simple: bool) -> crate::TestResult {
    use crate::{chat, transport};
    use serde_json::json;
    use std::cell::RefCell;
    let policy = crate::bundled::policy(protocol);
    let transport = transport::transport(vec![direct_response(protocol)]);
    let phases = Rc::new(RefCell::new(Vec::new()));
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let spawn = local_submission(Rc::clone(&tasks), Rc::clone(&admissions));
    let model = chat::model(&json!({"api": protocol}))?;
    let context = chat::context(
        &json!({"messages": [{"role":"user", "content":"controlled", "timestamp":0}]}),
    )?;
    let options = Some(local_direct_options(
        protocol,
        Rc::clone(&phases),
        transport.fetch.clone(),
    ));
    let stream = if simple {
        direct_simple_stream::<Local>(protocol, model, context, options, spawn)
    } else {
        direct_stream::<Local>(protocol, model, context, options, spawn)
    };
    assert_eq!(admissions.get(), 1);
    assert!(phases.borrow().is_empty());
    let (events, result) = transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    assert_eq!(result.error_message, None);
    assert_eq!(events, ["start", "done"]);
    assert_eq!(result.stop_reason, maestro_models::StopReason::Stop);
    assert_eq!(*phases.borrow(), hook_phases(policy));
    let (_, body) = transport.first_request()?;
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    assert_sent_temperature(protocol, &body);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// The closed hook transcript one protocol's response-hook policy produces.
fn hook_phases(policy: &crate::bundled::ProtocolPolicy) -> &'static [&'static str] {
    if policy.response_hook {
        &[
            "payload-before",
            "payload-after",
            "response-before",
            "response-after",
        ]
    } else {
        &["payload-before", "payload-after"]
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Inspect the protocol's actual post-hook wire field.
fn assert_sent_temperature(protocol: &str, body: &serde_json::Value) {
    use serde_json::json;
    assert_eq!(
        if matches!(protocol, "google-generative-ai" | "google-vertex") {
            &body["generationConfig"]["temperature"]
        } else {
            &body["temperature"]
        },
        &json!(0.25)
    );
}

#[cfg(not(target_arch = "wasm32"))]
/// Record and retain every admitted local task until its producer completes.
fn local_submission(
    tasks: Rc<std::cell::RefCell<Vec<tokio::task::JoinHandle<()>>>>,
    admissions: Rc<Cell<usize>>,
) -> maestro_models::ProviderSpawner<Local> {
    Rc::new(move |task| {
        admissions.set(admissions.get() + 1);
        let task = tokio::task::spawn_local(task);
        tasks.borrow_mut().push(task);
        true
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Use the public raw direct entry for each protocol.
fn direct_stream<E: maestro_models::ProviderExecution>(
    protocol: &str,
    model: Model,
    context: maestro_models::Context,
    options: Option<StreamOptions<E>>,
    spawn: maestro_models::ProviderSpawner<E>,
) -> maestro_models::AssistantMessageEventStream {
    use maestro_models::*;
    if matches!(
        protocol,
        "openai-responses" | "azure-openai-responses" | "openai-codex-responses"
    ) {
        return direct_response_stream(protocol, model, context, options, spawn);
    }
    let common = options.unwrap_or_default();
    match protocol {
        "google-vertex" => stream_google_vertex_for::<E>(
            model,
            context,
            Some(GoogleVertexOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        "google-generative-ai" => stream_google_for::<E>(
            model,
            context,
            Some(GoogleOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        "mistral-conversations" => stream_mistral_for::<E>(
            model,
            context,
            Some(MistralOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        "anthropic-messages" => stream_anthropic_for::<E>(
            model,
            context,
            Some(AnthropicOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        _ => stream_openai_completions_for::<E>(
            model,
            context,
            Some(OpenAICompletionsOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Use the direct response-family exports with the same local settings.
fn direct_response_stream<E: maestro_models::ProviderExecution>(
    protocol: &str,
    model: Model,
    context: maestro_models::Context,
    options: Option<StreamOptions<E>>,
    spawn: maestro_models::ProviderSpawner<E>,
) -> maestro_models::AssistantMessageEventStream {
    use maestro_models::*;
    let common = options.unwrap_or_default();
    match protocol {
        "openai-responses" => stream_openai_responses_for::<E>(
            model,
            context,
            Some(OpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        "openai-codex-responses" => stream_openai_codex_responses_for::<E>(
            model,
            context,
            Some(OpenAICodexResponsesOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        "azure-openai-responses" => stream_azure_openai_responses_for::<E>(
            model,
            context,
            Some(AzureOpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
            spawn,
        ),
        _ => unreachable!("response protocol selected above"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Root simple entries preserve their settled-stream boundary in either family.
fn direct_simple_stream<E: maestro_models::ProviderExecution>(
    protocol: &str,
    model: Model,
    context: maestro_models::Context,
    options: Option<StreamOptions<E>>,
    spawn: maestro_models::ProviderSpawner<E>,
) -> maestro_models::AssistantMessageEventStream {
    use maestro_models::*;
    let options = options.map(|common| SimpleStreamOptions {
        common,
        ..Default::default()
    });
    match protocol {
        "google-vertex" => stream_simple_google_vertex_for::<E>(model, context, options, spawn),
        "google-generative-ai" => stream_simple_google_for::<E>(model, context, options, spawn),
        "openai-codex-responses" => {
            stream_simple_openai_codex_responses_for::<E>(model, context, options, spawn)
        }
        "mistral-conversations" => stream_simple_mistral_for::<E>(model, context, options, spawn),
        "anthropic-messages" => stream_simple_anthropic_for::<E>(model, context, options, spawn),
        "openai-responses" => {
            stream_simple_openai_responses_for::<E>(model, context, options, spawn)
        }
        "azure-openai-responses" => {
            stream_simple_azure_openai_responses_for::<E>(model, context, options, spawn)
        }
        _ => stream_simple_openai_completions_for::<E>(model, context, options, spawn),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Caller-local hook observations survive suspension without requiring thread safety.
type LocalPhases = Rc<std::cell::RefCell<Vec<&'static str>>>;
#[cfg(not(target_arch = "wasm32"))]
/// Threaded observations retain the same phase order through the existing producer.
type ThreadedPhases = std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>;
#[cfg(not(target_arch = "wasm32"))]
/// Record a local hook observation without retaining a borrow across user work.
#[cfg(test)]
fn local_phase(phases: &LocalPhases, phase: &'static str) {
    phases.borrow_mut().push(phase);
}
#[cfg(not(target_arch = "wasm32"))]
/// Record a threaded hook observation without retaining a guard across user work.
#[cfg(test)]
fn threaded_phase(phases: &ThreadedPhases, phase: &'static str) {
    phases.lock().unwrap().push(phase);
}
#[cfg(not(target_arch = "wasm32"))]
/// One hook body, with only concrete pointer/log erasures differing by family.
macro_rules! common_options {
    ($name:ident, $mode:ty, $pointer:ident, $log:ty, $push:ident) => {
        /// Payload selection and response observation both cross real suspension.
        #[cfg(test)]
        fn $name(phases: $log, fetch: maestro_models::Fetch) -> StreamOptions<$mode> {
            let payload_phases = phases.clone();
            StreamOptions {
                api_key: Some("fixture-key".into()),
                fetch: Some(fetch),
                on_payload: Some($pointer::new(move |mut input, _| {
                    let phases = payload_phases.clone();
                    Box::pin(async move {
                        $push(&phases, "payload-before");
                        tokio::task::yield_now().await;
                        $push(&phases, "payload-after");
                        if let Some(config) = input.payload.get_mut("inferenceConfig") {
                            config["temperature"] = serde_json::json!(0.25);
                        } else if let Some(config) = input.payload.get_mut("config") {
                            config["temperature"] = serde_json::json!(0.25);
                        } else {
                            input.payload["temperature"] = serde_json::json!(0.25);
                        }
                        Ok(input)
                    })
                })),
                on_response: Some($pointer::new(move |response, _| {
                    let phases = phases.clone();
                    Box::pin(async move {
                        assert_eq!(response.status.to_bits(), 200.0_f64.to_bits());
                        $push(&phases, "response-before");
                        tokio::task::yield_now().await;
                        $push(&phases, "response-after");
                        Ok(())
                    })
                })),
                ..Default::default()
            }
        }
    };
}
#[cfg(not(target_arch = "wasm32"))]
common_options!(local_common_options, Local, Rc, LocalPhases, local_phase);
#[cfg(not(target_arch = "wasm32"))]
common_options!(
    threaded_common_options,
    maestro_models::Threaded,
    Arc,
    ThreadedPhases,
    threaded_phase
);

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn chat_local_simple_entries_keep_setup_and_admission_errors() -> crate::TestResult {
    use crate::{chat, transport};
    use maestro_models::{ProviderSpawner, stream_simple_openai_completions_for};
    use serde_json::json;
    let transport = transport::transport(vec![transport::Attempt::success()]);
    let phases = Rc::new(std::cell::RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let counted = Rc::clone(&admissions);
    let spawn: ProviderSpawner<Local> = Rc::new(move |_| {
        counted.set(counted.get() + 1);
        false
    });
    let model = chat::model(&json!({}))?;
    let context = chat::context(&json!({"messages": []}))?;
    let missing =
        maestro_models::providers::chat::openai_completions::stream_simple_openai_completions_for::<
            Local,
        >(model.clone(), context.clone(), None, Rc::clone(&spawn));
    let Err(error) = missing else {
        panic!("missing key must fail before a stream exists")
    };
    assert_eq!(error.message, "No API key for provider: fixture");
    assert_eq!(admissions.get(), 0);
    let options = SimpleStreamOptions {
        common: local_common_options(Rc::clone(&phases), transport.fetch.clone()),
        ..Default::default()
    };
    let stream =
        stream_simple_openai_completions_for::<Local>(model, context, Some(options), spawn);
    assert_eq!(admissions.get(), 1);
    let (events, result) = crate::runtime::run(transport::drain(&stream))?;
    assert_eq!(events, ["error"]);
    assert_eq!(
        result.error_message.as_deref(),
        Some("Streaming requires a running Tokio runtime.")
    );
    assert!(phases.borrow().is_empty());
    assert_eq!(transport.attempts(), 0);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn direct_cloud_local_submission_preserves_hook_phases() -> crate::TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "direct_cloud_local_submission_preserves_hook_phases",
            )?
            .nocapture()
            .env_clear()
            .envs(
                [
                    ("AWS_BEARER_TOKEN_BEDROCK", "fixture-token"),
                    ("AWS_REGION", "us-east-1"),
                ]
                .iter()
                .copied(),
            )
            .case("local-cloud")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    crate::runtime::run(tokio::task::LocalSet::new().run_until(async {
        for simple in [false, true] {
            direct_cloud_local_case(simple).await?;
        }
        Ok(())
    }))
}

/// Exercise the native SDK and response-admission path with genuinely local hooks.
#[cfg(not(target_arch = "wasm32"))]
async fn direct_cloud_local_case(simple: bool) -> crate::TestResult {
    use crate::{bedrock, transport};
    use serde_json::json;
    use std::cell::RefCell;
    let policy = crate::bundled::policy("bedrock-converse-stream");
    let transport = bedrock::serve(vec![
        bedrock::event("messageStart", &json!({"role":"assistant"}))?,
        bedrock::event("messageStop", &json!({"stopReason":"end_turn"}))?,
    ]);
    let phases = Rc::new(RefCell::new(Vec::new()));
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let spawn = local_submission(Rc::clone(&tasks), Rc::clone(&admissions));
    let model = bedrock::model("https://bedrock-runtime.us-east-1.amazonaws.com")?;
    let context = bedrock::context("controlled")?;
    let common = local_common_options(Rc::clone(&phases), transport.fetch.clone());
    let stream = direct_cloud_stream::<Local>(simple, model, context, common, spawn);
    assert_eq!(admissions.get(), 1);
    assert!(phases.borrow().is_empty());
    let (events, result) = transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    assert_eq!(result.error_message, None);
    assert_eq!(events, ["start", "done"]);
    assert_eq!(*phases.borrow(), hook_phases(policy));
    let (_, body) = transport.first_request()?;
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(body["inferenceConfig"]["temperature"], json!(0.25));
    Ok(())
}

/// Use the existing raw and simple cloud entry points with the same supplied admission.
#[cfg(not(target_arch = "wasm32"))]
fn direct_cloud_stream<E: maestro_models::ProviderExecution>(
    simple: bool,
    model: Model,
    context: maestro_models::Context,
    common: StreamOptions<E>,
    spawn: maestro_models::ProviderSpawner<E>,
) -> maestro_models::AssistantMessageEventStream {
    use maestro_models::providers::cloud::amazon_bedrock::{
        stream_bedrock_for, stream_simple_bedrock_for,
    };
    use maestro_models::{BedrockOptions, SimpleStreamOptions};
    if simple {
        stream_simple_bedrock_for::<E>(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )
    } else {
        stream_bedrock_for::<E>(
            model,
            context,
            Some(BedrockOptions {
                common,
                bearer_token: Some("controlled".into()),
                ..Default::default()
            }),
            spawn,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Synchronous setup owner shared by the two original option families.
struct EagerProvider(std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>);
#[cfg(not(target_arch = "wasm32"))]
impl<E, O> maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for EagerProvider
where
    E: maestro_models::ProviderExecution,
    O: Into<maestro_models::StreamOptionsInput<E>>,
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        use maestro_models::{
            AssistantMessageEvent, AssistantMessageEventStream, DoneReason, StreamOptionsInput,
        };
        let label = match call.options.map(Into::into) {
            None => "absent",
            Some(StreamOptionsInput::Raw(options)) => {
                if options.extra.is_empty() {
                    "raw-empty"
                } else {
                    "raw-full"
                }
            }
            Some(StreamOptionsInput::Simple(options)) => {
                if options.reasoning.is_none() {
                    "simple-empty"
                } else {
                    "simple-full"
                }
            }
        };
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(label);
        if call.model.id == "failure" {
            return Err(maestro_models::DiagnosticErrorInfo::error("setup stopped"));
        }
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: std::sync::Arc::new(std::sync::RwLock::new(
                maestro_models::faux_assistant_message(
                    label,
                    maestro_models::FauxAssistantMessageOptions::default(),
                ),
            )),
        });
        Ok(stream)
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Setup failure is immediate for streams and eager but independently observed for completions.
macro_rules! eager_failure_operations {
    ($model:ident, $context:ident, $calls:ident, $mode:ty, $shape:ident,
     $raw:path, $simple:path, $complete:path, $complete_simple:path $(, $spawn:ident)?) => {{
        let mut failure = $model.clone();
        failure.id = "failure".into();
        let (raw, simple, expected) = eager_options::<$mode>($shape);
        let before = $calls.lock().unwrap().len();
        assert_eq!($raw(failure.clone(), $context.clone(), raw.clone() $(, $spawn.clone())?).err().unwrap().message, "setup stopped");
        assert_eq!($simple(failure.clone(), $context.clone(), simple.clone() $(, $spawn.clone())?).err().unwrap().message, "setup stopped");
        let c = $complete(failure.clone(), $context.clone(), raw $(, $spawn.clone())?);
        let d = $complete_simple(failure, $context.clone(), simple $(, $spawn.clone())?);
        assert_eq!(&$calls.lock().unwrap()[before..], &expected);
        crate::runtime::run::<crate::TestResult<_>>(async {
            assert_eq!(c.await.unwrap_err().message, "setup stopped");
            assert_eq!(d.await.unwrap_err().message, "setup stopped");
            Ok(())
        })?;
    }};
}

#[cfg(not(target_arch = "wasm32"))]
/// Run each original option shape through all four eager public operations.
macro_rules! eager_operations {
    ($model:ident, $context:ident, $calls:ident, $mode:ty,
     $raw:path, $simple:path, $complete:path, $complete_simple:path $(, $spawn:ident)?) => {{
        for shape in 0..3 {
            let (raw, simple, expected) = eager_options::<$mode>(shape);
            let before = $calls.lock().unwrap().len();
            let a = $raw($model.clone(), $context.clone(), raw.clone() $(, $spawn.clone())?)?;
            let b = $simple($model.clone(), $context.clone(), simple.clone() $(, $spawn.clone())?)?;
            let c = $complete($model.clone(), $context.clone(), raw $(, $spawn.clone())?);
            let d = $complete_simple($model.clone(), $context.clone(), simple $(, $spawn.clone())?);
            assert_eq!(&$calls.lock().unwrap()[before..], &expected);
            crate::runtime::run::<crate::TestResult<_>>(async {
                for (message, label) in [a.result().await, b.result().await, c.await?, d.await?].into_iter().zip(expected) {
                    let content = &message.read().unwrap().content;
                    assert_eq!(*content, [maestro_models::AssistantContent::Text(maestro_models::faux_text(label))]);
                }
                assert!(matches!(a.next().await, Some(maestro_models::AssistantMessageEvent::Done { .. })));
                assert!(matches!(b.next().await, Some(maestro_models::AssistantMessageEvent::Done { .. })));
                assert!(a.next().await.is_none());
                assert!(b.next().await.is_none());
                Ok(())
            })?;
            eager_failure_operations!($model, $context, $calls, $mode, shape, $raw, $simple, $complete, $complete_simple $(, $spawn)?);
        }
    }};
}

#[cfg(not(target_arch = "wasm32"))]
/// Absent, supplied-empty and supplied-full original options, with independent expected labels.
#[cfg(test)]
fn eager_options<E: maestro_models::ProviderExecution>(
    shape: u8,
) -> (
    Option<maestro_models::ProviderStreamOptions<E>>,
    Option<SimpleStreamOptions<E>>,
    [&'static str; 4],
) {
    match shape {
        0 => (None, None, ["absent"; 4]),
        1 => (
            Some(maestro_models::ProviderStreamOptions::default()),
            Some(SimpleStreamOptions::default()),
            ["raw-empty", "simple-empty", "raw-empty", "simple-empty"],
        ),
        2 => (
            Some(maestro_models::ProviderStreamOptions {
                extra: [("custom".into(), serde_json::json!(17))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            }),
            Some(SimpleStreamOptions {
                reasoning: Some(maestro_models::ThinkingLevel::High),
                ..Default::default()
            }),
            ["raw-full", "simple-full", "raw-full", "simple-full"],
        ),
        _ => unreachable!("only recorded option shapes are used"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dispatch_and_completion_keep_eager_error_boundaries() -> crate::TestResult {
    use maestro_models::{ApiProvider, Context};
    use std::sync::{Arc, Mutex};
    let _serial = crate::bundled::registry();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let owner = Arc::new(EagerProvider(Arc::clone(&calls)));
    maestro_models::register_api_provider(
        ApiProvider {
            api: "controlled-eager".into(),
            stream: owner.clone(),
            stream_simple: owner,
        },
        Some("eager-owner".into()),
    );
    let model = crate::bundled::descriptor("controlled-eager", "https://fixture.invalid")?;
    let context = Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    };
    eager_local(&model, &context, &calls)?;
    eager_threaded(&model, &context, &calls)?;
    eager_default(&model, &context, &calls)?;
    maestro_models::unregister_api_providers("eager-owner");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Exercise local eager success and setup failure with each original option shape.
#[cfg(test)]
fn eager_local(
    model: &Model,
    context: &maestro_models::Context,
    calls: &std::sync::Mutex<Vec<&'static str>>,
) -> crate::TestResult {
    let local: maestro_models::ProviderSpawner<Local> =
        Rc::new(|_| panic!("synchronous callback must not submit"));
    eager_operations!(
        model,
        context,
        calls,
        Local,
        maestro_models::stream_for::<Local>,
        maestro_models::stream_simple_for::<Local>,
        maestro_models::complete_for::<Local>,
        maestro_models::complete_simple_for::<Local>,
        local
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Exercise threaded eager success and setup failure with each original option shape.
#[cfg(test)]
fn eager_threaded(
    model: &Model,
    context: &maestro_models::Context,
    calls: &std::sync::Mutex<Vec<&'static str>>,
) -> crate::TestResult {
    let threaded: maestro_models::ProviderSpawner<maestro_models::Threaded> =
        std::sync::Arc::new(|_| panic!("synchronous callback must not submit"));
    eager_operations!(
        model,
        context,
        calls,
        maestro_models::Threaded,
        maestro_models::stream_for::<maestro_models::Threaded>,
        maestro_models::stream_simple_for::<maestro_models::Threaded>,
        maestro_models::complete_for::<maestro_models::Threaded>,
        maestro_models::complete_simple_for::<maestro_models::Threaded>,
        threaded
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Exercise default eager success and setup failure with each original option shape.
#[cfg(test)]
fn eager_default(
    model: &Model,
    context: &maestro_models::Context,
    calls: &std::sync::Mutex<Vec<&'static str>>,
) -> crate::TestResult {
    eager_operations!(
        model,
        context,
        calls,
        maestro_models::Threaded,
        maestro_models::stream,
        maestro_models::stream_simple,
        maestro_models::complete,
        maestro_models::complete_simple
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn registered_scripted_submission_keeps_local_hooks() -> crate::TestResult {
    use maestro_models::{ProviderInvocation, ProviderStreamOptions, RegisterFauxProviderOptions};
    let _serial = crate::bundled::registry();
    crate::runtime::run(tokio::task::LocalSet::new().run_until(async {
        let registration =
            maestro_models::register_faux_provider(RegisterFauxProviderOptions::default());
        let model = registration.get_model(None).unwrap().clone();
        let context = crate::bundled::conversation()?;
        queue_scripted_pair(&registration);
        let retained = maestro_models::get_api_provider(&registration.api).unwrap();
        registration.unregister();
        let tasks = Rc::new(std::cell::RefCell::new(Vec::new()));
        let admissions = Rc::new(Cell::new(0));
        let observed = Rc::new(Cell::new(0));
        let common = scripted_local_observer(Rc::clone(&observed));
        let local = retained.stream.call(
            model.clone(),
            context.clone(),
            ProviderInvocation::Local(
                Some(ProviderStreamOptions {
                    common,
                    ..Default::default()
                }),
                local_submission(Rc::clone(&tasks), Rc::clone(&admissions)),
            ),
        )?;
        assert_eq!(admissions.get(), 1);
        assert_eq!(observed.get(), 0);
        assert_eq!(
            (
                registration.state.call_count(),
                registration.get_pending_response_count()
            ),
            (1, 1)
        );
        let (_, first) = crate::transport::drain(&local).await?;
        for task in tasks.take() {
            task.await?;
        }
        assert_eq!(observed.get(), 2);
        assert_eq!(
            first.content,
            [maestro_models::AssistantContent::Text(
                maestro_models::faux_text("local")
            )]
        );
        retained_scripted_threaded(&retained, model, context, &registration).await?;
        assert_eq!(admissions.get(), 1);
        Ok(())
    }))
}

#[cfg(not(target_arch = "wasm32"))]
/// Count both sides of a suspended response hook on its caller-local state.
#[cfg(test)]
fn scripted_local_observer(hook_calls: Rc<Cell<usize>>) -> StreamOptions<Local> {
    StreamOptions::<Local> {
        on_response: Some(Rc::new(move |_, _| {
            hook_calls.set(hook_calls.get() + 1);
            let hook_calls = Rc::clone(&hook_calls);
            Box::pin(async move {
                tokio::task::yield_now().await;
                hook_calls.set(hook_calls.get() + 1);
                Ok(())
            })
        })),
        ..Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// A retained simple handle consumes the next response through its original native default.
#[cfg(test)]
async fn retained_scripted_threaded(
    retained: &maestro_models::ApiProvider,
    model: Model,
    context: maestro_models::Context,
    registration: &maestro_models::FauxProviderRegistration,
) -> crate::TestResult {
    use maestro_models::ProviderInvocation;
    let threaded =
        retained
            .stream_simple
            .call(model, context, ProviderInvocation::Threaded(None, None))?;
    assert_eq!(
        (
            registration.state.call_count(),
            registration.get_pending_response_count()
        ),
        (2, 0)
    );
    let (_, second) = crate::transport::drain(&threaded).await?;
    assert_eq!(
        second.content,
        [maestro_models::AssistantContent::Text(
            maestro_models::faux_text("threaded")
        )]
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Supply two distinguishable responses to the shared retained FIFO.
#[cfg(test)]
fn queue_scripted_pair(registration: &maestro_models::FauxProviderRegistration) {
    use maestro_models::FauxResponseStep;
    registration.set_responses(
        ["local", "threaded"]
            .into_iter()
            .map(|text| {
                FauxResponseStep::Message(Box::new(maestro_models::faux_assistant_message(
                    text,
                    maestro_models::FauxAssistantMessageOptions::default(),
                )))
            })
            .collect(),
    );
}

#[cfg(not(target_arch = "wasm32"))]
/// Retain each admitted threaded task using the same test admission owner.
#[cfg(test)]
fn threaded_submission(
    tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
) -> maestro_models::ProviderSpawner<maestro_models::Threaded> {
    Arc::new(move |task| {
        let task = tokio::spawn(task);
        tasks.lock().unwrap().push(task);
        true
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Exercise both cloud hook-capture families with the shared synchronized witnesses.
pub(crate) fn cloud_affinity() -> crate::TestResult {
    crate::runtime::run(tokio::task::LocalSet::new().run_until(bundled_cases::cloud_affinity()))
}
