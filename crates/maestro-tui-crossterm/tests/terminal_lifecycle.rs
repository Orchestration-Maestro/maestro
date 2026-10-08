//! Process terminal lifecycle: mode ownership, streamed input, keyboard negotiation, draining
//! and restart. Each scenario runs in a child process that owns its standard descriptors.
#![cfg(test)]
#![cfg(unix)]

mod lifecycle_support;
#[macro_use]
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use lifecycle_support::{
    UTF8_STREAMS, attributes, directory_input, file_input, open_descriptors, pty_input,
    socket_input, stdin_attributes, stdin_flags, unread_input, until, window_change,
};
use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use rustix::fs::OFlags;
use rustix::io::Errno;
use rustix::stdio::{stdin, stdout};
use rustix::termios::{
    LocalModes, OptionalActions, OutputModes, SpecialCodeIndex, Termios, tcsetattr,
};
use support::{
    Capture, Feed, Inputs, Resizes, Rig, START_BYTES, advance, pipe_input, pipe_output, pty_output,
    settle,
};
use tokio::task::LocalSet;

/// A started terminal and what it delivered.
struct Session {
    /// The terminal under test.
    terminal: ProcessTerminal,
    /// The input chunks it delivered.
    inputs: Inputs,
    /// The window-size notices it delivered.
    resizes: Resizes,
}

/// Starts a terminal that records everything it delivers.
fn start(rig: &Rig) -> Session {
    let (inputs, resizes) = (Inputs::default(), Resizes::default());
    let mut terminal = ProcessTerminal::new(rig.local());
    terminal
        .start(inputs.callback(), resizes.callback())
        .unwrap();
    Session {
        terminal,
        inputs,
        resizes,
    }
}

/// Checks the raw-mode settings the terminal applies: no line editing, echo or signal keys,
/// output post-processing kept, one-byte reads.
fn assert_raw(attributes: &Termios) {
    let line_modes = LocalModes::ICANON | LocalModes::ECHO | LocalModes::ISIG | LocalModes::IEXTEN;
    assert!(!attributes.local_modes.intersects(line_modes));
    assert!(attributes.output_modes.contains(OutputModes::ONLCR));
    assert_eq!(attributes.special_codes[SpecialCodeIndex::VMIN], 1);
    assert_eq!(attributes.special_codes[SpecialCodeIndex::VTIME], 0);
}

/// Every way `bytes` can be cut into consecutive chunks.
fn partitions(bytes: &[u8]) -> Vec<Vec<&[u8]>> {
    (0..1_u32 << (bytes.len() - 1))
        .map(|mask| {
            let (mut chunks, mut from) = (Vec::new(), 0);
            for cut in 1..bytes.len() {
                if mask & (1 << (cut - 1)) != 0 {
                    chunks.push(&bytes[from..cut]);
                    from = cut;
                }
            }
            chunks.push(&bytes[from..]);
            chunks
        })
        .collect()
}

/// Sends each chunk and lets the input task consume it, returning what had been delivered
/// after each one.
async fn send_each(
    rig: &Rig,
    feed: &Feed,
    session: &Session,
    chunks: &[&[u8]],
) -> Vec<Vec<String>> {
    let mut seen = Vec::new();
    for chunk in chunks {
        feed.send(chunk);
        settle(&rig.local()).await;
        seen.push(session.inputs.all());
    }
    seen
}

/// Starts a fresh session, sends each chunk in turn and returns the session with what had
/// been delivered after each.
async fn send_split(rig: &Rig, feed: &Feed, chunks: &[&[u8]]) -> (Session, Vec<Vec<String>>) {
    let session = start(rig);
    let seen = send_each(rig, feed, &session, chunks).await;
    (session, seen)
}

#[test]
fn native_start_stop_restores_modes_and_listeners() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pty_input();
        let output = pty_output();
        output.resize(80, 24);
        let (cooked, flags) = (stdin_attributes(), stdin_flags());
        rig.run(async {
            let mut session = start(&rig);
            assert_eq!(output.take(), "\x1b[?2004h\x1b[?u");
            assert_raw(&attributes(stdin()));
            assert!(stdin_flags().contains(OFlags::NONBLOCK));
            output.resize(100, 30);
            window_change();
            until(&rig.local(), || session.resizes.count() == 1).await;
            assert_eq!(
                (session.terminal.columns(), session.terminal.rows()),
                (100, 30)
            );
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l");
            assert_eq!((stdin_attributes(), stdin_flags()), (cooked, flags));
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l");
            output.resize(120, 40);
            window_change();
            settle(&rig.local()).await;
            assert_eq!(session.resizes.count(), 1);
        });
    });
}

#[test]
fn native_preserves_preexisting_raw_mode() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pty_input();
        let _output = pipe_output();
        let mut custom = attributes(stdin());
        custom.make_raw();
        custom.local_modes.insert(LocalModes::ECHOK);
        custom.special_codes[SpecialCodeIndex::VTIME] = 7;
        tcsetattr(stdin(), OptionalActions::Now, &custom).unwrap();
        let before = stdin_attributes();
        rig.run(async {
            let mut session = start(&rig);
            assert_eq!(
                attributes(stdin()).special_codes[SpecialCodeIndex::VTIME],
                0
            );
            session.terminal.stop().unwrap();
        });
        assert_eq!(stdin_attributes(), before);
    });
}

#[test]
fn native_accepts_input_without_a_raw_mode_method() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _output = pipe_output();
        rig.run(async {
            let pipe = pipe_input();
            let mut session = start(&rig);
            pipe.send(b"ab");
            until(&rig.local(), || session.inputs.count() == 2).await;
            pipe.close();
            settle(&rig.local()).await;
            session.terminal.stop().unwrap();
            assert_eq!(session.inputs.all(), ["a", "b"]);

            let socket = socket_input();
            let mut session = start(&rig);
            socket.send(b"cd");
            until(&rig.local(), || session.inputs.count() == 2).await;
            session.terminal.stop().unwrap();
            assert_eq!(session.inputs.all(), ["c", "d"]);

            file_input(b"ef");
            let mut session = start(&rig);
            until(&rig.local(), || session.inputs.count() == 2).await;
            session.terminal.stop().unwrap();
            assert_eq!(session.inputs.all(), ["e", "f"]);
        });
    });
}

#[test]
fn native_decodes_every_utf8_split_before_buffering() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _output = pipe_output();
        rig.run(async {
            for cuts in partitions("🎉".as_bytes()) {
                let feed = pipe_input();
                let mut chunks: Vec<&[u8]> = vec![b"A"];
                chunks.extend(&cuts);
                chunks.push(b"B");
                let (mut session, seen) = send_split(&rig, &feed, &chunks).await;
                let expected = ["A", "🎉", "B"].map(String::from);
                for snapshot in &seen {
                    assert!(
                        expected.starts_with(snapshot),
                        "{cuts:?} delivered {snapshot:?}"
                    );
                }
                assert_eq!(session.inputs.all(), expected, "{cuts:?}");
                session.terminal.stop().unwrap();
            }
        });
    });
}

#[test]
fn native_suppresses_fragmented_matching_emoji_once() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _output = pipe_output();
        rig.run(async {
            for cuts in partitions("🎉".as_bytes()) {
                let feed = pipe_input();
                let mut chunks: Vec<&[u8]> = vec![b"\x1b[127881u"];
                chunks.extend(&cuts);
                chunks.push(b"x");
                let (mut session, _) = send_split(&rig, &feed, &chunks).await;
                assert_eq!(session.inputs.all(), ["\x1b[127881u", "x"], "{cuts:?}");
                session.terminal.stop().unwrap();
            }
        });
    });
}

#[test]
fn native_wraps_streamed_paste_as_one_input() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _output = pipe_output();
        rig.run(async {
            for cuts in partitions("🎉".as_bytes()) {
                let feed = pipe_input();
                let mut chunks: Vec<&[u8]> = vec![b"\x1b[200~"];
                chunks.extend(&cuts);
                chunks.push(b"\x1b[201~");
                let (mut session, _) = send_split(&rig, &feed, &chunks).await;
                assert_eq!(session.inputs.all(), ["\x1b[200~🎉\x1b[201~"], "{cuts:?}");
                session.terminal.stop().unwrap();
            }
            let feed = pipe_input();
            let (mut session, _) = send_split(&rig, &feed, &[b"\x1b[200~\x1b[201~"]).await;
            assert_eq!(session.inputs.all(), ["\x1b[200~\x1b[201~"]);
            session.terminal.stop().unwrap();
            let feed = pipe_input();
            let adjacent = b"a\x1b[200~x\x1b[201~b\x1b[200~y\x1b[201~c";
            let (mut session, _) = send_split(&rig, &feed, &[adjacent]).await;
            let expected = ["a", "\x1b[200~x\x1b[201~", "b", "\x1b[200~y\x1b[201~", "c"];
            assert_eq!(session.inputs.all(), expected);
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_replaces_invalid_utf8_and_flushes_decoder_at_eof() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _output = pipe_output();
        rig.run(async {
            for &(chunks, text) in UTF8_STREAMS {
                let feed = pipe_input();
                let (mut session, _) = send_split(&rig, &feed, chunks).await;
                feed.close();
                settle(&rig.local()).await;
                assert_eq!(session.inputs.all().concat(), text, "{chunks:?}");
                session.terminal.stop().unwrap();
            }
        });
    });
}

/// Starts a session over a fresh pipe and drops the start-up bytes from `output`.
fn started(rig: &Rig, output: &Capture) -> (Feed, Session) {
    let feed = pipe_input();
    let session = start(rig);
    assert_eq!(output.take(), START_BYTES);
    (feed, session)
}

#[test]
fn native_enables_fallback_at_150ms() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (_feed, mut session) = started(&rig, &output);
            advance(&rig.local(), 149).await;
            assert_eq!(output.take(), "");
            advance(&rig.local(), 1).await;
            assert_eq!(output.take(), "\x1b[>4;2m");
            advance(&rig.local(), 1000).await;
            assert_eq!(output.take(), "");
            assert!(!session.terminal.kitty_protocol_active());
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l\x1b[>4;0m");
        });
    });
}

#[test]
fn native_enables_flags_and_consumes_first_response() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            feed.send(b"\x1b[?0u");
            settle(&rig.local()).await;
            assert_eq!(output.take(), "\x1b[>7u");
            assert!(session.terminal.kitty_protocol_active());
            assert!(maestro_tui::is_kitty_protocol_active());
            advance(&rig.local(), 150).await;
            assert_eq!(output.take(), "");
            feed.send(b"\x1b[?1u");
            settle(&rig.local()).await;
            assert_eq!(session.inputs.all(), ["\x1b[?1u"]);
            assert_eq!(output.take(), "");
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l\x1b[<u");
            assert!(!session.terminal.kitty_protocol_active());
            assert!(!maestro_tui::is_kitty_protocol_active());
        });
    });
}

/// Starts a terminal whose fallback mode was enabled and whose support reply came late, so
/// both keyboard modes are on, and returns it with its input and what it has written.
async fn with_both_keyboard_modes(rig: &Rig, output: &Capture) -> (Feed, Session) {
    let (feed, session) = started(rig, output);
    advance(&rig.local(), 150).await;
    assert_eq!(output.take(), "\x1b[>4;2m");
    feed.send(b"\x1b[?25u");
    settle(&rig.local()).await;
    assert_eq!(output.take(), "\x1b[>7u");
    assert!(session.terminal.kitty_protocol_active());
    assert!(session.inputs.all().is_empty());
    (feed, session)
}

#[test]
fn native_accepts_late_support_without_disabling_fallback() {
    isolated!(0, &[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (_feed, mut session) = with_both_keyboard_modes(&rig, &output).await;
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l\x1b[<u\x1b[>4;0m");
        });
    });
    isolated!(1, &[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (_feed, mut session) = with_both_keyboard_modes(&rig, &output).await;
            let (max, idle) = (Duration::ZERO, Duration::ZERO);
            session
                .terminal
                .drain_input(Some(max), Some(idle))
                .await
                .unwrap();
            assert_eq!(output.take(), "\x1b[<u\x1b[>4;0m");
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l");
        });
    });
}

#[test]
fn native_recognizes_only_ascii_decimal_protocol_replies() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let replies = ["\x1b[?0u", "\x1b[?000u", "\x1b[?999999999999999999999999u"];
            let others = ["\x1b[?u", "\x1b[?-1u", "\x1b[?\u{ff11}u", "\x1b[?1;2u"];
            for (reply, enables) in replies
                .map(|r| (r, true))
                .into_iter()
                .chain(others.map(|o| (o, false)))
            {
                let (feed, mut session) = started(&rig, &output);
                feed.send(reply.as_bytes());
                settle(&rig.local()).await;
                let (written, delivered) = (output.take(), session.inputs.all());
                assert_eq!(written, if enables { "\x1b[>7u" } else { "" }, "{reply:?}");
                assert_eq!(delivered.is_empty(), enables, "{reply:?}");
                session.terminal.stop().unwrap();
                output.take();
            }
        });
    });
}

#[test]
fn native_buffers_split_replies_until_complete() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let reply = b"\x1b[?12u";
            for cut in 1..reply.len() {
                let (feed, mut session) = started(&rig, &output);
                feed.send(&reply[..cut]);
                settle(&rig.local()).await;
                assert_eq!(
                    (output.take(), session.inputs.count()),
                    (String::new(), 0),
                    "{cut}"
                );
                feed.send(&reply[cut..]);
                settle(&rig.local()).await;
                assert_eq!(
                    (output.take(), session.inputs.count()),
                    ("\x1b[>7u".into(), 0),
                    "{cut}"
                );
                session.terminal.stop().unwrap();
                output.take();
            }
            let (feed, mut session) = started(&rig, &output);
            feed.send(b"\x1b[?");
            advance(&rig.local(), 9).await;
            feed.send(b"7u");
            settle(&rig.local()).await;
            assert_eq!(
                (output.take(), session.inputs.count()),
                ("\x1b[>7u".into(), 0)
            );
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_expires_only_the_pending_fragment() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            feed.send(b"\x1b[?");
            settle(&rig.local()).await;
            advance(&rig.local(), 9).await;
            assert!(session.inputs.all().is_empty());
            advance(&rig.local(), 1).await;
            assert_eq!(session.inputs.all(), ["\x1b[?"]);
            feed.send(b"7u");
            settle(&rig.local()).await;
            assert_eq!(session.inputs.all(), ["\x1b[?", "7", "u"]);
            assert_eq!(output.take(), "");
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_stop_cancels_all_mode_enable_work() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (_feed, mut session) = started(&rig, &output);
            session.terminal.stop().unwrap();
            advance(&rig.local(), 150).await;
            advance(&rig.local(), 1000).await;
            assert_eq!(output.take(), "\x1b[?2004l");
        });
    });
}

#[test]
fn native_stop_discards_pending_input() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let pending: [(&[u8], &[u8]); 3] = [
                (b"\x1b[", b"A"),
                (b"\x1b[200~unfinished", b"B"),
                (&[240, 159], &[142, 137]),
            ];
            for (before, after) in pending {
                let (feed, mut session) = started(&rig, &output);
                feed.send(before);
                settle(&rig.local()).await;
                session.terminal.stop().unwrap();
                feed.send(after);
                advance(&rig.local(), 10).await;
                assert!(session.inputs.all().is_empty(), "{before:?}");
                assert_eq!(unread_input(), after, "{before:?}");
                output.take();
            }
        });
    });
}

#[test]
fn native_restart_ignores_prior_generation_timers() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (_feed, mut session) = started(&rig, &output);
            advance(&rig.local(), 50).await;
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l");
            session
                .terminal
                .start(session.inputs.callback(), session.resizes.callback())
                .unwrap();
            assert_eq!(output.take(), START_BYTES);
            advance(&rig.local(), 100).await;
            assert_eq!(output.take(), "");
            advance(&rig.local(), 50).await;
            assert_eq!(output.take(), "\x1b[>4;2m");
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_eof_keeps_pending_buffer_and_negotiation_deadlines() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pty_input();
        let output = pty_output();
        output.resize(80, 24);
        rig.run(async {
            let pipe = pipe_input();
            let mut session = start(&rig);
            output.take();
            pipe.send(b"\x1b[");
            settle(&rig.local()).await;
            pipe.close();
            settle(&rig.local()).await;
            advance(&rig.local(), 10).await;
            assert_eq!(session.inputs.all(), ["\x1b["]);
            output.resize(90, 30);
            window_change();
            until(&rig.local(), || session.resizes.count() == 1).await;
            advance(&rig.local(), 140).await;
            assert_eq!(output.take(), "\x1b[>4;2m");
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l\x1b[>4;0m");
        });
    });
}

/// Runs `drain` beside `script`, which can check `done` to see whether the drain has ended.
async fn drain_beside<F: std::future::Future>(
    drain: impl std::future::Future<Output = std::io::Result<()>>,
    done: &Cell<bool>,
    script: F,
) {
    let finishing = async {
        drain.await.unwrap();
        done.set(true);
    };
    tokio::join!(finishing, script);
}

#[test]
fn native_drain_uses_default_idle_exit_and_restores_handler() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            feed.send(b"\x1b[?1u");
            settle(&rig.local()).await;
            assert_eq!(output.take(), "\x1b[>7u");
            let (done, local) = (Cell::new(false), rig.local());
            let drain = session.terminal.drain_input(None, None);
            assert_eq!(output.take(), "\x1b[<u");
            drain_beside(drain, &done, async {
                advance(&local, 49).await;
                assert!(!done.get());
                advance(&local, 1).await;
                assert!(done.get());
            })
            .await;
            assert!(session.inputs.all().is_empty());
            feed.send(b"z");
            settle(&rig.local()).await;
            assert_eq!(session.inputs.all(), ["z"]);
            assert!(!session.terminal.kitty_protocol_active());
        });
    });
}

/// Drains with a 120 ms limit and 50 ms idle exit while input keeps arriving.
fn busy_drain_with_explicit_limits() {
    let rig = Rig::paused();
    let output = pipe_output();
    rig.run(async {
        let (feed, mut session) = started(&rig, &output);
        let local = rig.local();
        advance(&local, 150).await;
        assert_eq!(output.take(), "\x1b[>4;2m");
        let done = Cell::new(false);
        let (max, idle) = (Duration::from_millis(120), Duration::from_millis(50));
        let drain = session.terminal.drain_input(Some(max), Some(idle));
        assert_eq!(output.take(), "\x1b[>4;0m");
        drain_beside(drain, &done, async {
            advance(&local, 40).await;
            feed.send(b"a");
            settle(&local).await;
            advance(&local, 10).await;
            advance(&local, 30).await;
            feed.send(b"b");
            settle(&local).await;
            advance(&local, 20).await;
            advance(&local, 19).await;
            assert!(!done.get());
            advance(&local, 1).await;
            assert!(done.get());
        })
        .await;
        assert!(session.inputs.all().is_empty());
        feed.send(b"z");
        settle(&local).await;
        assert_eq!(session.inputs.all(), ["z"]);
        session.terminal.stop().unwrap();
    });
}

/// Advances 10 ms at a time for a second, sending input every 40 ms, and checks that the
/// drain ends exactly at the last step.
async fn input_every_forty_milliseconds(local: &LocalSet, feed: &Feed, done: &Cell<bool>) {
    for step in 1..=100 {
        advance(local, 10).await;
        if step % 4 == 0 && step < 100 {
            feed.send(b"x");
            settle(local).await;
        }
        assert_eq!(done.get(), step == 100, "step {step}");
    }
}

/// Drains with the default limits while input arrives every 40 ms.
fn busy_drain_with_default_limits() {
    let rig = Rig::paused();
    let output = pipe_output();
    rig.run(async {
        let (feed, mut session) = started(&rig, &output);
        let (local, done) = (rig.local(), Cell::new(false));
        let drain = session.terminal.drain_input(None, None);
        let script = input_every_forty_milliseconds(&local, &feed, &done);
        drain_beside(drain, &done, script).await;
        assert!(session.inputs.all().is_empty());
        session.terminal.stop().unwrap();
    });
}

#[test]
fn native_drain_stops_at_maximum_with_continuing_input() {
    isolated!(0, &[], busy_drain_with_explicit_limits);
    isolated!(1, &[], busy_drain_with_default_limits);
}

#[test]
fn native_drain_checks_idle_on_its_wait_cadence() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            let (local, done) = (rig.local(), Cell::new(false));
            let (max, idle) = (Duration::from_millis(1000), Duration::from_millis(50));
            let drain = session.terminal.drain_input(Some(max), Some(idle));
            drain_beside(drain, &done, async {
                advance(&local, 49).await;
                feed.send(b"x");
                settle(&local).await;
                advance(&local, 1).await;
                advance(&local, 49).await;
                assert!(
                    !done.get(),
                    "idle is checked every wait, not at the earliest deadline"
                );
                advance(&local, 1).await;
                assert!(done.get());
            })
            .await;
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_drain_zero_limit_restores_immediately() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            feed.send(b"\x1b[?1u");
            settle(&rig.local()).await;
            output.take();
            let (max, idle) = (Duration::ZERO, Duration::from_millis(50));
            session
                .terminal
                .drain_input(Some(max), Some(idle))
                .await
                .unwrap();
            assert_eq!(output.take(), "\x1b[<u");
            feed.send(b"z");
            settle(&rig.local()).await;
            assert_eq!(session.inputs.all(), ["z"]);
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_drain_zero_idle_restores_immediately() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            let (local, done) = (rig.local(), Cell::new(false));
            let (max, idle) = (Duration::from_millis(1000), Duration::ZERO);
            session
                .terminal
                .drain_input(Some(max), Some(idle))
                .await
                .unwrap();
            feed.send(b"z");
            settle(&local).await;
            assert_eq!(session.inputs.all(), ["z"]);
            let (max, idle) = (Duration::from_millis(10), Duration::from_millis(50));
            let drain = session.terminal.drain_input(Some(max), Some(idle));
            drain_beside(drain, &done, async {
                advance(&local, 9).await;
                assert!(!done.get());
                advance(&local, 1).await;
                assert!(done.get());
            })
            .await;
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_drain_cannot_reenable_keyboard_modes() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            let (local, done) = (rig.local(), Cell::new(false));
            let (max, idle) = (Duration::from_millis(1000), Duration::from_millis(200));
            let drain = session.terminal.drain_input(Some(max), Some(idle));
            drain_beside(drain, &done, async {
                advance(&local, 150).await;
                feed.send(b"\x1b[?1u");
                settle(&local).await;
                advance(&local, 50).await;
                advance(&local, 199).await;
                assert!(!done.get());
                advance(&local, 1).await;
            })
            .await;
            assert_eq!(output.take(), "");
            assert!(session.inputs.all().is_empty());
            feed.send(b"\x1b[?1u");
            advance(&local, 1000).await;
            assert!(!session.terminal.kitty_protocol_active());
            assert_eq!((output.take(), session.inputs.count()), (String::new(), 0));
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l");
        });
    });
}

#[test]
fn native_cancelled_drain_restores_handler() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            let local = rig.local();
            feed.send(b"\x1b[?1u");
            settle(&local).await;
            output.take();
            let drain = session.terminal.drain_input(None, None);
            assert_eq!(output.take(), "\x1b[<u");
            tokio::select! {
                biased;
                _ = drain => unreachable!("the drain outlasts ten milliseconds"),
                () = advance(&local, 10) => {}
            }
            feed.send(b"z");
            settle(&local).await;
            assert_eq!(session.inputs.all(), ["z"]);
            session.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b[?2004l");
        });
    });
}

#[test]
fn native_drain_counts_decoded_input_activity() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let (feed, mut session) = started(&rig, &output);
            let (local, done) = (rig.local(), Cell::new(false));
            let drain = session.terminal.drain_input(None, None);
            drain_beside(drain, &done, async {
                advance(&local, 40).await;
                feed.send(&[240, 159]);
                settle(&local).await;
                advance(&local, 9).await;
                assert!(!done.get());
                advance(&local, 1).await;
                assert!(
                    done.get(),
                    "the half character does not restart the idle wait"
                );
            })
            .await;
            let drain = session.terminal.drain_input(None, None);
            done.set(false);
            drain_beside(drain, &done, async {
                advance(&local, 40).await;
                feed.send(&[142, 137]);
                settle(&local).await;
                advance(&local, 10).await;
                advance(&local, 49).await;
                assert!(
                    !done.get(),
                    "the completed character restarts the idle wait"
                );
                advance(&local, 1).await;
                assert!(done.get());
            })
            .await;
            assert!(session.inputs.all().is_empty());
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_callbacks_can_reenter_output_and_stop() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let feed = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let terminal = Rc::new(RefCell::new(ProcessTerminal::new(rig.local())));
            let seen = Rc::new(RefCell::new(Vec::new()));
            let (reentrant, log) = (Rc::clone(&terminal), Rc::clone(&seen));
            let on_input = move |chunk: &str| {
                log.borrow_mut().push(chunk.to_owned());
                reentrant.borrow_mut().write("reentered").unwrap();
                reentrant.borrow_mut().stop().unwrap();
            };
            terminal
                .borrow_mut()
                .start(Box::new(on_input), Box::new(|| {}))
                .unwrap();
            assert_eq!(output.take(), START_BYTES);
            feed.send(b"ab");
            settle(&rig.local()).await;
            assert_eq!(*seen.borrow(), ["a"]);
            assert_eq!(output.take(), "reentered\x1b[?2004l");
            assert_eq!(Rc::strong_count(&terminal), 1);
        });
    });
}

#[test]
fn native_idle_reader_cancellation_needs_no_input() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _feed = pipe_input();
        let _output = pipe_output();
        rig.run(async {
            let local = rig.local();
            start(&rig).terminal.stop().unwrap();
            settle(&local).await;
            let baseline = open_descriptors();
            let mut session = start(&rig);
            settle(&local).await;
            assert!(
                open_descriptors() > baseline,
                "the reader holds a descriptor"
            );
            session.terminal.stop().unwrap();
            settle(&local).await;
            assert_eq!(open_descriptors(), baseline);
        });
    });
}

#[test]
fn native_reader_uses_buffer_deadline_on_a_real_pty() {
    isolated!(&[], || {
        let rig = Rig::real();
        let feed = pty_input();
        let _output = pipe_output();
        rig.run(async {
            let mut session = start(&rig);
            feed.send(b"\x1b[");
            until(&rig.local(), || session.inputs.count() == 1).await;
            assert_eq!(session.inputs.all(), ["\x1b["]);
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_resize_tracks_stdout_not_the_controlling_terminal() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let input = pty_input();
        let output = pty_output();
        input.resize(50, 10);
        output.resize(120, 40);
        rig.run(async {
            let mut session = start(&rig);
            assert_eq!(
                (session.terminal.columns(), session.terminal.rows()),
                (120, 40)
            );
            output.resize(130, 45);
            input.resize(70, 20);
            window_change();
            until(&rig.local(), || session.resizes.count() == 1).await;
            assert_eq!(
                (session.terminal.columns(), session.terminal.rows()),
                (130, 45)
            );
            let _redirected = pipe_output();
            window_change();
            settle(&rig.local()).await;
            assert_eq!(
                (session.terminal.columns(), session.terminal.rows()),
                (130, 45)
            );
            assert_eq!(session.resizes.count(), 1);
            session.terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_raw_mode_restores_the_actual_stdin_device() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pty_input();
        let _output = pty_output();
        let untouched = format!("{:?}", attributes(stdout()));
        let (before, flags) = (stdin_attributes(), stdin_flags());
        rig.run(async {
            let mut session = start(&rig);
            assert_ne!(stdin_attributes(), before);
            assert_eq!(format!("{:?}", attributes(stdout())), untouched);
            session.terminal.stop().unwrap();
        });
        assert_eq!((stdin_attributes(), stdin_flags()), (before, flags));
        assert_eq!(format!("{:?}", attributes(stdout())), untouched);
    });
}

#[test]
fn native_failed_start_and_stop_release_owned_resources() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pty_input();
        let (before, flags) = (stdin_attributes(), stdin_flags());
        let mut orphan = ProcessTerminal::new(rig.local());
        let failure = orphan.start(Box::new(|_| {}), Box::new(|| {})).unwrap_err();
        assert_eq!(failure.kind(), std::io::ErrorKind::Other);
        assert_eq!((stdin_attributes(), stdin_flags()), (before.clone(), flags));
        rig.run(async {
            let local = rig.local();
            let warm = pipe_output();
            start(&rig).terminal.stop().unwrap();
            settle(&local).await;
            drop(warm);
            drop(pipe_output());
            let baseline = open_descriptors();
            let mut terminal = ProcessTerminal::new(local.clone());
            let failure = terminal
                .start(Box::new(|_| {}), Box::new(|| {}))
                .unwrap_err();
            assert_eq!(failure.kind(), std::io::ErrorKind::BrokenPipe);
            settle(&local).await;
            assert_eq!((stdin_attributes(), stdin_flags()), (before.clone(), flags));
            assert_eq!(open_descriptors(), baseline);
            assert!(!terminal.kitty_protocol_active());

            let output = pipe_output();
            let baseline = open_descriptors();
            let mut session = start(&rig);
            drop(output);
            let failure = session.terminal.stop().unwrap_err();
            assert_eq!(failure.kind(), std::io::ErrorKind::BrokenPipe);
            settle(&local).await;
            assert_eq!((stdin_attributes(), stdin_flags()), (before, flags));
            assert_eq!(
                open_descriptors(),
                baseline - 1,
                "only the closed capture is gone"
            );
        });
    });
}

#[test]
fn native_background_io_failure_is_reported_by_stop() {
    isolated!(0, &[], || {
        let rig = Rig::paused();
        directory_input();
        let _output = pipe_output();
        rig.run(async {
            let mut session = start(&rig);
            settle(&rig.local()).await;
            let failure = session.terminal.stop().unwrap_err();
            assert_eq!(failure.raw_os_error(), Some(Errno::ISDIR.raw_os_error()));
            session.terminal.stop().unwrap();
        });
    });
    isolated!(1, &[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal.set_progress(true).unwrap();
            drop(output);
            advance(&rig.local(), 1000).await;
            let failure = terminal.stop().unwrap_err();
            assert_eq!(failure.kind(), std::io::ErrorKind::BrokenPipe);
        });
    });
}

#[test]
fn native_dropped_terminal_releases_only_owned_active_resources() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pty_input();
        let output = pipe_output();
        let (before, flags) = (stdin_attributes(), stdin_flags());
        rig.run(async {
            drop(ProcessTerminal::new(rig.local()));
            assert_eq!(output.take(), "");
            let session = start(&rig);
            output.take();
            drop(session);
            assert_eq!(output.take(), "\x1b[?2004l");
            assert_eq!((stdin_attributes(), stdin_flags()), (before, flags));
            let mut session = start(&rig);
            session.terminal.stop().unwrap();
            output.take();
            drop(session);
            assert_eq!(output.take(), "");
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal.set_progress(true).unwrap();
            output.take();
            drop(terminal);
            assert_eq!(output.take(), "\x1b]9;4;0;\x07");
            advance(&rig.local(), 3000).await;
            assert_eq!(output.take(), "");
        });
    });
}

#[test]
fn native_restarting_active_terminal_replaces_resources_once() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let feed = pty_input();
        let output = pipe_output();
        let (before, flags) = (stdin_attributes(), stdin_flags());
        rig.run(async {
            let local = rig.local();
            let mut first = start(&rig);
            first.terminal.set_progress(true).unwrap();
            settle(&local).await;
            let one_reader = open_descriptors();
            assert_eq!(output.take(), format!("{START_BYTES}\x1b]9;4;3\x07"));
            let second = (Inputs::default(), Resizes::default());
            first
                .terminal
                .start(second.0.callback(), second.1.callback())
                .unwrap();
            settle(&local).await;
            assert_eq!(output.take(), START_BYTES);
            assert_eq!(open_descriptors(), one_reader);
            feed.send(b"a");
            settle(&local).await;
            assert_eq!(
                (first.inputs.count(), second.0.all()),
                (0, vec!["a".to_owned()])
            );
            advance(&local, 1000).await;
            assert_eq!(output.take(), "\x1b[>4;2m\x1b]9;4;3\x07");
            first.terminal.stop().unwrap();
            assert_eq!(output.take(), "\x1b]9;4;0;\x07\x1b[?2004l\x1b[>4;0m");
            assert_eq!((stdin_attributes(), stdin_flags()), (before, flags));
        });
    });
}
