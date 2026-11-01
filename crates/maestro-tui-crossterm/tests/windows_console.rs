//! The process terminal on a Windows console: console modes, repeated start and stop,
//! draining, resizing and the one-input wait.
//!
//! Each scenario needs standard input and output that are a console in a known state, so the
//! test starts a copy of itself on a new console, and that copy starts the scenario with the
//! new console's input and output buffers as its standard handles.
#![cfg(test)]
#![cfg(windows)]

use std::cell::Cell;
use std::fs::OpenOptions;
use std::future::Future;
use std::pin::pin;
use std::process::Stdio;
use std::rc::Rc;
use std::task::{Context, Waker};
use std::time::Duration;

use crossterm_winapi::{Console, ConsoleMode, Handle, HandleType, ScreenBuffer, WindowPositions};
use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use maestro_tui_crossterm::terminal::wait_for_input;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

/// The input modes raw mode turns off.
const COOKED: u32 = 0x0001 | 0x0002 | 0x0004;

/// The window-size record mode.
const WINDOW_INPUT: u32 = 0x0008;

/// The escape-sequence input mode.
const VT_INPUT: u32 = 0x0200;

/// The escape-sequence output mode.
const VT_PROCESSING: u32 = 0x0004;

/// Runs `scenario` with a fresh console as standard input and output, or with the null
/// device as standard input when `null_input` is set.
fn on_console(test: &str, null_input: bool, scenario: impl FnOnce()) {
    maestro_test_process::on_console(
        test,
        |command| {
            let console = |name| {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(name)
                    .unwrap()
            };
            let input: Stdio = if null_input {
                Stdio::null()
            } else {
                console("CONIN$").into()
            };
            command.stdin(input).stdout(console("CONOUT$"))
        },
        scenario,
    )
    .unwrap();
}

/// The console mode of the standard handle `kind`.
fn mode(kind: HandleType) -> u32 {
    ConsoleMode::from(Handle::new(kind).unwrap())
        .mode()
        .unwrap()
}

/// Runs `scenario` on a current-thread runtime with a real clock and a local set.
fn drive<F: Future<Output = ()>>(scenario: impl FnOnce(Rc<LocalSet>) -> F) {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let local = Rc::new(LocalSet::new());
    runtime.block_on(local.run_until(scenario(Rc::clone(&local))));
}

/// Waits on the real clock until `done` holds.
async fn until(done: impl Fn() -> bool) {
    while !done() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// A terminal started with callbacks that count input chunks and resizes.
fn started(local: &Rc<LocalSet>) -> (ProcessTerminal, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let mut terminal = ProcessTerminal::new(Rc::clone(local));
    let (inputs, resizes) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (input, resize) = (Rc::clone(&inputs), Rc::clone(&resizes));
    terminal
        .start(
            Box::new(move |_| input.set(input.get() + 1)),
            Box::new(move || resize.set(resize.get() + 1)),
        )
        .unwrap();
    (terminal, inputs, resizes)
}

/// Both console modes of the scenario, after escape-sequence output is turned off so the
/// terminal has to turn it on.
fn fresh_modes() -> (u32, u32) {
    (mode(HandleType::InputHandle), fresh_output())
}

/// Turns escape-sequence output off and returns the output console mode.
fn fresh_output() -> u32 {
    let output = ConsoleMode::from(Handle::new(HandleType::OutputHandle).unwrap());
    output
        .set_mode(output.mode().unwrap() & !VT_PROCESSING)
        .unwrap();
    output.mode().unwrap()
}

/// The console modes of standard input and standard output.
fn modes() -> (u32, u32) {
    (
        mode(HandleType::InputHandle),
        mode(HandleType::OutputHandle),
    )
}

/// Checks that standard input is in raw mode with window-size records and escape-sequence
/// input, that its other flags are as they were in `original`, and that standard output
/// processes escape sequences.
fn assert_acquired(original: u32) {
    let input = mode(HandleType::InputHandle);
    let changed = COOKED | WINDOW_INPUT | VT_INPUT;
    assert_eq!(input & COOKED, 0, "raw input: {input:#x}");
    assert_eq!(input & (WINDOW_INPUT | VT_INPUT), WINDOW_INPUT | VT_INPUT);
    assert_eq!(input & !changed, original & !changed);
    assert_ne!(mode(HandleType::OutputHandle) & VT_PROCESSING, 0);
}

#[test]
fn start_applies_raw_escape_sequence_input_and_stop_restores_both_modes() {
    on_console(
        "start_applies_raw_escape_sequence_input_and_stop_restores_both_modes",
        false,
        acquire_and_restore,
    );
}

/// Starts and stops once, checking the modes in between and after.
fn acquire_and_restore() {
    let original = fresh_modes();
    drive(|local| async move {
        let (mut terminal, inputs, _) = started(&local);
        assert_acquired(original.0);
        terminal.stop().unwrap();
        assert_eq!(inputs.get(), 0);
    });
    assert_eq!(modes(), original);
}

#[test]
fn repeated_and_replacing_starts_restore_the_modes_saved_first() {
    on_console(
        "repeated_and_replacing_starts_restore_the_modes_saved_first",
        false,
        repeat_starts,
    );
}

/// Starts, replaces and stops three times, checking the modes each time.
fn repeat_starts() {
    let original = fresh_modes();
    drive(|local| async move {
        for _ in 0..3 {
            let (mut terminal, _, _) = started(&local);
            let replaced = terminal.start(Box::new(|_: &str| ()), Box::new(|| ()));
            replaced.unwrap();
            assert_acquired(original.0);
            terminal.stop().unwrap();
            assert_eq!(modes(), original);
            terminal.stop().unwrap();
        }
    });
}

#[test]
fn a_drain_with_no_input_ends_after_the_idle_time_and_delivers_nothing() {
    on_console(
        "a_drain_with_no_input_ends_after_the_idle_time_and_delivers_nothing",
        false,
        drain_idle,
    );
}

/// Drains a started terminal that receives no input.
fn drain_idle() {
    drive(|local| async move {
        let (mut terminal, inputs, _) = started(&local);
        let drain = terminal.drain_input(
            Some(Duration::from_secs(5)),
            Some(Duration::from_millis(20)),
        );
        drain.await.unwrap();
        assert!(!terminal.kitty_protocol_active());
        terminal.stop().unwrap();
        assert_eq!(inputs.get(), 0);
    });
}

#[test]
fn a_console_resize_reaches_the_resize_callback_with_the_new_size() {
    on_console(
        "a_console_resize_reaches_the_resize_callback_with_the_new_size",
        false,
        resize,
    );
}

/// Shrinks the console window and buffer of a started terminal.
fn resize() {
    drive(|local| async move {
        let (mut terminal, _, resizes) = started(&local);
        tokio::task::yield_now().await;
        let before = resizes.get();
        let window = WindowPositions {
            left: 0,
            right: 39,
            top: 0,
            bottom: 9,
        };
        Console::from(Handle::new(HandleType::OutputHandle).unwrap())
            .set_console_info(true, window)
            .unwrap();
        ScreenBuffer::new(Handle::new(HandleType::OutputHandle).unwrap())
            .set_size(40, 10)
            .unwrap();
        until(|| resizes.get() > before).await;
        assert_eq!((terminal.columns(), terminal.rows()), (40, 10));
        terminal.stop().unwrap();
    });
}

#[test]
fn null_input_stops_cleanly_and_restores_output() {
    on_console(
        "null_input_stops_cleanly_and_restores_output",
        true,
        null_input,
    );
}

/// Starts and stops with the null device as standard input.
fn null_input() {
    let original = fresh_output();
    drive(|local| async move {
        let (mut terminal, inputs, _) = started(&local);
        assert_ne!(mode(HandleType::OutputHandle) & VT_PROCESSING, 0);
        terminal.stop().unwrap();
        assert_eq!(inputs.get(), 0);
    });
    assert_eq!(mode(HandleType::OutputHandle), original);
}

#[test]
fn the_one_input_wait_acquires_raw_input_and_restores_it_when_dropped() {
    on_console(
        "the_one_input_wait_acquires_raw_input_and_restores_it_when_dropped",
        false,
        dropped_wait,
    );
}

/// Polls the one-input wait once, then drops it.
fn dropped_wait() {
    let original = fresh_modes();
    drive(|_| async move {
        let mut wait = pin!(wait_for_input());
        let mut context = Context::from_waker(Waker::noop());
        assert!(wait.as_mut().poll(&mut context).is_pending());
        let input = mode(HandleType::InputHandle);
        assert_eq!(input & COOKED, 0);
        assert_ne!(input & VT_INPUT, 0);
        assert_eq!(mode(HandleType::OutputHandle), original.1);
    });
    assert_eq!(modes(), original);
}

#[test]
fn concurrent_terminal_and_wait_restore_input_after_the_last_release() {
    on_console(
        "concurrent_terminal_and_wait_restore_input_after_the_last_release",
        false,
        concurrent_wait,
    );
}

/// Stops a terminal while a standalone wait still owns raw input, then releases the wait.
fn concurrent_wait() {
    let original = fresh_modes();
    drive(|local| async move {
        let (mut terminal, _, _) = started(&local);
        {
            let mut wait = pin!(wait_for_input());
            assert!(
                wait.as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            terminal.stop().unwrap();
            assert_eq!(mode(HandleType::InputHandle) & COOKED, 0);
        }
        assert_eq!(modes(), original);
    });
}
