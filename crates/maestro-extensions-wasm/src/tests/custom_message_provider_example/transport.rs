//! Client retry policy observed through the existing controlled native transport.
use super::*;

/// Reads only native waits from the controlled host's transcript.
fn waits(observed: &super::super::observed::Observed) -> Vec<&str> {
    observed
        .transcript
        .iter()
        .filter(|line| line.starts_with("native-wait"))
        .map(String::as_str)
        .collect()
}

#[test]
fn custom_message_provider_retries_only_transient_failures_twice() -> ExtensionResult<()> {
    for (status, hint, attempts) in [
        (408, None, 3),
        (409, None, 3),
        (429, None, 3),
        (500, None, 3),
        (400, None, 1),
        (503, Some("false"), 1),
        (400, Some("true"), 3),
        (200, Some("true"), 1),
    ] {
        let host = Controlled::default();
        host.observe().native_http.status = status;
        host.observe().native_http.body = Some(vec![response_events().into_bytes()]);
        if let Some(hint) = hint {
            host.observe().native_http.headers = vec![("X-Should-Retry".into(), hint.into())];
        }

        let (_, result) = stream(
            &host,
            default_options(),
        )?;

        let observed = host.observed();
        assert_eq!(
            observed.native_http.requests.len(),
            attempts,
            "status {status}, hint {hint:?}"
        );
        let reason = result.read().unwrap().stop_reason.clone();
        assert_eq!(
            reason,
            if status == 200 {
                crate::StopReason::ToolUse
            } else {
                crate::StopReason::Error
            }
        );
        assert_eq!(
            waits(&observed),
            if attempts == 3 {
                vec!["native-wait 0 500000000", "native-wait 1 0"]
            } else {
                vec![]
            }
        );
    }
    Ok(())
}

#[test]
fn custom_message_provider_retries_connection_errors_twice() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.error = Some("connection refused".into());

    let (_, result) = stream(
        &host,
        default_options(),
    )?;

    assert_eq!(host.observed().native_http.requests.len(), 3);
    assert_eq!(result.read().unwrap().stop_reason, crate::StopReason::Error);
    assert_eq!(
        result.read().unwrap().error_message.as_deref(),
        Some("connection refused")
    );
    Ok(())
}

#[test]
fn custom_message_provider_uses_retry_hints_and_jittered_backoff() -> ExtensionResult<()> {
    for (headers, random, expected) in [
        (
            vec![("Retry-After-Ms", "125"), ("Retry-After", "2")],
            0.0,
            vec!["native-wait 0 125000000"; 2],
        ),
        (
            vec![("Retry-After", "1.5")],
            0.0,
            vec!["native-wait 1 500000000"; 2],
        ),
        (
            vec![("Retry-After", "Thu, 01 Jan 1970 00:00:11 GMT")],
            0.0,
            vec!["native-wait 1 0"; 2],
        ),
        (
            vec![("Retry-After-Ms", "0"), ("Retry-After", "2")],
            0.0,
            vec!["native-wait 2 0"; 2],
        ),
        (
            vec![("Retry-After-Ms", "NaN"), ("Retry-After", "2")],
            0.0,
            vec!["native-wait 2 0"; 2],
        ),
        (
            vec![("Retry-After-Ms", "60000")],
            0.5,
            vec!["native-wait 0 437500000", "native-wait 0 875000000"],
        ),
        (
            vec![("Retry-After", "invalid")],
            0.5,
            vec!["native-wait 0 437500000", "native-wait 0 875000000"],
        ),
    ] {
        check_retry_hint(&headers, random, &expected)?;
    }
    Ok(())
}

/// Checks one server hint against the host's observed retry waits.
fn check_retry_hint(
    headers: &[(&str, &str)],
    random: f64,
    expected: &[&str],
) -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.status = 429;
    host.observe().native_http.headers = headers
        .iter()
        .map(|(name, value)| ((*name).into(), (*value).into()))
        .collect();
    let mut provider = authorization(&host);
    Rc::get_mut(&mut provider).unwrap().random = Rc::new(move || random);

    stream_provider(
        &provider,
        default_options(),
    )?;

    let observed = host.observed();
    assert_eq!(observed.native_http.requests.len(), 3);
    assert_eq!(
        waits(&observed),
        expected
    );
    Ok(())
}

#[test]
fn custom_message_provider_forwards_cancellation_until_body_consumption_ends() -> ExtensionResult<()>
{
    use std::{
        sync::{Arc, atomic::Ordering},
        task::{Context as TaskContext, Waker},
    };
    let host = Controlled::default();
    host.observe().native_http.body = Some(vec![response_events().into_bytes()]);
    let caller = super::super::controlled::Flag::default();
    let signal = crate::component_adapter::signal(&host, host.signal(&caller));
    let mut open = Vec::new();
    for _ in 0..3 {
        let (hold, _started, release) = super::super::observed::gate();
        host.observe().hold_next_idle(hold);
        open.push(release);
    }
    let provider = authorization(&host);
    let output = output_stream();
    let mut producing = provider.stream_custom_anthropic(
        model(),
        conversation(),
        Some(with_signal(signal)),
        output.clone(),
    );
    let mut cx = TaskContext::from_waker(Waker::noop());
    assert!(producing.as_mut().poll(&mut cx).is_pending());
    open.remove(0).send(()).unwrap();
    assert!(producing.as_mut().poll(&mut cx).is_pending());
    let native = Arc::clone(host.observed().native_http.signals[0].as_ref().unwrap());
    assert!(!native.load(Ordering::SeqCst));

    host.abort(&caller)
        .into_iter()
        .collect::<ExtensionResult<Vec<_>>>()?;

    assert!(
        native.load(Ordering::SeqCst),
        "caller cancellation must reach the native body"
    );
    open.pop().unwrap().send(()).unwrap();
    super::super::block_on(producing)?;
    let result = super::super::block_on(output.result())?;
    assert_eq!(
        result.read().unwrap().stop_reason,
        crate::StopReason::Aborted
    );
    assert!(host.observed().bus.listeners().is_empty());
    assert_eq!(host.observed().native_http.requests.len(), 1);
    assert!(
        open.pop().unwrap().send(()).is_err(),
        "headers deadline must be dropped"
    );
    Ok(())
}

#[test]
fn custom_message_provider_times_out_headers_after_ten_minutes_and_retries() -> ExtensionResult<()>
{
    let host = Controlled::default();
    host.observe().native_http.body = Some(vec![response_events().into_bytes()]);
    let (hold, started, open) = super::super::observed::gate();
    host.observe().hold_next_idle(hold);
    let caller = super::super::controlled::Flag::default();
    let caller_signal = crate::component_adapter::signal(&host, host.signal(&caller));

    let (_, result) = stream(&host, with_signal(caller_signal.clone()))?;

    assert_eq!(
        result.read().unwrap().stop_reason,
        crate::StopReason::ToolUse
    );
    super::super::block_on(started).unwrap();
    assert!(open.send(()).is_err(), "timed-out attempt must be dropped");
    let observed = host.observed();
    assert_eq!(observed.native_http.requests.len(), 1);
    assert!(
        observed.native_http.signals[0]
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)),
        "deadline must abort the native attempt"
    );
    assert!(
        !caller_signal.aborted(),
        "attempt deadline must not cancel the caller"
    );
    assert!(
        observed.bus.listeners().is_empty(),
        "caller forwarding must be removed"
    );
    assert_eq!(
        waits(&observed),
        ["native-wait 600 0", "native-wait 0 500000000"]
    );
    Ok(())
}
