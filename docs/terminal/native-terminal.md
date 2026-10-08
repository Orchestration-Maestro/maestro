# Native terminal

`ProcessTerminal` connects a Unix process to the toolkit. It implements the toolkit's
`Terminal` port over the process's own standard input and standard output, so it needs no
terminal framework and sends the toolkit raw protocol text rather than decoded key events.
The type exists on Unix only; on other targets the crate builds empty. It drives the
descriptors directly and never opens the controlling terminal, so redirecting one of the two
changes what it controls: raw mode and input come from standard input, window size and
output from standard output.

```rust,no_run
# #[cfg(unix)] {
use std::rc::Rc;

use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

let runtime = Builder::new_current_thread().enable_all().build()?;
let local = Rc::new(LocalSet::new());
runtime.block_on(local.run_until(async {
    let mut terminal = ProcessTerminal::new(Rc::clone(&local));
    terminal.start(Box::new(|input| println!("{input:?}")), Box::new(|| {}))?;
    // ... render and handle input until the application ends ...
    terminal.stop()
}))?;
# }
# Ok::<(), std::io::Error>(())
```

## The runtime it needs

The terminal starts no runtime and no thread. It is given a `LocalSet`, and every task it
creates runs there: the input reader, the progress keepalive and every callback. Tasks run
only while the caller drives that set inside a Tokio runtime whose I/O, time and signal
drivers are enabled; a runtime built without one of them is a caller error that Tokio reports
by panicking. `start` called outside a running runtime returns an error and acquires
nothing. Because callbacks never leave the thread that drives the set, they need not be
`Send`, and a callback may call back into the terminal to write or to stop it.

## Starting and stopping

`new` reads the size of standard output and the write-log setting and does nothing else.
`start` puts standard input in raw mode when it is a terminal, makes its reads nonblocking,
subscribes to window-change signals, writes the bracketed-paste enable and the keyboard
query, and spawns the input task. Raw mode keeps output post-processing so a bare newline
still returns the carriage. Standard input that is not a terminal (a pipe, a socket, a
regular file, `/dev/null`) skips raw mode and is still read as a stream of text.

If a step of `start` fails, the attributes and flags of standard input are restored and the
terminal is left stopped; the error is the operating system's, and output already written is
not retracted. Calling `start` on a started terminal replaces its input generation: the
earlier callbacks and reader are retired, the original standard-input state is kept for the
final `stop`, and active progress continues.

`stop` retires the input task before it writes anything, so no further callback starts, then
writes, in order: the progress clear when progress was active, the bracketed-paste disable,
the keyboard disables for the modes this terminal enabled (enhanced protocol first), and
finally restores the exact attributes and status flags standard input had when it was first
taken. Every step is attempted even when an earlier one fails, and `stop` returns the first
failure, including a failure retained from a background task. Stopping again writes the
paste disable again and changes nothing else. Dropping a started terminal does what `stop`
does and ignores its errors; dropping a terminal that is not started writes nothing, except
that dropping one with active progress clears it.

Restoration covers what the terminal itself changed. It does not undo a change another
party makes to standard input while the terminal is started.

## Input

Standard input is read as a stream of UTF-8. A character split across reads is held until it
completes, and invalid bytes become U+FFFD; only text that is complete reaches the toolkit's
`StdinBuffer`, which frames it into one event per character or escape sequence and one per
bracketed paste. Each event reaches the input callback in order; a paste arrives as one
chunk with its two markers put back. Malformed UTF-8 is never treated as a legacy
alt-modified byte. Incomplete escape sequences wait the buffer's deadline (10 ms) and are
then released as they are. At the end of input the decoder is flushed and reading stops, but
the buffer's deadline, the keyboard decision and window-size tracking continue until `stop`.
A read error ends the input task; the first such error is kept and returned by the next
`stop`.

## Keyboard protocols

`start` asks the terminal for its enhanced keyboard flags. A reply is exactly `ESC [ ?`, one
or more ASCII digits and `u`. The first reply enables the enhanced protocol (flags 7) and is
not passed on; `kitty_protocol_active` becomes true and the toolkit's shared flag is set. A
later reply while the protocol is active is ordinary input. If no reply arrives within
150 ms of `start`, the modified-key reporting mode is enabled instead; a reply that arrives
after that still enables the enhanced protocol, and both modes are then disabled at
`stop`. Replies that do not match the form are ordinary input.

## Draining

`drain_input(max, idle)` is for the end of a session, so key releases do not reach the
shell. It defaults to at most 1000 ms and 50 ms without input. When called, before the
returned future is polled, it disables the keyboard modes and stops delivering input; input
that arrives is discarded but still counts as activity. The future checks the elapsed
quiet time and the maximum, then waits the shorter of the idle time and the time left, so
the idle exit is noticed on that cadence. Only complete decoded text counts as activity, not
the bytes of an unfinished character. When the future ends, or is dropped, input delivery
resumes with the original callback. A drain also closes keyboard negotiation for the current
generation: no later reply and no pending fallback decision enables a mode, and a reply
that arrives is consumed instead of delivered.

## Window size

The size is cached from standard output when the terminal is created and refreshed when the
input task starts and on each window-change signal. The resize callback runs only when the
size differs from the cached one. A failed query leaves the cached size as it was. A
dimension the cache holds as zero, or never observed, falls back independently: to the
positive decimal in `COLUMNS` (width) or `LINES` (height), then to 80 by 24. The variable is
trimmed of ASCII whitespace and must then be nonempty ASCII digits naming a nonzero number
that fits `usize`; signs, fractions, exponents, radix prefixes and other text are ignored.

## Output

Every command writes all of its bytes to standard output before it returns, waiting on the
calling thread while the output is full: the status flag that makes reads nonblocking lives
on the open file description, which standard output usually shares with standard input. `move_by` writes nothing for zero. The title is written unchanged between
`ESC ] 0 ;` and a bell. `set_progress(true)` writes the indeterminate-progress sequence at
once and starts one keepalive that writes it again every 1000 ms from then on; asking again
writes it again but never adds or restarts the keepalive. `set_progress(false)` always
writes the clear sequence and stops the keepalive. A keepalive write that fails ends the
keepalive and is returned by the next `stop`.

## Write log

When `MAESTRO_TUI_WRITE_LOG` is set to a nonempty value, the data passed to `write` is also
appended to a file once standard output has accepted it. A value naming an existing
directory selects `tui-YYYY-MM-DD_HH-MM-SS-PID.log` in it, with the local time when the
terminal was created and the process id; any other value is the file path. Directories are
never created, and a failure to append is ignored. Cursor, title, protocol and progress
output is not logged.

## Errors

Operating-system errors are returned as `std::io::Error` from the call that met them; the
terminal prints no diagnostics of its own. Failures in background tasks cannot be returned
where they happen, so the first is kept and `stop` reports it.
