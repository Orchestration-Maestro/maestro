//! Starting and stopping: which standard-input state is acquired and given back, and in
//! which order.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use maestro_tui::set_kitty_protocol_active;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::stdio::stdin;
use rustix::termios::{OptionalActions, OutputModes, Termios, tcgetattr, tcsetattr};
use tokio::runtime::Handle;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

use super::input::{Input, Source};
use super::output::{
    FALLBACK_OFF, FALLBACK_ON, KEYBOARD_ON, KEYBOARD_POP, KEYBOARD_QUERY, PASTE_OFF, PASTE_ON,
    PROGRESS_CLEAR, emit, emit_if, first_failure,
};
use super::{InputCallback, ProcessTerminal, ResizeCallback, Shared};

/// How long after `start` a missing keyboard reply selects the fallback reporting mode.
const NEGOTIATION: Duration = Duration::from_millis(150);

/// How long a drain lasts at most when the caller names no limit.
const DRAIN_LIMIT: Duration = Duration::from_millis(1000);

/// How long without input ends a drain when the caller names no idle time.
const DRAIN_IDLE: Duration = Duration::from_millis(50);

/// The live input task and the standard-input state it changed.
pub(super) struct Running {
    /// The task reading input and delivering it.
    pub(super) task: JoinHandle<()>,
    /// What to give back to standard input when the terminal stops.
    pub(super) acquired: Acquired,
}

/// The standard-input state a started terminal changed and must give back.
#[derive(Default)]
pub(super) struct Acquired {
    /// The attributes before raw mode, when standard input is a terminal.
    attributes: Option<Termios>,
    /// The status flags before reads became nonblocking.
    flags: Option<OFlags>,
}

impl Acquired {
    /// Puts standard input in raw mode, when it is a terminal, and makes its reads
    /// nonblocking. What is already remembered is kept, so repeating the call never records
    /// the changed state as the original.
    fn take(&mut self) -> io::Result<()> {
        if self.attributes.is_none()
            && let Ok(original) = tcgetattr(stdin())
        {
            tcsetattr(stdin(), OptionalActions::Drain, &raw(&original))?;
            self.attributes = Some(original);
        }
        if self.flags.is_none() {
            let flags = fcntl_getfl(stdin())?;
            fcntl_setfl(stdin(), flags | OFlags::NONBLOCK)?;
            self.flags = Some(flags);
        }
        Ok(())
    }

    /// Gives back what was taken, attempting the flags even when the attributes fail, and
    /// returns the first failure.
    pub(super) fn restore(&mut self) -> io::Result<()> {
        let attributes = self.attributes.take().map_or(Ok(()), |original| {
            tcsetattr(stdin(), OptionalActions::Drain, &original).map_err(io::Error::from)
        });
        let flags = self.flags.take().map_or(Ok(()), |original| {
            fcntl_setfl(stdin(), original).map_err(io::Error::from)
        });
        attributes.and(flags)
    }
}

impl Drop for Acquired {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// `original` in raw mode: no echo, line editing, signal keys, break, parity, strip, newline,
/// carriage-return or start/stop-key handling on input, reads that return as soon as one byte
/// is available, and output post-processing with newline-to-CR-LF mapping on so a bare
/// newline written to the device returns the carriage.
fn raw(original: &Termios) -> Termios {
    let mut raw = original.clone();
    raw.make_raw();
    raw.output_modes
        .insert(OutputModes::OPOST | OutputModes::ONLCR);
    raw
}

impl ProcessTerminal {
    /// Retires the live input task, if any, and returns the standard-input state it held.
    fn retire(&mut self) -> Acquired {
        self.shared.retire();
        self.running
            .take()
            .map_or_else(Acquired::default, |running| {
                running.task.abort();
                running.acquired
            })
    }

    /// Starts an input generation, replacing the live one and keeping the standard-input state
    /// the live one acquired. Outside a runtime it fails first and changes nothing. Any later
    /// failure ends the acquisition: the terminal is left stopped with standard input
    /// restored, after the paste and keyboard modes a replaced generation had enabled are
    /// written off, so the next start saves standard input afresh.
    pub(super) fn begin(
        &mut self,
        on_input: InputCallback,
        on_resize: ResizeCallback,
    ) -> io::Result<()> {
        Handle::try_current().map_err(io::Error::other)?;
        let replaced = self.running.is_some();
        let mut acquired = self.retire();
        match self.spawn_input(&mut acquired, on_input, on_resize) {
            Ok(task) => {
                self.running = Some(Running { task, acquired });
                Ok(())
            }
            Err(error) => {
                if replaced {
                    let _ = self.give_back(&mut acquired);
                }
                Err(error)
            }
        }
    }

    /// Acquires standard input, opens the reader, announces the modes and spawns the input
    /// task of a new generation.
    fn spawn_input(
        &self,
        acquired: &mut Acquired,
        on_input: InputCallback,
        on_resize: ResizeCallback,
    ) -> io::Result<JoinHandle<()>> {
        acquired.take()?;
        let source = Source::open()?;
        let resize = signal(SignalKind::window_change())?;
        emit(&[PASTE_ON, KEYBOARD_QUERY].concat())?;
        let generation = self.shared.install(on_input, on_resize);
        let input = Input::new(
            Rc::clone(&self.shared),
            generation,
            Instant::now() + NEGOTIATION,
        );
        Ok(self.local.spawn_local(input.run(source, resize)))
    }

    /// Writes the paste and keyboard disables and gives standard input back; every step is
    /// attempted and the first failure is returned.
    fn give_back(&self, acquired: &mut Acquired) -> io::Result<()> {
        first_failure([
            emit(PASTE_OFF),
            self.shared.disable_keyboard(),
            acquired.restore(),
        ])
    }

    /// Stops the terminal. Every step is attempted; the first failure, a retained background
    /// failure included, is returned.
    pub(super) fn end(&mut self) -> io::Result<()> {
        let mut acquired = self.retire();
        let progress = emit_if(self.cancel_keepalive(), PROGRESS_CLEAR);
        first_failure([
            self.shared.failure.take().map_or(Ok(()), Err),
            progress,
            self.give_back(&mut acquired),
        ])
    }
}

impl Shared {
    /// Enables the enhanced keyboard protocol after the terminal reported support.
    pub(super) fn enable_enhanced(&self) -> io::Result<()> {
        emit(KEYBOARD_ON)?;
        self.enhanced.set(true);
        set_kitty_protocol_active(true);
        Ok(())
    }

    /// Enables the modified-key reporting when negotiation is open and no keyboard mode is on.
    pub(super) fn fall_back(&self) -> io::Result<()> {
        if !self.negotiating.get() || self.enhanced.get() || self.fallback.get() {
            return Ok(());
        }
        emit(FALLBACK_ON)?;
        self.fallback.set(true);
        Ok(())
    }

    /// Disables the keyboard modes this terminal enabled, the enhanced protocol first, and
    /// attempts both even when the first write fails.
    pub(super) fn disable_keyboard(&self) -> io::Result<()> {
        let (enhanced, fallback) = (self.enhanced.replace(false), self.fallback.replace(false));
        if enhanced {
            set_kitty_protocol_active(false);
        }
        first_failure([
            emit_if(enhanced, KEYBOARD_POP),
            emit_if(fallback, FALLBACK_OFF),
        ])
    }

    /// Interprets `sequence` as a support reply. Returns whether it was one and so is not
    /// input: a reply is consumed while the enhanced protocol is off, and enables it only
    /// while negotiation is open.
    pub(super) fn consume_reply(&self, sequence: &str) -> io::Result<bool> {
        if self.enhanced.get() || !is_support_reply(sequence) {
            return Ok(false);
        }
        if self.negotiating.get() {
            self.enable_enhanced()?;
        }
        Ok(true)
    }
}

/// Whether `sequence` is exactly `ESC [ ?`, one or more ASCII digits and `u`.
fn is_support_reply(sequence: &str) -> bool {
    sequence
        .strip_prefix("\x1b[?")
        .and_then(|rest| rest.strip_suffix('u'))
        .is_some_and(|flags| !flags.is_empty() && flags.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The input callback taken out of service for a drain, put back when the drain ends or its
/// future is dropped, provided its generation is still the live one.
struct Suspended {
    /// The state the callback returns to.
    shared: Rc<Shared>,
    /// The generation the callback belongs to.
    generation: u64,
    /// The callback, until it is put back.
    callback: Option<InputCallback>,
}

impl Drop for Suspended {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take()
            && self.shared.is_current(self.generation)
        {
            self.shared.on_input.replace(Some(callback));
        }
    }
}

impl ProcessTerminal {
    /// Stops delivering input and disables the keyboard modes now, then waits until no decoded
    /// text has arrived for `idle` or `max` has passed, and delivers input again. Dropping the
    /// returned future early delivers input again at once. Keyboard replies received from
    /// now on never enable a mode for this generation. Framing state is not reset: an
    /// unfinished character or paste stays pending however long the drain lasts, and an
    /// unfinished escape sequence is released at its buffer deadline, which runs during the
    /// drain, so it is discarded when the drain outlasts it and otherwise stays pending for
    /// later input or its deadline.
    pub(super) fn drain(
        &mut self,
        max: Option<Duration>,
        idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        let begin = Instant::now();
        self.shared.activity.set(begin);
        self.shared.negotiating.set(false);
        let disabled = self.shared.disable_keyboard();
        let suspended = Suspended {
            shared: Rc::clone(&self.shared),
            generation: self.shared.generation.get(),
            callback: self.shared.on_input.take(),
        };
        let shared = Rc::clone(&self.shared);
        let (max, idle) = (max.unwrap_or(DRAIN_LIMIT), idle.unwrap_or(DRAIN_IDLE));
        Box::pin(async move {
            let _suspended = suspended;
            disabled?;
            wait_until_quiet(&shared, begin, max, idle).await;
            Ok(())
        })
    }
}

/// Waits until `idle` has passed since the last input, or `max` since `begin`, checking after
/// each wait of the shorter of `idle` and the time left.
async fn wait_until_quiet(shared: &Shared, begin: Instant, max: Duration, idle: Duration) {
    let end = begin.checked_add(max);
    loop {
        let now = Instant::now();
        let left = end.map(|end| end.saturating_duration_since(now));
        if left == Some(Duration::ZERO) || now.duration_since(shared.activity.get()) >= idle {
            return;
        }
        sleep(left.map_or(idle, |left| left.min(idle))).await;
    }
}
