//! Windows input sources: console records become UTF-8 text and resize notices, while
//! redirected input keeps its bytes. Both feed the same process-wide reader.

use std::io::{self, Read};
use std::thread;
use std::time::Duration;

/// The interval between console queries when no input is waiting.
const POLL: Duration = Duration::from_millis(10);

/// The Alt key, whose release can carry a character composed on the numeric keypad.
const VK_MENU: u16 = 0x12;

/// Either control key in a console record's control-state mask.
const CTRL: u32 = 0x0004 | 0x0008;

/// One console input record, including its control-state metadata.
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
        /// The console's control-key state bitmask.
        control: u32,
    },
    /// The console screen buffer changed size.
    Resize,
    /// A mouse, focus or menu record.
    Other,
}

/// Where the console source reads records from.
pub(super) trait Records: Send + 'static {
    /// How many records are waiting.
    ///
    /// # Errors
    /// Returns the console's failure.
    fn pending(&self) -> io::Result<u32>;

    /// Reads records. Another reader can drain the count first, so this can block.
    ///
    /// # Errors
    /// Returns the console's failure.
    fn read(&self) -> io::Result<Vec<Record>>;
}

/// What a source delivers to the input task.
#[derive(Debug)]
pub(super) enum Arrival {
    /// The UTF-8 bytes of typed text, never empty.
    Bytes(Vec<u8>),
    /// The window size may have changed.
    Resize,
    /// The source failed and ended.
    Failed(io::Error),
}

impl Clone for Arrival {
    fn clone(&self) -> Self {
        match self {
            Self::Bytes(bytes) => Self::Bytes(bytes.clone()),
            Self::Resize => Self::Resize,
            Self::Failed(error) => Self::Failed(error.raw_os_error().map_or_else(
                || io::Error::new(error.kind(), error.to_string()),
                io::Error::from_raw_os_error,
            )),
        }
    }
}

/// A console source that preserves UTF-16 pairs across reads. The pending query avoids
/// blocking on an idle console, but cancellation never relies on its count staying valid.
pub(super) fn console(
    records: impl Records,
) -> impl FnMut() -> io::Result<Option<Vec<Arrival>>> + Send {
    let mut text = Text::default();
    move || {
        if records.pending()? == 0 {
            thread::sleep(POLL);
            return Ok(Some(Vec::new()));
        }
        Ok(Some(text.deliver(records.read()?)))
    }
}

/// A redirected byte stream. Only the reader thread calls its blocking read.
pub(super) fn pipe(
    mut input: impl Read + Send + 'static,
) -> impl FnMut() -> io::Result<Option<Vec<Arrival>>> + Send {
    move || {
        let mut bytes = [0; 4096];
        let count = input.read(&mut bytes)?;
        Ok((count != 0).then(|| vec![Arrival::Bytes(bytes[..count].to_vec())]))
    }
}

/// Typed UTF-16 units held so a surrogate pair split across reads is joined.
#[derive(Default)]
struct Text(Vec<u16>);

impl Text {
    /// Translates text and resize notices in arrival order.
    fn deliver(&mut self, records: Vec<Record>) -> Vec<Arrival> {
        let mut arrivals = Vec::new();
        for record in records {
            match record {
                Record::Key {
                    down,
                    key,
                    unit,
                    repeat,
                    control,
                } if (down || key == VK_MENU)
                    && (unit != 0 || (control & CTRL != 0 && matches!(key, 0x20 | 0x32))) =>
                {
                    self.0
                        .extend(std::iter::repeat_n(unit, usize::from(repeat.max(1))));
                }
                Record::Resize => {
                    self.flush(&mut arrivals);
                    arrivals.push(Arrival::Resize);
                }
                Record::Key { .. } | Record::Other => {}
            }
        }
        self.flush(&mut arrivals);
        arrivals
    }

    /// Emits UTF-8, holding a trailing high surrogate; an unpaired surrogate becomes U+FFFD.
    fn flush(&mut self, arrivals: &mut Vec<Arrival>) {
        let held = self
            .0
            .pop_if(|unit| (0xd800..0xdc00).contains(unit))
            .map(|unit| vec![unit]);
        let units = std::mem::replace(&mut self.0, held.unwrap_or_default());
        let text: String = char::decode_utf16(units)
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        if !text.is_empty() {
            arrivals.push(Arrival::Bytes(text.into_bytes()));
        }
    }
}

#[cfg(test)]
mod tests;
