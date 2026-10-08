# Retained terminal frames

`TUI` draws components to a `Terminal` as a retained frame. It remembers the lines it
last drew and writes only what changed, so a spinner redraws one row and the
terminal's scrollback stays intact. It never switches to the alternate screen, and
each update is one synchronized write (`CSI ? 2026 h` … `CSI ? 2026 l`).

The writer performs no terminal or file I/O and starts no timers of its own. A
`Terminal` receives the bytes. A `TuiRuntime` supplies the monotonic clock, defers
render callbacks, answers environment queries and performs the log file effects, so
the same writer runs under a native event loop, a browser or a test with a manual
clock. `TerminalImage` is the shared image state its components already use; the
writer queries and updates the cell size through that same instance.

```rust
# use std::cell::RefCell;
# use std::future::Future;
# use std::io;
# use std::path::Path;
# use std::pin::Pin;
# use std::rc::Rc;
# use std::time::Duration;
use maestro_tui::tui::{
    ComponentHandle, LogContext, RenderCallback, RenderTimer, TuiRuntime,
};
use maestro_tui::{Component, Terminal, TerminalImage, TUI};

# /// Collects what the writer sends.
# struct Memory(Rc<RefCell<Vec<String>>>);
# impl Terminal for Memory {
#     fn start(&mut self, _: Box<dyn FnMut(&str)>, _: Box<dyn FnMut()>) -> io::Result<()> { Ok(()) }
#     fn stop(&mut self) -> io::Result<()> { Ok(()) }
#     fn drain_input(&mut self, _: Option<Duration>, _: Option<Duration>)
#         -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> { Box::pin(async { Ok(()) }) }
#     fn write(&mut self, data: &str) -> io::Result<()> { self.0.borrow_mut().push(data.to_owned()); Ok(()) }
#     fn columns(&self) -> usize { 12 }
#     fn rows(&self) -> usize { 4 }
#     fn kitty_protocol_active(&self) -> bool { false }
#     fn move_by(&mut self, _: isize) -> io::Result<()> { Ok(()) }
#     fn hide_cursor(&mut self) -> io::Result<()> { self.write("\x1b[?25l") }
#     fn show_cursor(&mut self) -> io::Result<()> { self.write("\x1b[?25h") }
#     fn clear_line(&mut self) -> io::Result<()> { Ok(()) }
#     fn clear_from_cursor(&mut self) -> io::Result<()> { Ok(()) }
#     fn clear_screen(&mut self) -> io::Result<()> { Ok(()) }
#     fn set_title(&mut self, _: &str) -> io::Result<()> { Ok(()) }
#     fn set_progress(&mut self, _: bool) -> io::Result<()> { Ok(()) }
# }
# /// Keeps deferred callbacks until the example runs them.
# #[derive(Default)]
# struct Deferred(RefCell<Vec<RenderCallback>>);
# struct Timer;
# impl RenderTimer for Timer { fn cancel(&mut self) {} }
# impl TuiRuntime for Deferred {
#     fn now(&self) -> Duration { Duration::from_secs(1) }
#     fn schedule(&self, _: Duration, callback: RenderCallback) -> Box<dyn RenderTimer> {
#         self.0.borrow_mut().push(callback);
#         Box::new(Timer)
#     }
#     fn environment(&self, _: &str) -> Option<String> { None }
#     fn log_context(&self) -> LogContext {
#         LogContext { home: "/home/user".into(), iso_time: String::new(), unix_ms: 0, nonce: String::new() }
#     }
#     fn append_log(&self, _: &Path, _: &str) -> io::Result<()> { Ok(()) }
#     fn write_log(&self, _: &Path, _: &str) -> io::Result<()> { Ok(()) }
# }
# fn main() -> io::Result<()> {
/// A line of text that can change after it is shown.
struct Status(RefCell<String>);

impl Component for Status {
    fn render(&self, _width: usize) -> Vec<String> {
        vec![self.0.borrow().clone()]
    }
}

let sent = Rc::new(RefCell::new(Vec::new()));
let host = Rc::new(Deferred::default());
let tui = TUI::new(
    Rc::new(RefCell::new(Memory(Rc::clone(&sent)))),
    host.clone(),
    TerminalImage::new(|_| None, || 1),
    None,
);
let status = Rc::new(Status(RefCell::new("working".to_owned())));
tui.add_child(status.clone() as ComponentHandle);

// Starting hides the cursor and asks for the first frame; the host runs it.
tui.start()?;
for callback in host.0.take() {
    callback()?;
}
assert_eq!(sent.borrow().concat(), "\x1b[?25l\x1b[?2026hworking\x1b[0m\x1b]8;;\x07\x1b[?2026l\x1b[?25l");

// A change redraws only the rows that differ.
sent.borrow_mut().clear();
*status.0.borrow_mut() = "done".to_owned();
tui.request_render(false);
for callback in host.0.take() {
    callback()?;
}
assert_eq!(sent.borrow().concat(), "\x1b[?2026h\r\x1b[2Kdone\x1b[0m\x1b]8;;\x07\x1b[?2026l\x1b[?25l");
# Ok(())
# }
```

## Defaults

| Setting | Default | How to change it |
| --- | --- | --- |
| Hardware cursor | hidden | `MAESTRO_HARDWARE_CURSOR=1`, the constructor argument or `set_show_hardware_cursor` |
| Clear on shrink | off | `MAESTRO_CLEAR_ON_SHRINK=1` or `set_clear_on_shrink` |
| Mobile-height exception | off | a nonempty `TERMUX_VERSION` |
| Redraw log | off | `MAESTRO_DEBUG_REDRAW=1` |
| Differential record | off | `MAESTRO_TUI_DEBUG=1` |

A boolean variable is on only when it is exactly `1`; the constructor argument wins
over the variable. The writer places the terminal cursor at the first `CURSOR_MARKER`
on the last visible row that holds one and shows it only when this setting is on; with
no marker, or with the setting off, the cursor stays hidden. A focused component is the
one that emits the marker, but the writer does not check focus. Turning the setting off
hides the cursor at once, and any change asks for a frame.

## How a frame is drawn

Every text line ends with a reset of styles and of the open hyperlink, so one line
cannot leak into the next, and the Thai and Lao AM vowels are decomposed for the
terminal. Image lines are written untouched.

1. **First frame.** The lines are written from the current position without clearing
   anything, so earlier output stays in the scrollback.
2. **Changed rows.** The writer finds the first and last rows that differ, moves the
   cursor there and rewrites every row in between, erasing each before drawing it.
   Rows added below the bottom of the screen scroll it by as many rows as they need
   before they are drawn.
3. **Shorter frames.** The rows that disappeared are erased and the cursor returns to
   the last row of the new content. A frame with no rows erases row zero as well.
4. **Full redraws.** The screen and scrollback are cleared and every line is drawn
   again when the width changes, when the height changes (unless the mobile-height
   exception applies, which keeps the scrollback), when clear-on-shrink is on and the
   frame is shorter than the most rows ever drawn, when erasing rows would scroll the
   viewport up, when a changed row has scrolled out of the viewport and when a redraw
   is forced.

Components render in order by walking the live list of children: one that an earlier
component adds while the frame renders is rendered in the same frame, and one that is
removed before its turn is skipped. `invalidate` walks the list the same way, and a
container does the same with its own children.

`full_redraws` counts the full redraws begun. The logical end of the content and the
row the terminal cursor is on are tracked separately, because placing the hardware
cursor moves it away from the end of the content.

## Images

The ids of Kitty images are read from each line's first graphics sequence: the
first valid `i` parameter, as a decimal or exponent literal or an unsigned `0x`,
`0b` or `0o` literal that is a whole number from 1 to 4294967295. Before a changed
span is drawn, the images on the retained rows it covers are deleted, each id once and
in the order first seen. The span is widened to end at the last retained image line
below its start, so an image is deleted and drawn again when a row above it changes.
A full redraw deletes every image of the previous frame before clearing the screen.

## Render requests

`request_render(false)` queues a frame at least 16 ms after the previous one starts
and merges any further request until then. `request_render(true)` forgets the
retained frame and queues an immediate redraw of everything. The host always defers
the callback, even for a zero delay, and the callback's `io::Result` is returned to
whoever drives the host. A component may request another frame while it renders; the
request is kept and queued after the current frame. `stop` cancels the pending frame,
including the immediate one a forced request queued, and forgets any request, so
`start` after a `stop` draws again, and requests made while stopped are dropped.

## Input

`start` installs the terminal's input callback. Each chunk goes through, in order:

1. The input listeners, in the order they were added. The list is walked live: a
   listener added by an earlier one runs in the same dispatch and a removed one is
   skipped. A listener passes the input on, consumes it or replaces it. A replacement,
   even an empty one, is what the later listeners see; input that is still empty after
   the last listener is delivered to nothing.
   `add_input_listener` returns a function that removes the listener and may be called
   again harmlessly; dropping it does not remove the listener.
2. The cell-size reply check. A chunk that is exactly `ESC [ 6 ; height ; width t`
   with ASCII digits is a reply and is never forwarded. A zero value or one beyond
   32 bits is dropped and leaves the measured size alone; any other reply updates the
   shared cell size, invalidates every component and asks for a frame. Anything else,
   including a bare escape, negative or fractional numbers, a trailing newline or a
   fragment, is ordinary input.
3. The debug key, `shift+ctrl+d`, when `set_on_debug` installed a callback.
4. The focused component, if it can take input. Key-release events are dropped
   unless it asks for them with `wants_key_release`. A frame is requested after
   each delivery.

`set_focus` clears the focus flag of the previous component and sets it on the new
one at once, without requesting a frame. A component that cannot hold focus is not
flagged. Because the flag changes before `set_focus` returns, a component that calls it
from inside its own input or render callback, and one nested in a container that was
never added to the writer, sees the change in the rest of that callback.

A component may also call `invalidate`, on the writer or on a container, from inside its
own input or render callback. Each call invalidates every child of the list it walks, in
order, before it returns: the running component is invalidated, the components still to
render in that frame lose their cached rendering before they render, and a child listed
twice is invalidated twice. A focused component that is not among the children is not
invalidated.

The cell size is only requested at startup when the terminal supports images.

## Errors and diagnostics

A text line wider than the terminal is never drawn. Before any frame is written the
writer checks every line; for the first oversized one it writes a crash report to
`{home}/.maestro/agent/maestro-crash.log`, restores the terminal (as `stop` does) and
fails the frame with

```text
Rendered line {i} exceeds terminal width ({visible_width} > {width}).

This is likely caused by a custom TUI component not truncating its output.
Use visibleWidth() to measure and truncateToWidth() to truncate lines.

Debug log written to: {crash_path}
```

The report lists the width, the offending row and every rendered line with its
measured width. Truncate component output to the width it is given; the writer does
not clip it.

With `MAESTRO_DEBUG_REDRAW=1` each full redraw appends one line to
`{home}/.maestro/agent/maestro-debug.log`:
`[{time}] fullRender: {reason} (prev={rows}, new={rows}, height={height})`. The
reasons are `first render`, `terminal width changed (a -> b)`,
`terminal height changed (a -> b)`, `clearOnShrink (maxLinesRendered=n)`,
`deleted lines moved viewport up (row < top)` and `firstChanged < viewportTop (row < top)`.
A forced redraw reports the retained extent as `-1`.

With `MAESTRO_TUI_DEBUG=1` each differential update writes its first changed row,
cursor rows, the new and retained lines and the exact bytes to
`/tmp/tui/render-{unix_ms}-{nonce}.log`.

Appending to the redraw log does not create its directory; the crash report and the
differential record create theirs. A failed log effect fails the frame.
