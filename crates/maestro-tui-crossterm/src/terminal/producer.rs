//! The thread that reads console input records for the Windows terminal and turns them into
//! UTF-8 bytes and window-size notices, in arrival order.
//!
//! The console offers no read that can be cancelled, so the thread asks how many records are
//! waiting and reads only when some are: a read never blocks, and stopping the thread is a flag
//! it checks before every read followed by a join.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

/// How long the thread sleeps when no record is waiting. It bounds the added input latency;
/// idle, the thread asks the console once per interval.
const POLL: Duration = Duration::from_millis(10);

/// The virtual-key code of the Alt key, whose release carries a character composed on the
/// numeric keypad.
const VK_MENU: u16 = 0x12;

/// One console input record, as far as the terminal reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Record {
    /// A key press or release.
    Key {
        /// Whether the key went down.
        down: bool,
        /// The virtual-key code.
        key: u16,
        /// The UTF-16 code unit the key produced, or zero.
        unit: u16,
        /// How many times the key repeated.
        repeat: u16,
    },
    /// The console screen buffer changed size.
    Resize,
    /// A record the terminal does not read: mouse, focus or menu.
    Other,
}

/// Where the thread reads records from.
pub(super) trait Records: Send + 'static {
    /// How many records are waiting.
    ///
    /// # Errors
    /// Returns the console's failure.
    fn pending(&self) -> io::Result<u32>;

    /// Reads the waiting records without blocking.
    ///
    /// # Errors
    /// Returns the console's failure.
    fn read(&self) -> io::Result<Vec<Record>>;
}

/// What the thread delivers to the input task.
#[derive(Debug)]
pub(super) enum Arrival {
    /// The UTF-8 bytes of typed text, never empty.
    Bytes(Vec<u8>),
    /// The window size may have changed.
    Resize,
    /// The console failed; the thread has ended.
    Failed(io::Error),
}

/// The running reader thread; dropping it stops and joins the thread.
pub(super) struct Producer {
    /// Asks the thread to stop before its next read.
    stop: Arc<AtomicBool>,
    /// The thread, until it is joined.
    thread: Option<JoinHandle<()>>,
}

impl Producer {
    /// Starts a thread that reads `records` and sends what they carry to `sender`.
    ///
    /// # Errors
    /// Returns the failure to create the thread.
    pub(super) fn spawn(
        records: impl Records,
        sender: UnboundedSender<Arrival>,
    ) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("console-input".into())
            .spawn(move || produce(&records, &flag, &sender))?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads until `stop` is set, the receiver is gone or the console fails, and sends the
/// failure.
fn produce(records: &impl Records, stop: &AtomicBool, sender: &UnboundedSender<Arrival>) {
    if let Err(error) = read_until_stopped(records, stop, sender) {
        let _ = send(sender, Arrival::Failed(error));
    }
}

/// Reads while records wait, until `stop` is set or the receiver is gone.
fn read_until_stopped(
    records: &impl Records,
    stop: &AtomicBool,
    sender: &UnboundedSender<Arrival>,
) -> io::Result<()> {
    let mut text = Text::default();
    while !stop.load(Ordering::Acquire) {
        if records.pending()? == 0 {
            thread::sleep(POLL);
            continue;
        }
        if stop.load(Ordering::Acquire) || text.deliver(records.read()?, sender).is_err() {
            break;
        }
    }
    Ok(())
}

/// Typed UTF-16 units not yet sent, so a surrogate pair split across reads is joined.
#[derive(Default)]
struct Text(Vec<u16>);

/// The receiver is gone.
struct Closed;

impl Text {
    /// Sends the text and resize notices `records` carry, in order.
    fn deliver(
        &mut self,
        records: Vec<Record>,
        sender: &UnboundedSender<Arrival>,
    ) -> Result<(), Closed> {
        for record in records {
            match record {
                Record::Key {
                    down,
                    key,
                    unit,
                    repeat,
                } if unit != 0 && (down || key == VK_MENU) => {
                    let count = usize::from(repeat.max(1));
                    self.0.extend(std::iter::repeat_n(unit, count));
                }
                Record::Resize => {
                    self.flush(sender)?;
                    send(sender, Arrival::Resize)?;
                }
                Record::Key { .. } | Record::Other => {}
            }
        }
        self.flush(sender)
    }

    /// Sends the held units as UTF-8, keeping a trailing high surrogate for its pair; an
    /// unpaired surrogate becomes U+FFFD.
    fn flush(&mut self, sender: &UnboundedSender<Arrival>) -> Result<(), Closed> {
        let held = self
            .0
            .pop_if(|unit| (0xd800..0xdc00).contains(unit))
            .map(|unit| vec![unit]);
        let units = std::mem::replace(&mut self.0, held.unwrap_or_default());
        let text: String = char::decode_utf16(units)
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        if text.is_empty() {
            return Ok(());
        }
        send(sender, Arrival::Bytes(text.into_bytes()))
    }
}

/// Sends `arrival`, or reports that the receiver is gone.
fn send(sender: &UnboundedSender<Arrival>, arrival: Arrival) -> Result<(), Closed> {
    sender.send(arrival).map_err(|_| Closed)
}

#[cfg(test)]
mod tests;
