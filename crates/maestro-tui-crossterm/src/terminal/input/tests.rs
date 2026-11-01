//! Shared input framing, UTF-8 completion, deadlines and retired generations.

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use tokio::runtime::Builder;
use tokio::task::LocalSet;
use tokio::time::Instant;

use super::{Input, Wake};
use crate::terminal::ProcessTerminal;

/// An input state with a live recording callback, without acquiring standard input.
fn input() -> (Input, Rc<RefCell<Vec<String>>>) {
    let terminal = ProcessTerminal::new(Rc::new(LocalSet::new()));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let record = Rc::clone(&seen);
    let generation = terminal.shared.install(
        Box::new(move |text| record.borrow_mut().push(text.to_owned())),
        Box::new(|| {}),
    );
    (
        Input::new(
            Rc::clone(&terminal.shared),
            generation,
            Instant::now() + Duration::from_millis(150),
        ),
        seen,
    )
}

/// Delivers a raw read to the shared input state.
fn read(input: &mut Input, bytes: &[u8]) {
    input.handle(Wake::Read(Ok(bytes.len())), bytes).unwrap();
}

#[test]
fn eof_flushes_incomplete_utf8_and_keeps_deadlines_alive() {
    drive(async {
        let (mut input, seen) = input();
        read(&mut input, &[0xe2, 0x82]);
        assert!(seen.borrow().is_empty());
        read(&mut input, b"");
        assert!(!input.reading());
        assert_eq!(&*seen.borrow(), &["\u{fffd}"]);
        assert!(input.next_deadline().is_some());
        input.handle(Wake::Resize, &[]).unwrap();
        assert!(input.is_current());
    });
}

#[test]
fn split_utf8_nul_and_paste_follow_the_same_framing_path() {
    drive(async {
        let (mut input, seen) = input();
        read(&mut input, &[0xc3]);
        read(&mut input, b"\xa9\0\x1b[200~paste\x1b[201~");
        assert_eq!(seen.borrow().concat(), "é\0\x1b[200~paste\x1b[201~");
    });
}

#[test]
fn fragment_deadline_releases_text_and_negotiation_deadline_selects_fallback() {
    drive(async {
        let (mut input, seen) = input();
        read(&mut input, b"\x1b");
        assert!(seen.borrow().is_empty());
        tokio::time::advance(Duration::from_secs(1)).await;
        input.handle(Wake::Deadline, &[]).unwrap();
        assert_eq!(&*seen.borrow(), &["\x1b"]);
        assert!(input.shared.fallback.get());
        assert!(input.next_deadline().is_none());
        input.handle(Wake::Deadline, &[]).unwrap();
    });
}

#[test]
fn retired_and_suspended_callbacks_receive_no_events() {
    drive(async {
        let (mut input, seen) = input();
        let callback = input.shared.on_input.take();
        read(&mut input, b"suppressed");
        assert!(seen.borrow().is_empty());
        input.shared.on_input.replace(callback);
        input.shared.retire();
        assert!(!input.is_current());
        read(&mut input, b"retired");
        assert!(seen.borrow().is_empty());
    });
}

#[test]
fn read_errors_propagate_and_are_kept_for_stop() {
    drive(async {
        let (mut input, _) = input();
        let error = input
            .handle(Wake::Read(Err(io::ErrorKind::BrokenPipe.into())), &[])
            .unwrap_err();
        input.fail(error);
        assert_eq!(
            input.shared.failure.borrow().as_ref().unwrap().kind(),
            io::ErrorKind::BrokenPipe
        );
    });
}

/// Drives one input action on a paused clock without acquiring standard input.
fn drive(action: impl Future<Output = ()>) {
    Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(action);
}
