//! Registered response observation keeps each protocol's existing read boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_models::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Read counters close when the retained producer is joined.
#[derive(Default)]
struct Reads {
    /// Actual body chunks consumed, across any setup retries.
    body: Arc<AtomicUsize>,
    /// Caller response callbacks invoked.
    responses: AtomicUsize,
    /// Chunks already admitted before the response callback is eligible.
    before_observation: usize,
}
/// Original request plus the observer's failure branch.
struct Case<'a> {
    /// The test policy of the protocol selected by the real registry.
    policy: &'a crate::bundled::ProtocolPolicy,
    /// Original raw or simple options.
    simple: bool,
    /// Response payload preparation selected by the payload hook.
    nonstream: bool,
    /// Response status observed before HTTP classification.
    status: u16,
}
/// Concrete closure coercion is the only family-specific policy setup.
trait Policy: ProviderExecution {
    /// Reject only after a real yield, retaining the caller's observation state.
    fn common(fetch: Fetch, reads: Arc<Reads>, nonstream: bool) -> StreamOptions<Self>;
}
/// One callback body with the family's original pointer and future representation.
macro_rules! policy {
    ($mode:ty, $pointer:ident, $capture:expr) => {
        impl Policy for $mode {
            fn common(fetch: Fetch, reads: Arc<Reads>, nonstream: bool) -> StreamOptions<Self> {
                let retained = $capture;
                StreamOptions {
                    fetch: Some(fetch),
                    transport: Some(Transport::Sse),
                    on_payload: Some($pointer::new(move |mut input, _| {
                        Box::pin(async move {
                            if nonstream {
                                input.payload["stream"] = serde_json::json!(false);
                            }
                            Ok(input)
                        })
                    })),
                    on_response: Some($pointer::new(move |_, _| {
                        let retained = retained.clone();
                        let reads = reads.clone();
                        reads.responses.fetch_add(1, Ordering::SeqCst);
                        assert_eq!(reads.body.load(Ordering::SeqCst), reads.before_observation);
                        Box::pin(async move {
                            tokio::task::yield_now().await;
                            assert_eq!(reads.body.load(Ordering::SeqCst), reads.before_observation);
                            drop(retained);
                            Err(DiagnosticErrorInfo::error("response stopped"))
                        })
                    })),
                    ..Default::default()
                }
            }
        }
    };
}
policy!(Local, Rc, Rc::new(Cell::new(0)));
policy!(Threaded, Arc, Arc::new(()));
/// Count actual chunk reads, not body construction or callback registration.
fn watched(case: &Case<'_>, reads: &Reads) -> crate::TestResult<crate::transport::Transport> {
    use crate::transport::Attempt;
    let chunks = if case.policy.api == "bedrock-converse-stream" {
        vec![
            crate::bedrock::event("messageStart", &serde_json::json!({"role":"assistant"}))?,
            crate::bedrock::event("messageStop", &serde_json::json!({"stopReason":"end_turn"}))?,
        ]
    } else if case.nonstream {
        vec![b"{}".to_vec()]
    } else {
        let Attempt::Respond { body, .. } = super::direct_response(case.policy.api) else {
            panic!("body fixture")
        };
        vec![body]
    };
    let body = reads.body.clone();
    let content_type = if case.nonstream {
        "application/json"
    } else {
        "text/event-stream"
    };
    Ok(crate::transport::transport(vec![
        Attempt::streaming_with_headers(
            case.status,
            &[("content-type", content_type)],
            move || {
                let body = body.clone();
                Box::pin(futures_util::stream::iter(chunks.clone().into_iter().map(
                    move |chunk| {
                        body.fetch_add(1, Ordering::SeqCst);
                        Ok(chunk)
                    },
                )))
            },
        ),
    ]))
}
#[test]
fn bundled_response_policies_survive_local_submission() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "bundled_response_policies_survive_local_submission",
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
            .case("response-policy")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    reset_api_providers();
    crate::runtime::run_paused(tokio::task::LocalSet::new().run_until(async {
        for protocol in crate::bundled::registered_apis() {
            cases(protocol, false, 200).await?;
        }
        cases("openai-responses", true, 200).await?;
        cases("azure-openai-responses", true, 200).await?;
        cases("openai-codex-responses", false, 503).await?;
        super::session_transport::exercise().await?;
        Ok(())
    }))
}
/// Repeat independent raw/simple and local/threaded choices with unequal read policies.
async fn cases(protocol: &str, nonstream: bool, status: u16) -> crate::TestResult {
    let policy = crate::bundled::policy(protocol);
    for simple in [false, true] {
        let case = Case {
            policy,
            simple,
            nonstream,
            status,
        };
        local(&case).await?;
        threaded(&case).await?;
    }
    Ok(())
}
/// The native SDK admits one first event; non-stream response preparation reads its JSON first.
fn counters(case: &Case<'_>) -> Arc<Reads> {
    Arc::new(Reads {
        before_observation: usize::from(
            case.policy.api == "bedrock-converse-stream" || case.nonstream,
        ),
        ..Default::default()
    })
}
/// A rejecting local observer keeps its non-Send state across the consuming suspension.
async fn local(case: &Case<'_>) -> crate::TestResult {
    let reads = counters(case);
    let transport = watched(case, &reads)?;
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let spawn = super::local_submission(tasks.clone(), admissions.clone());
    let mut common = Local::common(transport.fetch.clone(), reads.clone(), case.nonstream);
    common.api_key = Some(crate::bundled::api_key(case.policy.api));
    let stream = super::bundled_cases::registered::<Local>(
        case.policy.api,
        case.simple,
        common,
        Local::provided(spawn),
    )?;
    assert_eq!(admissions.get(), 1);
    let (events, result) = crate::transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    check(case, &reads, &transport, &events, &result);
    Ok(())
}
/// The threaded family reaches the same policy owner with supplied admission exactly once.
async fn threaded(case: &Case<'_>) -> crate::TestResult {
    let reads = counters(case);
    let transport = watched(case, &reads)?;
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let spawn = super::threaded_submission(tasks.clone());
    let mut common = Threaded::common(transport.fetch.clone(), reads.clone(), case.nonstream);
    common.api_key = Some(crate::bundled::api_key(case.policy.api));
    let stream = super::bundled_cases::registered::<Threaded>(
        case.policy.api,
        case.simple,
        common,
        Threaded::provided(spawn),
    )?;
    assert_eq!(tasks.lock().unwrap().len(), 1);
    let (events, result) = crate::transport::drain(&stream).await?;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    for task in tasks {
        task.await?;
    }
    check(case, &reads, &transport, &events, &result);
    Ok(())
}
/// Exact closed read/callback counts reject observing after reduction or before preparation.
fn check(
    case: &Case<'_>,
    reads: &Reads,
    transport: &crate::transport::Transport,
    events: &[String],
    result: &AssistantMessage,
) {
    if case.policy.response_hook {
        let attempts = if case.policy.api == "openai-codex-responses" {
            4
        } else {
            1
        };
        assert_eq!(transport.attempts(), attempts);
        assert_eq!(reads.responses.load(Ordering::SeqCst), attempts);
        assert_eq!(reads.body.load(Ordering::SeqCst), reads.before_observation);
        assert_eq!(result.error_message.as_deref(), Some("response stopped"));
        assert_eq!(result.stop_reason, StopReason::Error);
        assert_eq!(events.last().map(String::as_str), Some("error"));
    } else {
        assert_eq!(reads.responses.load(Ordering::SeqCst), 0);
        assert_eq!(reads.body.load(Ordering::SeqCst), 1);
        assert_eq!(result.stop_reason, StopReason::Stop);
        assert_eq!(events, ["start", "done"]);
    }
}
