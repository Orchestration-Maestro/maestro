#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Local cancellation uses each protocol's original signal-selection policy.
use maestro_models::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// Only selected-signal protocols accept a replacement signal from their payload callback.
#[derive(Clone, Copy)]
enum Selection {
    /// Abort the original signal while the hook is suspended.
    Original,
    /// Replace an active original with an aborted selection.
    Aborted,
    /// Replace an aborted original with an active selection.
    Active,
}
/// Preserve original cancellation everywhere, and both replacement directions where selected.
pub(super) async fn cancel(protocol: &str, simple: bool) -> crate::TestResult {
    run(protocol, simple, Selection::Original).await?;
    if crate::bundled::policy(protocol).selected_signal {
        run(protocol, simple, Selection::Aborted).await?;
        run(protocol, simple, Selection::Active).await?;
    }
    Ok(())
}
/// The public registered producer settles before its signal and terminal event are inspected.
async fn run(protocol: &str, simple: bool, selection: Selection) -> crate::TestResult {
    let policy = crate::bundled::policy(protocol);
    let transport = super::bundled_cases::transport(protocol)?;
    let original = Cancellation::new();
    let selected = Cancellation::new();
    let hook_original = original.clone();
    let hook_selected = selected.clone();
    let selected_signal = policy.selected_signal;
    let calls = Rc::new(Cell::new(0));
    let hook_calls = calls.clone();
    let common = StreamOptions::<Local> {
        api_key: Some(crate::bundled::api_key(protocol)),
        transport: Some(Transport::Sse),
        signal: Some(original.clone()),
        fetch: Some(transport.fetch.clone()),
        on_payload: Some(Rc::new(move |mut input, _| {
            let original = hook_original.clone();
            let selected = hook_selected.clone();
            let calls = hook_calls.clone();
            Box::pin(async move {
                calls.set(calls.get() + 1);
                tokio::task::yield_now().await;
                assert_eq!(input.signal.is_some(), selected_signal);
                match selection {
                    Selection::Original => original.abort(),
                    Selection::Aborted => {
                        selected.abort();
                        input.signal = Some(selected);
                    }
                    Selection::Active => {
                        original.abort();
                        input.signal = Some(selected);
                    }
                }
                Ok(input)
            })
        })),
        ..Default::default()
    };
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let spawn = super::local_submission(tasks.clone(), admissions.clone());
    let stream = super::bundled_cases::registered::<Local>(protocol, simple, common, spawn)?;
    let (events, result) = crate::transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(admissions.get(), 1);
    assert_eq!(
        original.is_aborted(),
        !matches!(selection, Selection::Aborted)
    );
    assert_eq!(
        selected.is_aborted(),
        matches!(selection, Selection::Aborted)
    );
    check(policy, selection, &transport, &events, &result);
    Ok(())
}
/// Request admission and terminal classification retain their separate signal owners.
fn check(
    policy: &crate::bundled::ProtocolPolicy,
    selection: Selection,
    transport: &crate::transport::Transport,
    events: &[String],
    result: &AssistantMessage,
) {
    if matches!(selection, Selection::Active) {
        assert_eq!(events, ["start", "error"]);
        assert_eq!(transport.attempts(), 1);
        assert_eq!(result.stop_reason, StopReason::Aborted);
    } else {
        assert_eq!(events.last().map(String::as_str), Some("error"));
        assert_eq!(
            result.stop_reason,
            if matches!(selection, Selection::Aborted) {
                StopReason::Error
            } else {
                StopReason::Aborted
            },
            "{}",
            policy.api
        );
        if policy.selected_signal {
            assert_eq!(transport.attempts(), 0);
        }
    }
}
/// The raw registered route must retain the injected message client and its controls.
pub(super) async fn injected_client() -> crate::TestResult {
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let selected = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = selected.clone();
    let signal = Cancellation::new();
    let client_signal = signal.clone();
    let client: AnthropicClient = Arc::new(move |payload, options| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(payload["temperature"], serde_json::json!(0.25));
        assert_eq!(options.timeout_ms, Some(123.0));
        assert_eq!(options.max_retries, Some(2.0));
        options.signal.unwrap().abort();
        assert!(client_signal.is_aborted());
        Box::pin(async { Err(DiagnosticErrorInfo::error("injected client stopped")) })
    });
    let mut raw = ProviderStreamOptions::<Local> {
        common: StreamOptions {
            api_key: Some("controlled".into()),
            timeout_ms: Some(123.0),
            max_retries: Some(2.0),
            signal: Some(signal),
            fetch: Some(Arc::new(|_| panic!("injected client must replace fetch"))),
            on_payload: Some(Rc::new(|mut input, _| {
                Box::pin(async move {
                    tokio::task::yield_now().await;
                    input.payload["temperature"] = serde_json::json!(0.25);
                    Ok(input)
                })
            })),
            ..Default::default()
        },
        ..Default::default()
    };
    raw.objects.insert(client);
    let stream = stream_for::<Local>(
        crate::bundled::descriptor("anthropic-messages", "https://fixture.invalid")?,
        crate::bundled::conversation()?,
        Some(raw),
        super::local_submission(tasks.clone(), admissions.clone()),
    )?;
    let (events, result) = crate::transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    assert_eq!(selected.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(admissions.get(), 1);
    assert_eq!(events, ["error"]);
    assert_eq!(
        result.error_message.as_deref(),
        Some("injected client stopped")
    );
    Ok(())
}
