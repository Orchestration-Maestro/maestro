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
/// Both exported routes must send the same request through the same producer.
#[derive(Clone, Copy)]
enum Entry {
    /// Checked dynamic registry entry.
    Registered,
    /// Protocol-specific mode-aware SDK entry.
    Direct,
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bundled_registered_requests_keep_local_payload_hooks() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    if std::env::var("MAESTRO_CHILD_CASE").ok().is_none() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "bundled_registered_requests_keep_local_payload_hooks",
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
            .env("MAESTRO_CHILD_CASE", "registered-protocols")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    reset_api_providers();
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            for protocol in crate::bundled::registered_apis()
                .into_iter()
                .filter(|api| *api != "google-vertex")
            {
                affinity(protocol).await?;
            }
            super::controls::injected_client().await?;
            Ok(())
        }),
    )
}
/// Keep the cloud SDK's envelope and all other protocol fixtures at their owning seams.
pub(super) fn transport(protocol: &str) -> crate::TestResult<crate::transport::Transport> {
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
/// Select a public entry without duplicating any protocol production.
fn invoke<E: ProviderExecution>(
    protocol: &str,
    simple: bool,
    common: StreamOptions<E>,
    spawn: ProviderSpawner<E>,
    entry: Entry,
) -> crate::TestResult<AssistantMessageEventStream> {
    if matches!(entry, Entry::Registered) {
        return registered::<E>(protocol, simple, common, E::provided(spawn));
    }
    let model = target(protocol)?;
    let context = crate::bundled::conversation()?;
    Ok(if protocol == "bedrock-converse-stream" {
        super::direct_cloud_stream::<E>(simple, model, context, common, spawn)
    } else if simple {
        super::direct_simple_stream::<E>(protocol, model, context, Some(common), spawn)
    } else {
        super::direct_stream::<E>(protocol, model, context, Some(common), spawn)
    })
}
/// The two routes receive the same real protocol descriptor.
fn target(protocol: &str) -> crate::TestResult<Model> {
    if protocol == "bedrock-converse-stream" {
        crate::bedrock::model("https://bedrock-runtime.us-east-1.amazonaws.com")
    } else {
        crate::bundled::descriptor(protocol, "https://fixture.invalid")
    }
}
/// Checked registered raw/simple calls share one builtin callback owner.
pub(super) fn registered<E: ProviderExecution>(
    protocol: &str,
    simple: bool,
    common: StreamOptions<E>,
    submit: E::Submission,
) -> crate::TestResult<AssistantMessageEventStream> {
    let model = target(protocol)?;
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
async fn local(
    protocol: &str,
    simple: bool,
    observation: Observation,
    entry: Entry,
) -> crate::TestResult<Vec<u8>> {
    let policy = crate::bundled::policy(protocol);
    let transport = transport(protocol)?;
    let phases = Rc::new(RefCell::new(Vec::new()));
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let common = authenticate(
        protocol,
        super::local_common_options(phases.clone(), transport.fetch.clone()),
    );
    let spawn = super::local_submission(tasks.clone(), admissions.clone());
    let stream = invoke::<Local>(protocol, simple, common, spawn, entry)?;
    assert_eq!(admissions.get(), 1);
    assert!(phases.borrow().is_empty());
    let (events, result) = observe(stream, observation).await?;
    for task in tasks.take() {
        task.await?;
    }
    check(
        policy,
        &transport,
        &phases.borrow(),
        events.as_deref(),
        &result,
    )
}
/// Supplied threaded admission never falls through to the provider's ambient default.
async fn threaded(
    protocol: &str,
    simple: bool,
    observation: Observation,
    entry: Entry,
) -> crate::TestResult<Vec<u8>> {
    let policy = crate::bundled::policy(protocol);
    let transport = transport(protocol)?;
    let phases = Arc::new(Mutex::new(Vec::new()));
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let common = authenticate(
        protocol,
        super::threaded_common_options(phases.clone(), transport.fetch.clone()),
    );
    let spawn = super::threaded_submission(tasks.clone());
    let stream = invoke::<Threaded>(protocol, simple, common, spawn, entry)?;
    assert_eq!(tasks.lock().unwrap().len(), 1);
    assert!(phases.lock().unwrap().is_empty());
    let (events, result) = observe(stream, observation).await?;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    for task in tasks {
        task.await?;
    }
    check(
        policy,
        &transport,
        &phases.lock().unwrap(),
        events.as_deref(),
        &result,
    )
}
/// Post-hook wire fields and closed callback phases reject a registry-only preflight.
fn check(
    policy: &crate::bundled::ProtocolPolicy,
    transport: &crate::transport::Transport,
    phases: &[&str],
    events: Option<&[String]>,
    result: &AssistantMessage,
) -> crate::TestResult<Vec<u8>> {
    let protocol = policy.api;
    assert_eq!(result.error_message, None, "{protocol}");
    assert_eq!(result.stop_reason, StopReason::Stop);
    if let Some(events) = events {
        assert_eq!(events, ["start", "done"]);
    }
    assert_eq!(phases, super::hook_phases(policy), "{protocol}");
    let (_, body) = transport.first_request()?;
    let value: serde_json::Value = serde_json::from_slice(&body)?;
    if protocol == "bedrock-converse-stream" {
        assert_eq!(
            value["inferenceConfig"]["temperature"],
            serde_json::json!(0.25)
        );
    } else {
        super::assert_sent_temperature(protocol, &value);
    }
    Ok(body)
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
fn protocol_test_policies_cover_the_registry() {
    let _serial = crate::bundled::registry();
    reset_api_providers();
    let registered = crate::bundled::registered();
    for api in &registered {
        assert_eq!(crate::bundled::policy(api).api, api);
    }
}
#[test]
fn default_submission_preserves_protocol_runtime_policy() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    if std::env::var("MAESTRO_CHILD_CASE").ok().is_none() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "default_submission_preserves_protocol_runtime_policy",
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
            .env("MAESTRO_CHILD_CASE", "runtime-policy")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    reset_api_providers();
    for protocol in crate::bundled::registered_apis() {
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
            for protocol in crate::bundled::registered_apis() {
                for simple in [false, true] {
                    local(protocol, simple, Observation::Dropped, Entry::Registered).await?;
                    threaded(protocol, simple, Observation::Dropped, Entry::Registered).await?;
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

/// Reuse both registered and direct execution-family witnesses for cloud content.
pub(super) async fn cloud_affinity() -> crate::TestResult {
    affinity("google-vertex").await
}

/// Compare all execution routes once for one protocol.
async fn affinity(protocol: &str) -> crate::TestResult {
    for simple in [false, true] {
        let local_body = local(protocol, simple, Observation::Drain, Entry::Registered).await?;
        let direct_body = local(protocol, simple, Observation::Drain, Entry::Direct).await?;
        let threaded_body =
            threaded(protocol, simple, Observation::Drain, Entry::Registered).await?;
        let threaded_direct = threaded(protocol, simple, Observation::Drain, Entry::Direct).await?;
        assert_eq!(local_body, direct_body, "{protocol}");
        assert_eq!(threaded_body, threaded_direct, "{protocol}");
        super::controls::cancel(protocol, simple).await?;
    }
    Ok(())
}
