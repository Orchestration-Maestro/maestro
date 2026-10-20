//! Feeds the same bytes to the workspace terminal as program output and to the
//! toolkit as keyboard input.
#![no_main]

use libfuzzer_sys::fuzz_target;
use maestro_terminal::Terminal;
use maestro_tui::{StdinBuffer, StdinBufferInput, StdinBufferOptions};
use std::hint::black_box;
use std::time::Instant;

/// The workspace daemon's default history limit in bytes.
const SCROLLBACK_LIMIT_BYTES: usize = 10_000_000;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut terminal) = Terminal::new(80, 24, SCROLLBACK_LIMIT_BYTES) {
        terminal.write(data);
    }
    let mut input = StdinBuffer::new(StdinBufferOptions::default());
    black_box(input.process(StdinBufferInput::Bytes(data), Instant::now()));
    black_box(input.flush());
});
