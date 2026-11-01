//! Appearance synchronization through registered events, command replies and native wait gates.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::controlled::Controlled;
use super::documents::{Answer, Ended};
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::{Value, json};
use std::task::{Context, Poll, Waker};

#[path = "../../../../examples/extensions/mac_system_theme.rs"]
mod mac_system_theme;

/// Loads the shipped factory without a desktop.
struct SystemTheme;
impl Extension for SystemTheme {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        mac_system_theme::factory(api)
    }
}

/// Delivers a lifecycle event through the public adapter.
fn event(host: &Controlled, exports: &Exports<Controlled>, name: &str) -> Result<(), String> {
    let key = host.observed().callback(&format!("event {name}"))?;
    let outcome = super::block_on(exports.invoke_event(
        key,
        super::documents::document(name).to_string(),
        Capabilities {
            ctx: host.context("/controlled"),
            signal: None,
        },
    ))?;
    assert_eq!(
        Answer::of(super::guest_family::delivery(outcome))?.ended,
        Ended::Returned(None)
    );
    Ok(())
}

/// Supplies captured command output without launching a process.
fn reply(host: &Controlled, stdout: &str, code: i32, killed: bool) {
    host.observe().actions.value =
        json!({"stdout":stdout,"stderr":"controlled","code":code,"killed":killed});
}

/// The literal theme names requested from the host, including rejected names.
fn themes(lines: &[String]) -> Vec<Value> {
    super::ui_host::receipts(lines)
        .into_iter()
        .filter(|receipt| receipt.method == "setThemeName")
        .map(|receipt| receipt.operands)
        .collect()
}

/// Polls one host tail without a timer or watchdog.
fn poll(tail: &mut ExtensionFuture<'static, ()>) -> Poll<Result<(), String>> {
    tail.as_mut().poll(&mut Context::from_waker(Waker::noop()))
}

/// Supplies the next native wait and observes its synchronous disposal.
fn hold(
    host: &Controlled,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (gate, entered, open) = super::observed::gate();
    host.observe().hold_next_idle(gate);
    (entered, open)
}

/// Initial selection, change-only updates, command failures, restart and shutdown cleanup.
#[test]
fn mac_system_theme_tracks_appearance_and_cancels_waits() -> Result<(), String> {
    let (host, exports) = super::block_on(super::example_support::start::<SystemTheme>())?;
    assert!(themes(&host.observed().transcript).is_empty());
    reply(&host, " true\n", 0, false);
    event(&host, &exports, "session_start")?;
    assert_eq!(themes(&host.observed().transcript), [json!(["dark"])]);
    let listener = host.observed().bus.listeners[0].1;
    let key = host.observed().bus.tails[0];
    let mut tail: ExtensionFuture<'static, ()> = Box::pin(Exports::<Controlled>::invoke_tail(key));
    let (entered, open) = hold(&host);
    assert!(poll(&mut tail).is_pending());
    super::block_on(entered).unwrap();
    let next = changes(&host, &mut tail, open);
    host.observe().actions.error = None;
    reply(&host, "true", 0, false);
    event(&host, &exports, "session_start")?;
    assert!(
        next.is_closed(),
        "restart drops the previous wait immediately"
    );
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    let key = *host.observed().bus.tails.last().unwrap();
    let mut restarted: ExtensionFuture<'static, ()> =
        Box::pin(Exports::<Controlled>::invoke_tail(key));
    let (entered, open) = hold(&host);
    assert!(poll(&mut restarted).is_pending());
    super::block_on(entered).unwrap();
    event(&host, &exports, "session_shutdown")?;
    assert!(open.is_closed(), "shutdown drops its timer immediately");
    assert_eq!(poll(&mut restarted), Poll::Ready(Ok(())));
    event(&host, &exports, "session_shutdown")?;
    assert!(host.observed().bus.listeners.is_empty());
    observations_and_release(&host, listener)?;
    Ok(())
}

/// Checks literal command and timer requests, then releases every registration.
fn observations_and_release(host: &Controlled, listener: u32) -> Result<(), String> {
    let observed = host.observed();
    let commands: Vec<_> = observed
        .transcript
        .iter()
        .filter(|line| line.starts_with("action exec"))
        .collect();
    assert_eq!(commands.len(), 8);
    let expected = r#"action exec ["osascript",["-e","tell application \"System Events\" to tell appearance preferences to return dark mode"],null]"#;
    assert!(
        commands.iter().all(|line| line.as_str() == expected),
        "{commands:?}"
    );
    let waits: Vec<_> = observed
        .transcript
        .iter()
        .filter(|line| line.starts_with("native-wait"))
        .collect();
    assert_eq!(waits, vec![&"native-wait 2 0".to_owned(); 9]);
    let mut keys = vec![
        listener,
        observed.callback("event session_start")?,
        observed.callback("event session_shutdown")?,
    ];
    keys.extend(observed.bus.tails.iter().copied());
    drop(observed);
    for key in keys {
        crate::component_adapter::release(key);
    }
    assert_eq!(host.observed().live, 0);
    Ok(())
}

/// Opens successive waits with changed or failed command replies.
fn changes(
    host: &Controlled,
    tail: &mut ExtensionFuture<'static, ()>,
    mut open: tokio::sync::oneshot::Sender<()>,
) -> tokio::sync::oneshot::Sender<()> {
    for (stdout, code, killed, expected) in [
        ("true", 0, false, vec!["dark"]),
        ("false", 0, false, vec!["dark", "light"]),
        ("TRUE", 0, false, vec!["dark", "light"]),
        ("true", 1, false, vec!["dark", "light"]),
        ("true", 0, true, vec!["dark", "light"]),
        ("true", 0, false, vec!["dark", "light", "dark"]),
    ] {
        reply(host, stdout, code, killed);
        let (command_entered, command_open) = hold(host);
        open.send(()).unwrap();
        assert!(poll(tail).is_pending());
        super::block_on(command_entered).unwrap();
        let (entered, next) = hold(host);
        command_open.send(()).unwrap();
        assert!(poll(tail).is_pending());
        super::block_on(entered).unwrap();
        assert_eq!(
            themes(&host.observed().transcript),
            expected
                .into_iter()
                .map(|name| json!([name]))
                .collect::<Vec<_>>()
        );
        open = next;
    }
    host.observe().actions.error = Some("osascript missing".into());
    let (entered, next) = hold(host);
    open.send(()).unwrap();
    assert!(poll(tail).is_pending());
    super::block_on(entered).unwrap();
    assert_eq!(
        themes(&host.observed().transcript).last(),
        Some(&json!(["light"]))
    );
    next
}

/// Delivers a lifecycle event to the compiled component.
async fn component_event(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    exports: &super::host_family::guest::Guest,
    key: u32,
    name: &str,
    ctx: wasmtime::component::Resource<super::host::Ordinary>,
) -> wasmtime::Result<()> {
    let outcome = exports
        .call_invoke_event(
            accessor,
            super::host::borrow(key),
            super::documents::document(name).to_string(),
            super::host_family::guest::Capabilities { ctx, signal: None },
        )
        .await?
        .map_err(wasmtime::Error::msg)?;
    let answer = Answer::of(super::host_family::delivery(outcome)).map_err(wasmtime::Error::msg)?;
    assert_eq!(answer.ended, Ended::Returned(None));
    Ok(())
}

/// Runs a built component's initial fallback, delayed change and cleanup with supplied effects.
#[test]
fn mac_system_theme_component_uses_controlled_process_and_clock() -> Result<(), String> {
    let mut harness = super::host::Harness::example("mac_system_theme")?;
    component_lifecycle(&mut harness)?;
    assert!(harness.observed().bus.listeners.is_empty());
    for (_, result) in super::block_on(harness.release_all()) {
        result.map_err(|e| e.to_string())?;
    }
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

/// Drives an appearance transition while its host tail remains live.
fn component_lifecycle(harness: &mut super::host::Harness) -> Result<(), String> {
    let start = harness
        .callback("event session_start")
        .map_err(|e| e.to_string())?;
    let shutdown = harness
        .callback("event session_shutdown")
        .map_err(|e| e.to_string())?;
    let ctx = harness.ordinary("/controlled").map_err(|e| e.to_string())?;
    let closing = harness.ordinary("/controlled").map_err(|e| e.to_string())?;
    let exports = harness.exports.clone();
    harness.observed().actions.error = Some("osascript unavailable".into());
    super::block_on(harness.store.run_concurrent(async |accessor| {
        component_event(accessor, &exports, start, "session_start", ctx).await?;
        let (entered, open) = accessor.with(|mut a| {
            assert_eq!(themes(&a.get().observed.transcript), [json!(["light"])]);
            a.get().observed.actions.error = None;
            a.get().observed.actions.value =
                json!({"stdout":"true\n","stderr":"","code":0,"killed":false});
            let (gate, entered, open) = super::observed::gate();
            a.get().observed.hold_next_idle(gate);
            (entered, open)
        });
        let key = accessor.with(|mut a| *a.get().observed.bus.tails.last().unwrap());
        let tail = exports.call_invoke_tail(accessor, super::host::borrow(key));
        let control = component_control(accessor, &exports, shutdown, closing, (entered, open));
        super::join::alongside(tail, control)
            .await?
            .map_err(wasmtime::Error::msg)?;
        Ok::<_, wasmtime::Error>(())
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Releases one timer and command before shutting down the next pending wait.
async fn component_control(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    exports: &super::host_family::guest::Guest,
    shutdown: u32,
    closing: wasmtime::component::Resource<super::host::Ordinary>,
    waiting: (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
) {
    let (entered, open) = waiting;
    entered.await.unwrap();
    let (gate, entered, command_open) = super::observed::gate();
    accessor.with(|mut a| a.get().observed.hold_next_idle(gate));
    open.send(()).unwrap();
    entered.await.unwrap();
    let (gate, entered, pending) = super::observed::gate();
    accessor.with(|mut a| a.get().observed.hold_next_idle(gate));
    command_open.send(()).unwrap();
    entered.await.unwrap();
    accessor.with(|mut a| {
        assert_eq!(
            themes(&a.get().observed.transcript),
            [json!(["light"]), json!(["dark"])]
        );
        assert!(
            a.get()
                .observed
                .transcript
                .contains(&"native-wait 2 0".to_owned())
        );
    });
    component_event(accessor, exports, shutdown, "session_shutdown", closing)
        .await
        .unwrap();
    assert!(pending.is_closed());
}
