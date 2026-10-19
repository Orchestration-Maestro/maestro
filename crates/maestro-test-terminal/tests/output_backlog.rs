//! Retained frame scheduling while terminal output is backed up.

use std::rc::Rc;

use maestro_tui::{TUI, TerminalImage};

use crate::support::components::Probe;
use crate::support::manual_runtime::{ManualRuntime, ms};
use crate::support::recording_terminal::RecordingTerminal;

/// A controlled output port with one observable component.
fn writer() -> (TUI, RecordingTerminal, ManualRuntime, Rc<Probe>) {
    let (tui, terminal, runtime) = crate::support::writer::writer();
    let probe = Probe::shared(&["frame-A"]);
    tui.add_child(probe.clone());
    (tui, terminal, runtime, probe)
}

#[test]
fn output_backlog_draws_latest_state_after_drain() {
    let (tui, terminal, runtime, probe) = writer();
    terminal.set_pending_output_bytes(8 * 1024 * 1024);
    tui.request_render(false);
    runtime
        .advance_to(ms(16))
        .into_iter()
        .for_each(Result::unwrap);
    assert_eq!(probe.renders.get(), 0);
    assert!(terminal.writes().is_empty());
    probe.set_lines(&["frame-B".to_owned()]);
    tui.request_render(false);
    runtime
        .advance_to(ms(26))
        .into_iter()
        .for_each(Result::unwrap);
    assert_eq!(probe.renders.get(), 0);
    terminal.set_pending_output_bytes(0);
    runtime
        .advance_to(ms(36))
        .into_iter()
        .for_each(Result::unwrap);
    assert_eq!(probe.renders.get(), 1);
    let bytes = terminal.writes().concat();
    assert!(bytes.contains("frame-B"));
    assert!(!bytes.contains("frame-A"));
}

#[allow(
    dead_code,
    reason = "Reuse the toolkit's existing default terminal adapters."
)]
#[path = "../../maestro-tui/tests/fixtures/terminals.rs"]
mod default_terminals;

#[test]
fn output_backlog_default_terminal_is_never_deferred() {
    use maestro_tui::Terminal;
    use std::cell::RefCell;
    let terminal = Rc::new(RefCell::new(default_terminals::Recording::new(false)));
    assert_eq!(terminal.borrow().pending_output_bytes(), 0);
    let runtime = ManualRuntime::new();
    let tui = TUI::new(
        terminal.clone(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        None,
    );
    let probe = Probe::shared(&["initial"]);
    tui.add_child(probe.clone());
    tui.request_render(false);
    runtime.settle().unwrap();
    let before = terminal.borrow().log.len();
    probe.set_lines(&["updated".to_owned()]);
    tui.request_render(false);
    runtime.settle().unwrap();
    let log = &terminal.borrow().log;
    assert!(log[..before].iter().any(|line| line.contains("initial")));
    assert!(log[before..].iter().any(|line| line.contains("updated")));
    assert_eq!(probe.renders.get(), 2);
}

#[test]
fn output_backlog_boundary_and_retry_cadence() {
    for count in [0, 262_143, 262_144, 262_145, 8 * 1024 * 1024] {
        let (tui, terminal, runtime, probe) = writer();
        terminal.set_pending_output_bytes(count);
        tui.request_render(false);
        runtime
            .advance_to(ms(16))
            .into_iter()
            .for_each(Result::unwrap);
        if count <= 262_144 {
            assert_eq!(probe.renders.get(), 1);
            assert_eq!(runtime.pending(), 0);
            assert!(terminal.writes().concat().contains("frame-A"));
            continue;
        }
        assert_eq!(probe.renders.get(), 0);
        assert_eq!(runtime.next_deadline(), Some(ms(26)));
        assert!(runtime.advance_to(ms(25)).is_empty());
        let results = runtime.advance_to(ms(26));
        assert_eq!(results.len(), 1);
        results.into_iter().for_each(Result::unwrap);
        assert_eq!(probe.renders.get(), 0);
        assert_eq!(runtime.next_deadline(), Some(ms(36)));
        terminal.set_pending_output_bytes(0);
        runtime
            .advance_to(ms(36))
            .into_iter()
            .for_each(Result::unwrap);
        assert_eq!(probe.renders.get(), 1);
        probe.set_lines(&["next".to_owned()]);
        tui.request_render(false);
        assert_eq!(runtime.next_deadline(), Some(ms(52)));
        runtime.settle().unwrap();
        assert_eq!(probe.renders.get(), 2);
    }
}

#[test]
fn output_backlog_preserves_forced_and_clear_intent() {
    for force in [false, true] {
        for clear in [false, true] {
            let (tui, terminal, runtime, probe) = writer();
            tui.set_clear_on_shrink(clear);
            probe.set_lines(&["one".to_owned(), "two".to_owned(), "three".to_owned()]);
            tui.request_render(false);
            runtime.settle().unwrap();
            let redraws = tui.full_redraws();
            terminal.clear_writes();
            terminal.set_pending_output_bytes(262_145);
            probe.set_lines(&["one".to_owned()]);
            tui.request_render(force);
            runtime
                .advance_to(ms(32))
                .into_iter()
                .for_each(Result::unwrap);
            assert_eq!(probe.renders.get(), 1);
            assert!(terminal.writes().is_empty());
            terminal.set_pending_output_bytes(0);
            runtime.settle().unwrap();
            assert_eq!(probe.renders.get(), 2);
            let bytes = terminal.writes().concat();
            assert_eq!(tui.full_redraws() > redraws, force || clear);
            assert_eq!(bytes.contains("\x1b[2J\x1b[H"), force || clear);
            if force {
                terminal.clear_writes();
                terminal.set_pending_output_bytes(262_145);
                tui.request_render(true);
                runtime.run_due().into_iter().for_each(Result::unwrap);
                assert_eq!(probe.renders.get(), 2);
                terminal.set_pending_output_bytes(0);
                runtime.settle().unwrap();
                assert!(terminal.writes().concat().contains("one"));
                assert_eq!(probe.renders.get(), 3);
            }
        }
    }
}

#[test]
fn output_backlog_retry_coalesces_and_stop_cancels() {
    let (tui, terminal, runtime, probe) = writer();
    terminal.set_pending_output_bytes(262_145);
    tui.start().unwrap();
    runtime
        .advance_to(ms(16))
        .into_iter()
        .for_each(Result::unwrap);
    for force in [false, true, false, true] {
        tui.request_render(force);
    }
    runtime.run_due().into_iter().for_each(Result::unwrap);
    assert_eq!(runtime.pending(), 1);
    assert_eq!(probe.renders.get(), 0);
    tui.stop().unwrap();
    assert_eq!(runtime.pending(), 0);
    terminal.clear_writes();
    terminal.set_pending_output_bytes(0);
    assert!(runtime.advance_to(ms(100)).is_empty());
    assert!(terminal.writes().is_empty());
    probe.set_lines(&["restart".to_owned()]);
    tui.start().unwrap();
    runtime.settle().unwrap();
    assert!(terminal.writes().concat().contains("restart"));
    terminal.set_pending_output_bytes(262_145);
    tui.request_render(false);
    runtime
        .advance_to(ms(116))
        .into_iter()
        .for_each(Result::unwrap);
    let weak = Rc::downgrade(&probe);
    drop(probe);
    drop(tui);
    assert!(weak.upgrade().is_none());
    terminal.clear_writes();
    terminal.set_pending_output_bytes(0);
    runtime.settle().unwrap();
    assert!(terminal.writes().is_empty());
}

#[test]
fn output_backlog_instances_keep_independent_state() {
    let (first, first_terminal, first_runtime, first_probe) = writer();
    let (second, second_terminal, second_runtime, second_probe) = writer();
    first_terminal.set_pending_output_bytes(262_145);
    second_terminal.set_pending_output_bytes(0);
    first.request_render(false);
    second.request_render(false);
    first_runtime
        .advance_to(ms(16))
        .into_iter()
        .for_each(Result::unwrap);
    second_runtime.settle().unwrap();
    assert_eq!(
        (first_probe.renders.get(), second_probe.renders.get()),
        (0, 1)
    );
    first_terminal.set_pending_output_bytes(0);
    second_terminal.set_pending_output_bytes(262_145);
    second_probe.set_lines(&["second".to_owned()]);
    second.request_render(false);
    first_runtime.settle().unwrap();
    second_runtime
        .advance_to(ms(32))
        .into_iter()
        .for_each(Result::unwrap);
    assert_eq!(
        (first_probe.renders.get(), second_probe.renders.get()),
        (1, 1)
    );
    second_terminal.set_pending_output_bytes(0);
    second_runtime.settle().unwrap();
    assert_eq!(second_probe.renders.get(), 2);
    assert!(second_terminal.writes().concat().contains("second"));
}
