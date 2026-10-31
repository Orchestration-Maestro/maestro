//! Starting and stopping: which standard-input state is acquired and given back, and in
//! which order.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use maestro_tui::set_kitty_protocol_active;
use tokio::runtime::Handle;
use tokio::time::{Instant, sleep};

use super::input::Input;
use super::output::{
    CURSOR_SHOW, FALLBACK_OFF, FALLBACK_ON, KEYBOARD_POP, KEYBOARD_PUSH, KEYBOARD_QUERY, PASTE_OFF,
    PASTE_ON, PROGRESS_CLEAR, first_failure,
};
use super::platform::{self, Acquired, Task};
use super::{InputCallback, OutputPort, ProcessTerminal, ResizeCallback, Shared};

/// How long a drain lasts at most when the caller names no limit.
const DRAIN_LIMIT: Duration = Duration::from_millis(1000);

/// How long without input ends a drain when the caller names no idle time.
const DRAIN_IDLE: Duration = Duration::from_millis(50);

/// The live input task and the standard-input state it changed.
pub(super) struct Running {
    /// The task reading input and delivering it.
    pub(super) task: Task,
    /// What to give back to standard input when the terminal stops.
    pub(super) acquired: Acquired,
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
                let _ = self.shared.finish_output();
                if replaced {
                    let _ = self.give_back(&mut acquired);
                }
                Err(error)
            }
        }
    }

    /// Acquires standard input, opens the reader, announces the modes and spawns the input
    /// task of a new generation. The keyboard flags are requested once: a restarted terminal
    /// that still holds them only asks again.
    fn spawn_input(
        &self,
        acquired: &mut Acquired,
        on_input: InputCallback,
        on_resize: ResizeCallback,
    ) -> io::Result<Task> {
        let stream = platform::acquire(acquired)?;
        self.shared
            .output
            .open(&self.local, Rc::downgrade(&self.shared))?;
        let push = if self.shared.pushed.get() {
            ""
        } else {
            KEYBOARD_PUSH
        };
        self.shared
            .emit(&[PASTE_ON, push, KEYBOARD_QUERY].concat())?;
        self.shared.pushed.set(true);
        let generation = self.shared.install(on_input, on_resize);
        Ok(stream.spawn(&self.local, Input::new(Rc::clone(&self.shared), generation)))
    }

    /// Writes the paste and keyboard disables and gives standard input back; every step is
    /// attempted and the first failure is returned.
    fn give_back(&self, acquired: &mut Acquired) -> io::Result<()> {
        first_failure([
            self.shared.emit(PASTE_OFF),
            self.shared.disable_keyboard(),
            acquired.restore(),
        ])
    }

    /// Stops the terminal. Every step is attempted; the first failure, a retained background
    /// failure included, is returned.
    pub(super) fn end(&mut self) -> io::Result<()> {
        let mut acquired = self.retire();
        let output = self.shared.finish_output();
        let cursor = self.shared.show_hidden_cursor();
        let active = self.cancel_keepalive();
        let progress = self.shared.emit_if(active, PROGRESS_CLEAR);
        first_failure([
            self.shared.failure.take().map_or(Ok(()), Err),
            output,
            cursor,
            progress,
            self.give_back(&mut acquired),
        ])
    }
}

/// What a keyboard reply says about the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reply {
    /// The terminal runs enhanced keyboard flags.
    Enhanced,
    /// The terminal runs none: it reported zero flags, or only its device attributes.
    Plain,
}

impl Reply {
    /// The reply `sequence` is, or `None` when it is something else. A reply is exactly
    /// `ESC [ ?`, one or more ASCII digits and `u` for the flags, or `ESC [ ?`, any ASCII
    /// digits and semicolons and `c` for the device attributes.
    pub(super) fn parse(sequence: &str) -> Option<Self> {
        let rest = sequence.strip_prefix("\x1b[?")?;
        rest.strip_suffix('u')
            .and_then(Self::from_flags)
            .or_else(|| rest.strip_suffix('c').and_then(Self::from_attributes))
    }

    /// The reply for the `digits` of a flags report, or `None` unless they are one or more
    /// ASCII digits.
    fn from_flags(digits: &str) -> Option<Self> {
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some(if digits.bytes().all(|byte| byte == b'0') {
            Self::Plain
        } else {
            Self::Enhanced
        })
    }

    /// The reply for the parameters of a device-attributes report, or `None` unless they are
    /// ASCII digits and semicolons.
    fn from_attributes(parameters: &str) -> Option<Self> {
        parameters
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b';')
            .then_some(Self::Plain)
    }

    /// Whether `sequence` could be the start of a reply: `ESC [`, or `ESC [ ?` followed by any
    /// ASCII digits and semicolons.
    pub(super) fn is_prefix(sequence: &str) -> bool {
        sequence == "\x1b["
            || sequence.strip_prefix("\x1b[?").is_some_and(|rest| {
                rest.bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
            })
    }
}

impl Shared {
    /// Acts on a reply read by the live generation: the enhanced protocol turns the modified-key
    /// reporting off and the toolkit's shared flag on, and a plain terminal gets the modified-key
    /// reporting unless the enhanced protocol is on. A drain closes negotiation, so a later
    /// reply changes nothing.
    pub(super) fn answer(&self, reply: Reply) -> io::Result<()> {
        if !self.negotiating.get() {
            return Ok(());
        }
        match reply {
            Reply::Enhanced => {
                self.stop_fallback()?;
                if !self.enhanced.replace(true) {
                    set_kitty_protocol_active(true);
                }
                Ok(())
            }
            Reply::Plain => self.fall_back(),
        }
    }

    /// Enables the modified-key reporting unless a keyboard mode is on.
    fn fall_back(&self) -> io::Result<()> {
        if self.enhanced.get() || self.fallback.get() {
            return Ok(());
        }
        self.emit(FALLBACK_ON)?;
        self.fallback.set(true);
        Ok(())
    }

    /// Disables the modified-key reporting when it is on.
    fn stop_fallback(&self) -> io::Result<()> {
        self.emit_if(self.fallback.replace(false), FALLBACK_OFF)
    }

    /// Shows the cursor when this terminal hid it and nothing showed it since.
    pub(super) fn show_hidden_cursor(&self) -> io::Result<()> {
        self.emit_if(self.hidden.replace(false), CURSOR_SHOW)
    }

    /// Pops the requested keyboard flags and disables the modified-key reporting, whichever
    /// are on, and attempts both even when the first write fails.
    pub(super) fn disable_keyboard(&self) -> io::Result<()> {
        let (pushed, enhanced) = (self.pushed.replace(false), self.enhanced.replace(false));
        if enhanced {
            set_kitty_protocol_active(false);
        }
        first_failure([self.emit_if(pushed, KEYBOARD_POP), self.stop_fallback()])
    }
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
    /// now on never enable a mode for this generation. Input still goes to the shared buffer
    /// under the framing and release rules described in `docs/terminal/input.md`. Every event
    /// it releases during the drain is discarded, whatever released it. The drain does not
    /// reset framing state.
    pub(super) fn drain(
        &mut self,
        max: Option<Duration>,
        idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + 'static>> {
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
