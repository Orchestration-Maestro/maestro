# Native terminal

`ProcessTerminal` connects a Unix process to the toolkit. It implements the toolkit's
`Terminal` port over the process's own standard input and standard output, so it needs no
terminal framework and sends the toolkit raw protocol text rather than decoded key events.
The type exists on Unix only; the native runtime host described [below](#the-runtime-host) exists
on every native target, and only browser targets build the crate empty. It drives the
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
drivers are enabled; a runtime built without one of them is a caller error, which Tokio
reports itself. `start` called outside a running runtime returns an `ErrorKind::Other` error
and changes nothing, even on a terminal that is already started. Because callbacks never
leave the thread that drives the set, they need not be `Send`, and a callback may call back
into the terminal to write or to stop it.

## The runtime host

`ProcessTuiRuntime` is the toolkit's `TuiRuntime` for native targets. It is built on the
same caller-driven `LocalSet` as the terminal, so one set can serve both, and it starts no
runtime, thread or task of its own. Work runs only while the caller drives the set inside a
Tokio runtime with the time driver enabled.

- `now` is the monotonic time since the host was created, on Tokio's clock.
- `schedule` measures its delay from the call and runs the callback once, never before the
  call returns, even for a zero delay. Dropping the returned handle leaves the callback
  scheduled; `cancel` prevents it, and what it captured is released once the set next
  processes the cancellation.
- `spawn_local` queues a future on the set; it is not polled before the call returns.
- A callback or future that returns an error has the `Display` text of the error and then of
  each source error written to standard error, each followed by a newline; the host then
  keeps running. A failure to write to standard error is ignored.
- `environment` returns the value as the platform holds it, with invalid text converted
  lossily; it does not trim or interpret the value.
- `log_context` reports the home directory (empty when the platform has none), the current
  UTC time in milliseconds and a random lowercase hexadecimal nonce.
- `append_log` and `write_log` act on the given path as the operating system resolves it.
  Append creates the file but not its directory; write creates the directory and replaces the
  file. Both return the operating system's errors.

Dropping the set drops the work still pending. The host's own tasks hold no handle to the
host; callers release any strong handle their callbacks capture.

## Starting and stopping

`new` reads the size of standard output and the write-log setting and does nothing else.
`start` puts standard input in raw mode when it is a terminal, makes its reads nonblocking,
subscribes to window-change signals, writes the bracketed-paste enable and the keyboard
query, and spawns the input task.

Raw mode is applied to the device standard input refers to. It clears echo, line editing,
signal keys, extended input processing, break handling, parity marking, stripping, newline
and carriage-return mapping and the start and stop keys, so bytes are delivered as they
become available, without line buffering. It also turns output post-processing and
newline-to-CR-LF on, so a bare newline written to that device returns the carriage. Standard
output's own device is not touched; when it is a different terminal, that guarantee does not
reach it. Standard input that is not a terminal (a pipe, a socket, a regular file,
`/dev/null`) skips raw mode and is still read as a stream of text.

If an operating-system step of `start` fails, the attributes and flags of standard input are
restored and the terminal is left stopped; the error is that call's, and output already
written is not retracted. Calling `start` on a started terminal replaces its input
generation: the earlier callbacks and reader are retired, the standard-input state saved by
the acquisition it replaces is kept for the final `stop`, and active progress continues. If
the replacing `start` fails (other than outside a runtime), the replaced generation's paste
and keyboard modes are written off as `stop` writes them, standard input is restored, the
acquisition ends, and progress is unchanged.

`stop` retires the input task before it writes anything, so no callback starts after it
returns (a callback that called `stop` finishes, and the events it had left are not
delivered). It then writes, in order: the progress clear when progress was active, the
bracketed-paste disable, and the keyboard disables for the modes this terminal enabled
(enhanced protocol first); finally it restores the attributes and status flags it saved, as
described below. Every step is attempted even when an earlier one fails, and `stop` returns
the first failure, a failure retained from a background task first. Stopping again writes
the paste disable again and changes nothing else. Dropping a started terminal does what
`stop` does and ignores its errors; dropping a terminal that is not started writes nothing,
except that dropping one with active progress clears it.

Restoration writes back the complete attributes and status flags saved by the acquisition
that `stop` ends. An acquisition begins with a `start` that finds the terminal stopped
(never started, stopped, or left stopped by a failed `start`). A replacing `start` that
succeeds keeps it; one that fails, other than outside a runtime, restores and ends it, so the
next `start` saves standard input as it is then. Because the saved state is written whole,
restoration also replaces any change another party made to standard input while the terminal
was started. Standard input that was not a terminal has no attributes to restore.

## Input

Standard input is read as a stream of UTF-8, at most 4096 bytes per read: a read takes what
is available at that moment, and a longer burst arrives in consecutive reads. A character
split across reads is held until it completes, and invalid bytes become U+FFFD; only text
that is complete reaches the toolkit's shared `StdinBuffer`. Its framing and release rules
are described in [Input](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/terminal/input.md). Each event it releases reaches the input callback in
order, except keyboard replies and the events of a drain, which are described below; a
paste has its two markers put back. Malformed UTF-8 is never treated as a legacy
alt-modified byte. At the end of input the decoder is flushed and reading stops, but the
buffer's deadline, the keyboard decision and window-size tracking continue until `stop`.
A read error ends the input task, and with it that tracking; the first such error is kept
and returned by the next `stop`.

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
returned future is polled, it disables the keyboard modes and stops delivering input.
Input still goes to the shared buffer under the [Input](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/terminal/input.md) framing and release rules.
While the drain runs, every event the buffer releases is discarded, whatever released it.
The drain does not reset framing state.

The future checks the elapsed quiet time and the maximum, then waits the shorter of the idle
time and the time left, so the idle exit is noticed on that cadence. Only complete decoded
text counts as activity, not the bytes of an unfinished character. When the future ends, or
is dropped, input delivery resumes with the original callback. A drain also closes keyboard
negotiation for the current generation: no later reply and no pending fallback decision
enables a mode, and a reply that arrives is consumed instead of delivered.

## Window size

The size is cached from standard output when the terminal is created and refreshed when the
input task starts and on each window-change signal. The resize callback runs only when the
size differs from the cached one. A failed query leaves the cached size as it was. A
dimension the cache holds as zero, or never observed, falls back independently: to the
number in `COLUMNS` (width) or `LINES` (height), then to 80 by 24.

The variable is read as a number. Surrounding whitespace is skipped (Unicode white space,
line terminators included, except U+0085; and U+FEFF); then a decimal with optional sign,
fraction and exponent, or an unsigned `0x`, `0b` or `0o` integer, gives a number rounded to
the nearest double. It is used when it is a whole number from 1 to `usize::MAX`. Empty, zero and
non-numeric text is ignored, and so is a negative, fractional, non-finite or larger number,
which a `usize` cannot hold.

## Output

A command that returns `Ok` has written all of its bytes to standard output, waiting on the
calling thread while the output is full: the status flag that makes reads nonblocking lives
on the open file description, which standard output usually shares with standard input. A
write that fails is returned as it is, and part of the bytes may already have been written.
`move_by` writes nothing for zero. The title is written unchanged between `ESC ] 0 ;` and a
bell. `set_progress(true)` writes the indeterminate-progress sequence at once and, when no
keepalive is running, starts one that writes it again every 1000 ms from then on; asking
again writes it again but never adds or restarts a running keepalive. `set_progress(false)`
always writes the clear sequence and stops the keepalive. A keepalive write that fails ends
that keepalive and is returned by the next `stop`; the next `set_progress(true)` then starts
a new keepalive.

## Write log

When `MAESTRO_TUI_WRITE_LOG` is set to a nonempty value, the data passed to `write` is also
appended to a file once standard output has accepted it; a write that standard output
refuses returns its error and appends nothing. A value naming an existing directory selects
`tui-YYYY-MM-DD_HH-MM-SS-PID.log` in it, with the local time when the terminal was created
and the process id; any other value is the file path. Directories are never created, and a
failure to append is ignored. Cursor, title, protocol and progress output is not logged.

## Errors

Operating-system errors are returned as `std::io::Error` from the call that met them, and
`start` outside a runtime returns an `ErrorKind::Other` error; the terminal prints no
diagnostics of its own (the runtime host prints the errors its work returns, as described
above), and a failure to append to the write log is ignored. Failures in
background tasks cannot be returned where they happen, so the first is kept and the next
`stop` reports it; dropping the terminal discards it.
