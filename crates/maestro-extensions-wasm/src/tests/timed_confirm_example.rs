//! Registered timed dialogs on the controlled imports and the built component.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};
use std::task::{Context, Waker};

use super::example_support::{release_all, start};
use super::observed::{Observed, gate};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/timed_confirm.rs"]
mod timed_confirm;

/// The factory exported by the shipped component.
struct TimedConfirm;

impl Extension for TimedConfirm {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        timed_confirm::factory(api)
    }
}

/// One host decision and its independently recorded notification.
struct Case {
    /// Registered command name.
    command: &'static str,
    /// The host's reply, not a decision made by the guest.
    answer: Value,
    /// Whether the controlled five-second wait expires before the dialog answers.
    expires: bool,
    /// Exact notification text and severity.
    notification: (&'static str, &'static str),
}

/// Confirmation and selection decisions with their literal expected output.
fn automatic_cases() -> Vec<Case> {
    [
        ("timed", json!(true), "Confirmed by user!"),
        ("timed", json!(false), "Cancelled or timed out"),
        ("timed-select", json!("Option A"), "Selected: Option A"),
        ("timed-select", json!("Option B"), "Selected: Option B"),
        ("timed-select", json!("Option C"), "Selected: Option C"),
        (
            "timed-select",
            Value::Null,
            "Selection cancelled or timed out",
        ),
        (
            "timed-select",
            json!(""),
            "Selection cancelled or timed out",
        ),
    ]
    .into_iter()
    .map(|(command, answer, message)| Case {
        command,
        answer,
        expires: false,
        notification: (message, "Info"),
    })
    .collect()
}

/// Approval, manual cancellation and expiration of caller-owned cancellation.
fn signal_cases() -> Vec<Case> {
    [
        (true, false, "Confirmed by user!", "Info"),
        (false, false, "Cancelled by user", "Info"),
        (false, true, "Dialog timed out (auto-cancelled)", "Warning"),
    ]
    .into_iter()
    .map(|(answer, expires, message, kind)| Case {
        command: "timed-signal",
        answer: json!(answer),
        expires,
        notification: (message, kind),
    })
    .collect()
}

/// All metadata is preserved, including command order and countdown descriptions.
fn assert_registration(observed: &Observed) {
    assert_eq!(
        observed.registrations.commands,
        [
            (
                "timed".into(),
                Some("Show a timed confirmation dialog (auto-cancels in 5s with countdown)".into()),
                false
            ),
            (
                "timed-select".into(),
                Some("Show a timed select dialog (auto-cancels in 10s with countdown)".into()),
                false
            ),
            (
                "timed-signal".into(),
                Some("Show a timed confirm using AbortSignal (manual approach)".into()),
                false
            ),
        ]
    );
}

/// Compares complete UI receipts, including timeout, signal and settlement state.
fn assert_receipts(lines: &[String], case: &Case) {
    let manual = case.command == "timed-signal";
    let mut expected = Vec::new();
    if manual {
        expected.push((
            "notify".into(),
            json!(["Dialog will auto-cancel in 5 seconds...", "Info"]),
        ));
    }
    if case.command == "timed-select" {
        expected.push((
            "select".into(),
            json!([
                "Pick an option",
                ["Option A", "Option B", "Option C"],
                null,
                10000.0
            ]),
        ));
    } else {
        expected.push((
            "confirm".into(),
            json!([
                "Timed Confirmation",
                "This dialog will auto-cancel in 5 seconds. Confirm?",
                manual.then_some(false),
                if manual { None } else { Some(5000.0) }
            ]),
        ));
    }
    if manual {
        expected.push(("settle".into(), json!([case.expires])));
        assert!(lines.iter().any(|line| line == "native-wait 5 0"));
    }
    expected.push((
        "notify".into(),
        json!([case.notification.0, case.notification.1]),
    ));
    let actual: Vec<_> = super::ui_host::receipts(lines)
        .into_iter()
        .map(|receipt| (receipt.method, receipt.operands))
        .collect();
    assert_eq!(
        actual, expected,
        "command {} reply {}",
        case.command, case.answer
    );
}

#[test]
fn timed_confirm_commands_keep_timeouts_and_outcomes() -> Result<(), String> {
    let (host, exports) = super::block_on(start::<TimedConfirm>())?;
    assert_registration(&host.observed());
    for case in automatic_cases() {
        host.observe()
            .ui
            .control(&json!({"answer": case.answer}).to_string());
        let mark = host.observed().transcript.len();
        let key = host
            .observed()
            .callback(&format!("command {}", case.command))?;
        super::block_on(exports.invoke_command(
            key,
            "ignored".into(),
            host.command_context("/work"),
        ))?;
        assert_receipts(&host.observed().transcript[mark..], &case);
    }
    for (command, method) in [
        ("timed", "confirm"),
        ("timed-select", "select"),
        ("timed-signal", "confirm"),
    ] {
        host.observe()
            .ui
            .control(&json!({"method": method, "error": "dialog refused"}).to_string());
        let key = host.observed().callback(&format!("command {command}"))?;
        assert_eq!(
            super::block_on(exports.invoke_command(
                key,
                String::new(),
                host.command_context("/work")
            )),
            Err("dialog refused".into())
        );
    }
    release_all(
        &host,
        &[
            "command timed",
            "command timed-select",
            "command timed-signal",
        ],
    );
    Ok(())
}

#[test]
fn timed_signal_distinguishes_approval_cancellation_and_timeout() -> Result<(), String> {
    let (host, exports) = super::block_on(start::<TimedConfirm>())?;
    let key = host.observed().callback("command timed-signal")?;
    for case in signal_cases() {
        host.observe()
            .ui
            .control(&json!({"answer": case.answer}).to_string());
        let mark = host.observed().transcript.len();
        let (dialog, mut dialog_entered, open_dialog) = gate();
        let (clock, mut clock_entered, advance_clock) = gate();
        let mut advance_clock = Some(advance_clock);
        host.observe().hold_next_idle(dialog);
        host.observe().hold_next_idle(clock);
        let mut call =
            Box::pin(exports.invoke_command(key, String::new(), host.command_context("/work")));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(call.as_mut().poll(&mut cx).is_pending());
        assert_eq!(dialog_entered.try_recv(), Ok(()));
        assert_eq!(clock_entered.try_recv(), Ok(()));
        if case.expires {
            advance_clock.take().unwrap().send(()).unwrap();
            assert!(call.as_mut().poll(&mut cx).is_pending());
        }
        open_dialog.send(()).unwrap();
        super::block_on(call)?;
        if let Some(clock) = advance_clock {
            assert!(clock.is_closed(), "answering drops the unfinished wait");
        }
        assert_receipts(&host.observed().transcript[mark..], &case);
        assert!(host.observed().bus.listeners.is_empty());
    }
    release_all(
        &host,
        &[
            "command timed",
            "command timed-select",
            "command timed-signal",
        ],
    );
    Ok(())
}

/// Runs a signal dialog with separate host-controlled dialog and clock barriers.
fn component_signal(
    harness: &mut super::host::Harness,
    key: u32,
    case: &Case,
) -> Result<(), String> {
    let (dialog, entered_dialog, open_dialog) = gate();
    let (clock, entered_clock, advance_clock) = gate();
    let mut advance_clock = Some(advance_clock);
    harness.observed().hold_next_idle(dialog);
    harness.observed().hold_next_idle(clock);
    let (aborted, saw_abort) = tokio::sync::oneshot::channel();
    harness.store.data_mut().abort_notice = Some(aborted);
    let ctx = harness
        .command_context("/work")
        .map_err(|e| e.to_string())?;
    let exports = harness.exports.clone();
    super::block_on(harness.store.run_concurrent(async |accessor| {
        let call =
            exports.call_invoke_command(accessor, super::host::borrow(key), String::new(), ctx);
        let control = async {
            entered_dialog.await.unwrap();
            entered_clock.await.unwrap();
            if case.expires {
                advance_clock.take().unwrap().send(()).unwrap();
                saw_abort.await.unwrap();
            }
            open_dialog.send(()).unwrap();
        };
        super::join::alongside(call, control).await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())??;
    if let Some(clock) = advance_clock {
        assert!(clock.is_closed(), "answering drops the unfinished wait");
    }
    harness.store.data_mut().abort_notice = None;
    Ok(())
}

#[test]
fn timed_confirm_component_runs_registered_dialogs() -> Result<(), String> {
    let mut harness = super::host::Harness::example("timed_confirm")?;
    assert_registration(harness.observed());
    for case in automatic_cases().into_iter().chain(signal_cases()) {
        harness
            .observed()
            .ui
            .control(&json!({"answer": case.answer}).to_string());
        let mark = harness.observed().transcript.len();
        let key = harness
            .callback(&format!("command {}", case.command))
            .map_err(|e| e.to_string())?;
        if case.command == "timed-signal" {
            component_signal(&mut harness, key, &case)?;
        } else {
            super::example_support::invoke_command_on_component(
                &mut harness,
                key,
                "ignored",
                false,
            )?;
        }
        assert_receipts(&harness.observed().transcript[mark..], &case);
    }
    for name in [
        "command timed",
        "command timed-select",
        "command timed-signal",
    ] {
        let key = harness.callback(name).map_err(|e| e.to_string())?;
        super::block_on(harness.release(key)).map_err(|e| e.to_string())?;
    }
    assert_eq!(harness.observed().live, 0);
    Ok(())
}
