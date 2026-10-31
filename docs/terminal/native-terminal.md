# Native terminal

`ProcessTerminal` connects a Unix or Windows process to the toolkit. It implements the
toolkit's `Terminal` port over the process's own standard input and standard output, so it
needs no terminal framework and sends the toolkit raw protocol text rather than decoded key
events. The native runtime host described [below](#the-runtime-host) exists on every native
target, and only browser targets build the crate empty. The terminal drives the standard
handles directly and never opens the controlling terminal or console, so redirecting one of
the two changes what it controls: raw mode and input come from standard input, window size
and output from standard output. [Windows consoles](#windows-consoles) describes what differs
there.

```rust,no_run
# #[cfg(any(unix, windows))] {
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

The terminal starts no runtime, and on Unix no thread. It is given a `LocalSet`, and every task it
creates runs there: the input reader, the output writer, the progress keepalive and every callback. Tasks run
only while the caller drives that set inside a Tokio runtime whose I/O, time and signal
drivers are enabled (time alone on Windows); a runtime built without one of them is a caller error, which Tokio
reports itself. The standalone one-input wait validates the I/O driver before changing
standard-input attributes or flags. `start` called outside a running runtime returns an `ErrorKind::Other` error
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
  keeps running. A failure to write there releases the lock on standard error and then
  reaches the registered output-error handlers once.
- `environment` returns the value as the platform holds it, with invalid text converted
  lossily; it does not trim or interpret the value.
- `log_context` reports the home directory (empty when the platform has none), the current
  UTC time in milliseconds and a random lowercase hexadecimal nonce.
- `append_log` and `write_log` act on the given path as the operating system resolves it.
  Append creates the file but not its directory; write creates the directory and replaces the
  file. Both return the operating system's errors.

Dropping the set drops the work still pending. The host's own tasks hold no handle to the
host; callers release any strong handle their callbacks capture.

The interactive mode installs termination and output-error handlers before starting
this terminal. Its Unix signal registrations share the process owner's native
default lifetime: the final removal restores TERM/HUP defaults, including while
application disposal is pending. See [interactive shutdown](../chat/interactive-mode.md#native-shutdown).

## Starting and stopping

`new` reads the size of standard output and the write-log setting and does nothing else.
On Unix, `start` puts standard input in raw mode when it is a terminal, makes its reads nonblocking,
subscribes to window-change signals, writes the bracketed-paste enable and the keyboard
query, registers nonblocking output and spawns the input and output tasks.

On Unix, raw mode is applied to the device standard input refers to. It clears echo, line editing,
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
delivered). It drains queued output before writing, in order: the progress clear when progress was active, the
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
was started. Standard input that was not a terminal has no attributes to restore. Output status flags
are saved when its writer is acquired and restored after the queue drains. When
input and output share an open file description, input restoration runs last.

## Input

Standard input uses the shared [streaming text decoder](../text.md) and is read as UTF-8,
on Unix at most 4096 bytes per read: a read takes what
is available at that moment, and a longer burst arrives in consecutive reads. A character
split across reads is held until it completes, and invalid bytes become U+FFFD; only text
that is complete reaches the toolkit's shared `StdinBuffer`. Its framing and release rules
are described in [Input](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/terminal/input.md). Each event it releases reaches the input callback in
order, except keyboard replies and the events of a drain, which are described below; a
paste has its two markers put back. Malformed UTF-8 is never treated as a legacy
alt-modified byte. At the end of input the decoder is flushed and reading stops, but the
buffer's and held fragment's deadlines and window-size tracking continue until `stop`.
A read error ends the input task, and with it that tracking; the first such error is kept
and returned by the next `stop`.

## Keyboard protocols

`start` requests the enhanced keyboard flags (7), then asks the terminal which flags it runs
and for its device attributes. Maestro waits for the answers; no timer chooses a mode. A reply
is exactly `ESC [ ?`, one or more ASCII digits and `u` for the flags, or `ESC [ ?`, any ASCII
digits and semicolons and `c` for the device attributes. Every reply is consumed and never
reaches the input callback; other sequences are ordinary input.

- Non-zero flags set `kitty_protocol_active` and the toolkit's shared flag, and switch the
  modified-key reporting off if it is on.
- Zero flags, or device attributes while the enhanced protocol is off, enable the modified-key
  reporting. A terminal that does not know the enhanced protocol answers only the attributes,
  so it gets the fallback as soon as that reply arrives.
- A reply that arrives later still counts: a late flags reply turns the fallback off and the
  enhanced protocol on.

The framing may release the start of a reply (`ESC [` or `ESC [ ?` and digits) before the rest
arrives. The terminal holds such a fragment for up to 150 ms and joins it with what follows. A
fragment that never completes is then delivered as ordinary input, and a following sequence
that cannot continue it delivers the fragment first. At `stop` the flags requested at `start`
are popped, and the modified-key reporting is disabled if it was on. Every start requests the
flags, so a terminal that is started again while it holds them pops each request at `stop`.

## Ending without `stop`

Dropping a terminal that was started, or that hid the cursor, restores what it owns
exactly as `stop` does: it shows the cursor if this terminal hid it and nothing showed
it since, clears an active progress indicator, turns bracketed paste off, pops the
requested keyboard flags and turns the modified-key reporting off. `stop` writes the
cursor only in that same case, so a toolkit that showed the cursor before stopping
causes no second write. A process that ends through `std::process::exit` skips drops;
the application stops its terminal before it calls that function.

## Draining

`drain_input(max, idle)` is for the end of a session, so key releases do not reach the
shell. It defaults to at most 1000 ms and 50 ms without input. When called, before the
returned future is polled, it disables the keyboard modes and stops delivering input.
Input still goes to the shared buffer under the [Input](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/terminal/input.md) framing and release rules.
While the drain runs, every event the buffer releases is discarded, whatever released it.
The drain does not reset framing state.

The future owns its state, so the terminal it came from stays usable while the drain
is pending. It checks the elapsed quiet time and the maximum, then waits the shorter
of the idle time and the time left, so the idle exit is noticed on that cadence. Only complete decoded
text counts as activity, not the bytes of an unfinished character. When the future ends, or
is dropped, input delivery resumes with the original callback. A drain also closes keyboard
negotiation for the current generation: no later reply changes a mode, a held reply fragment
is dropped, and a reply that arrives is consumed instead of delivered.

## Window size

The size is cached from standard output when the terminal is created and refreshed when the
input task starts and on each window-change signal (on Windows, each window-size record). The resize callback runs only when the
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

On Unix, while started, output commands accept owned UTF-8 bytes into one ordered queue and
return without waiting for the terminal reader. One task writes at most 16 KiB per
syscall and waits on descriptor readiness when output is full, leaving input runnable.
A permanent write failure drops the remaining queue, refuses further accumulation
and is reported by the next `stop`. Regular files and `/dev/null`, which the reactor
refuses, stay synchronous; commands while stopped also write synchronously and
return the original write error. Stopping or dropping a started terminal cancels
the writer and drains its retained partial buffer before restoring modes. This final
drain has no deadline and can wait indefinitely for a live reader that never resumes.

`on_output_error` registers a handler for a failure to write, and returns the operation
that removes it; removing it twice does nothing and a stale removal leaves a later
registration in place. Each failure is passed to the registered handlers once, with the
original error: a synchronous write, an asynchronous writer failure, a keepalive write and
a failure to report on standard error. A read error from standard input is retained for
`stop` and is never passed to them. The handlers run with no borrow held, so one may remove
or replace itself.

`move_by` writes nothing for zero. The title is written unchanged between `ESC ] 0 ;` and a
bell. `set_progress(true)` writes the indeterminate-progress sequence to the same queue and, when no
keepalive is running, starts one that writes it again every 1000 ms from then on; asking
again writes it again but never adds or restarts a running keepalive. `set_progress(false)`
always submits the clear sequence and stops the keepalive. A synchronous keepalive write that fails ends
that keepalive and is returned by the next `stop`; the next `set_progress(true)` then starts
a new keepalive. A queued writer failure is retained for `stop`, which cancels the
keepalive even when the failed queue has already been discarded.

## Output backlog

`Terminal::pending_output_bytes()` reports accepted bytes not yet written, including
the remainder of a partially written buffer. The default is zero for adapters that
do not report a backlog. The toolkit defers composition while more than 256 KiB
are pending and retries after 10 ms. It draws the latest component state after the
backlog drains, preserving forced-redraw and clear-on-shrink intent. Ordinary and
forced renders share this gate; stopping cancels its retry.

## Write log

When `MAESTRO_TUI_WRITE_LOG` is set to a nonempty value, the data passed to `write` is also
appended to a file at queue admission while the terminal is started. Synchronous
writes append only after standard output accepts them; a refused synchronous write
returns its error and appends nothing. A value naming an existing directory selects
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

## One-input wait

`terminal::wait_for_input()` is a standalone future for a host-owned pause.
Its first poll checks the caller's Tokio runtime and opens the existing input source
before acquiring raw/nonblocking stdin through the terminal's restoration guard.
It completes on one nonempty raw read: no key decoding, Enter requirement, protocol
filtering, resize listener, output-mode negotiation, background task or runtime creation.
A pipe is supported and arbitrary bytes count as input. EOF stays pending until drop.

Success, errors and future drop restore the saved terminal attributes and descriptor
flags, including initially raw state. Explicit completion reports restoration failures
unless an earlier operation already failed. Opening or acquisition failures release the
input registration. On Unix, a later wait opens its own reader rather than retaining one.
On Windows it acquires the standard-input console as `start` does, without the output
mode, and completes on the first text or redirected bytes the shared reader delivers. EOF
stays pending until drop, and drop restores the saved mode.

## Windows consoles

On Windows, `start` saves the console mode of standard input and applies raw mode: line
input, echo and processed input (so Ctrl+C arrives as input) are turned off and window-size
records on. It then adds escape-sequence input, so the console sends the sequences a
terminal sends and modified keys such as Shift+Tab are told apart. A console that refuses
escape-sequence input stays in raw mode without it, and `start` still succeeds; such a
console delivers typed characters but no sequences for keys that produce none. `start` also
turns on escape-sequence processing for the standard-output console when it accepts it.
`stop` restores the saved output mode. Standard input's original mode is restored after
the last terminal or standalone wait releases it; repeated and replacing starts keep the
mode saved first, as on Unix. Standard input or output that is not a console is left alone.

Windows input uses one process-wide reader thread shared by terminal generations and
standalone waits. It queries an idle console every 10 ms and reads the waiting records.
The count is only a latency optimization: another reader can consume those records before
the read begins. Stopping removes the generation's subscription without joining or waiting
for that read, so neither a console nor a pipe can block shutdown. A completed batch with
no subscriber is held for the next start, and concurrent subscriptions each receive the
same input. External readers are not coordinated.

Character-producing key presses, including NUL from Ctrl+Space or Ctrl+2, and Alt releases
that carry a character composed on the numeric keypad become text in order, repeated as
the record says. Modifier-only records are not text. Control-state metadata distinguishes
NUL-producing keys from records that carry no character. Surrogate pairs split across
reads are joined; an unpaired surrogate becomes U+FFFD. Text then follows the shared
[Input](#input) path. Window-size records refresh the size from the standard-output console.
A read failure ends the source and is returned by the next `stop`.

Redirected standard input is read as bytes on the same reader thread, at most 4096 bytes
per read. A pending read can outlive `stop`, but cannot forward to a stopped generation;
its bytes are held for resume. EOF ends input delivery. Output is synchronous, so
`pending_output_bytes` is always zero.
