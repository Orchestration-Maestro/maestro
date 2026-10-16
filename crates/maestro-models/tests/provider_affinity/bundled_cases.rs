//! Registered bundled producers use the same worker as their direct entries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_models::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Caller observation never controls producer ownership.
#[derive(Clone, Copy)]
enum Observation {
    /// Drain all events before inspecting the independent result.
    Drain,
    /// Drop the event observer before driving the retained result.
    Dropped,
}
/// Eight delivered protocols, including the native SDK route.
const PROTOCOLS: [&str; 8] = [
    "openai-completions",
    "anthropic-messages",
    "openai-responses",
    "azure-openai-responses",
    "mistral-conversations",
    "google-generative-ai",
    "openai-codex-responses",
    "bedrock-converse-stream",
];
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bundled_registered_requests_keep_local_payload_hooks() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    if crate::child_process::child_case().is_none() {
        return crate::child_process::rerun(
            "bundled_registered_requests_keep_local_payload_hooks",
            "registered-protocols",
            &[
                ("AWS_BEARER_TOKEN_BEDROCK", "fixture-token"),
                ("AWS_REGION", "us-east-1"),
            ],
        );
    }
    reset_api_providers();
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            for protocol in PROTOCOLS {
                for simple in [false, true] {
                    local(protocol, simple, Observation::Drain).await?;
                    threaded(protocol, simple, Observation::Drain).await?;
                }
            }
            Ok(())
        }),
    )
}
/// Keep the cloud SDK's envelope and all other protocol fixtures at their owning seams.
fn transport(protocol: &str) -> crate::TestResult<crate::transport::Transport> {
    if protocol == "bedrock-converse-stream" {
        return Ok(crate::bedrock::serve(vec![
            crate::bedrock::event("messageStart", &serde_json::json!({"role":"assistant"}))?,
            crate::bedrock::event("messageStop", &serde_json::json!({"stopReason":"end_turn"}))?,
        ]));
    }
    Ok(crate::transport::transport(vec![super::direct_response(
        protocol,
    )]))
}
/// Build the same caller-supplied common controls for either execution family.
fn authenticate<E: ProviderExecution>(
    protocol: &str,
    mut common: StreamOptions<E>,
) -> StreamOptions<E> {
    common.api_key = Some(crate::bundled::api_key(protocol));
    common.transport = Some(Transport::Sse);
    common.temperature = Some(0.75);
    common.max_tokens = Some(17.0);
    common.headers = Some([("x-controlled".into(), "retained".into())].into());
    common
}
/// Checked registered raw/simple calls share one builtin callback owner.
pub(super) fn registered<E: ProviderExecution>(
    protocol: &str,
    simple: bool,
    common: StreamOptions<E>,
    submit: E::Submission,
) -> crate::TestResult<AssistantMessageEventStream> {
    let model = if protocol == "bedrock-converse-stream" {
        crate::bedrock::model("https://bedrock-runtime.us-east-1.amazonaws.com")?
    } else {
        crate::bundled::descriptor(protocol, "https://fixture.invalid")?
    };
    let context = crate::bundled::conversation()?;
    let provider = get_api_provider(protocol).unwrap();
    Ok(if simple {
        provider.stream_simple.call(
            model,
            context,
            E::submission::<SimpleStreamOptions>(
                Some(SimpleStreamOptions {
                    common,
                    ..Default::default()
                }),
                submit,
            ),
        )?
    } else {
        provider.stream.call(
            model,
            context,
            E::submission::<ProviderStreamOptions>(
                Some(ProviderStreamOptions {
                    common,
                    ..Default::default()
                }),
                submit,
            ),
        )?
    })
}
/// Local admission is mandatory and its non-Send producer is joined before exact counts.
async fn local(protocol: &str, simple: bool, observation: Observation) -> crate::TestResult {
    let transport = transport(protocol)?;
    let phases = Rc::new(RefCell::new(Vec::new()));
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let common = authenticate(
        protocol,
        super::local_common_options(phases.clone(), transport.fetch.clone()),
    );
    let spawn = super::local_submission(tasks.clone(), admissions.clone());
    let stream = registered::<Local>(protocol, simple, common, Local::provided(spawn))?;
    assert_eq!(admissions.get(), 1);
    assert!(phases.borrow().is_empty());
    let (events, result) = observe(stream, observation).await?;
    for task in tasks.take() {
        task.await?;
    }
    check(
        protocol,
        &transport,
        &phases.borrow(),
        events.as_deref(),
        &result,
    )
}
/// Supplied threaded admission never falls through to the provider's ambient default.
async fn threaded(protocol: &str, simple: bool, observation: Observation) -> crate::TestResult {
    let transport = transport(protocol)?;
    let phases = Arc::new(Mutex::new(Vec::new()));
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let common = authenticate(
        protocol,
        super::threaded_common_options(phases.clone(), transport.fetch.clone()),
    );
    let spawn = super::threaded_submission(tasks.clone());
    let stream = registered::<Threaded>(protocol, simple, common, Threaded::provided(spawn))?;
    assert_eq!(tasks.lock().unwrap().len(), 1);
    assert!(phases.lock().unwrap().is_empty());
    let (events, result) = observe(stream, observation).await?;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    for task in tasks {
        task.await?;
    }
    check(
        protocol,
        &transport,
        &phases.lock().unwrap(),
        events.as_deref(),
        &result,
    )
}
/// Post-hook wire fields and closed callback phases reject a registry-only preflight.
fn check(
    protocol: &str,
    transport: &crate::transport::Transport,
    phases: &[&str],
    events: Option<&[String]>,
    result: &AssistantMessage,
) -> crate::TestResult {
    assert_eq!(result.error_message, None, "{protocol}");
    assert_eq!(result.stop_reason, StopReason::Stop);
    if let Some(events) = events {
        assert_eq!(events, ["start", "done"]);
    }
    let expected: &[&str] = if matches!(protocol, "mistral-conversations" | "google-generative-ai")
    {
        &["payload-before", "payload-after"]
    } else {
        &[
            "payload-before",
            "payload-after",
            "response-before",
            "response-after",
        ]
    };
    assert_eq!(phases, expected, "{protocol}");
    let (_, body) = transport.first_request()?;
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    if protocol == "bedrock-converse-stream" {
        assert_eq!(
            body["inferenceConfig"]["temperature"],
            serde_json::json!(0.25)
        );
    } else {
        super::assert_sent_temperature(protocol, &body);
    }
    Ok(())
}

/// Dropping stream observation never revokes already-admitted production.
async fn observe(
    stream: AssistantMessageEventStream,
    observation: Observation,
) -> Result<(Option<Vec<String>>, AssistantMessage), Box<dyn std::error::Error>> {
    if matches!(observation, Observation::Dropped) {
        let result = stream.result();
        drop(stream);
        let message = result.await;
        let message = message.read().unwrap().clone();
        Ok((None, message))
    } else {
        let (events, result) = crate::transport::drain(&stream).await?;
        Ok((Some(events), result))
    }
}
#[test]
fn default_submission_preserves_protocol_runtime_policy() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    if crate::child_process::child_case().is_none() {
        return crate::child_process::rerun(
            "default_submission_preserves_protocol_runtime_policy",
            "runtime-policy",
            &[
                ("AWS_BEARER_TOKEN_BEDROCK", "fixture-token"),
                ("AWS_REGION", "us-east-1"),
            ],
        );
    }
    reset_api_providers();
    for protocol in PROTOCOLS {
        for simple in [false, true] {
            missing_runtime(protocol, simple)?;
        }
    }
    let registration = register_faux_provider(RegisterFauxProviderOptions::default());
    let first = default_scripted(&registration)?;
    crate::chat::block_on(false, async { check_default_scripted(&first).await })?;
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            let second = default_scripted(&registration)?;
            check_default_scripted(&second).await?;
            for protocol in PROTOCOLS {
                for simple in [false, true] {
                    local(protocol, simple, Observation::Dropped).await?;
                    threaded(protocol, simple, Observation::Dropped).await?;
                }
            }
            Ok(())
        }),
    )?;
    registration.unregister();
    Ok(())
}
/// Ambient HTTP admission rejects synchronously, before hooks or transport work.
fn missing_runtime(protocol: &str, simple: bool) -> crate::TestResult {
    let transport = transport(protocol)?;
    let phases = Arc::new(Mutex::new(Vec::new()));
    let common = authenticate(
        protocol,
        super::threaded_common_options(phases.clone(), transport.fetch.clone()),
    );
    let stream = registered::<Threaded>(protocol, simple, common, None)?;
    crate::chat::block_on(false, async {
        let (events, result) = crate::transport::drain(&stream).await?;
        assert_eq!(events, ["error"]);
        assert_eq!(
            result.error_message.as_deref(),
            Some("Streaming requires a running Tokio runtime.")
        );
        Ok(())
    })?;
    assert!(phases.lock().unwrap().is_empty());
    assert_eq!(transport.attempts(), 0);
    Ok(())
}
/// Simulator admission uses its existing independent runtime both outside and inside a caller runtime.
fn default_scripted(
    registration: &FauxProviderRegistration,
) -> crate::TestResult<AssistantMessageEventStream> {
    registration.set_responses(vec![FauxResponseStep::Message(Box::new(
        faux_assistant_message("ok", FauxAssistantMessageOptions::default()),
    ))]);
    Ok(stream(
        registration.get_model(None).unwrap().clone(),
        crate::bundled::conversation()?,
        None,
    )?)
}
/// Observe actual terminal text after the independent simulator producer has settled.
async fn check_default_scripted(stream: &AssistantMessageEventStream) -> crate::TestResult {
    let (_, message) = crate::transport::drain(stream).await?;
    assert_eq!(message.stop_reason, StopReason::Stop);
    assert_eq!(message.error_message, None);
    assert_eq!(message.content, [AssistantContent::Text(faux_text("ok"))]);
    Ok(())
}
