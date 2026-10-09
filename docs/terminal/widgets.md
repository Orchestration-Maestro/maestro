# Terminal widgets

The passive `Box`, `Text` and `Spacer` widgets implement the [component
contract](components.md): `Box` lays its children out inside padding over an optional
background, `Text` shows word-wrapped text, and `Spacer` shows empty rows. None of them
accepts input, takes focus or wants key-release events. `Box` here is
`maestro_tui::Box`, not `std::boxed::Box`; a glob import of the crate root would hide the
standard one.

A background is any `Rc<dyn Fn(&str) -> String>`. For displayed rows, a widget passes it
one row at a time, padded with spaces up to the viewport width, and shows what it returns,
so it can wrap the row in a colour escape. Box also passes the raw sample `"test"` and uses
its result only to decide whether to repaint. A widget does not measure what a background
returns.

Every method takes `&self`, so a background or a child may call back into the widget that
is rendering it. A widget never holds a borrow of its own state while it runs a
background or another component.

## Box

`Box::new(padding_x, padding_y, bg_fn)` renders its children one after another, indents
every child row by `padding_x` spaces and adds `padding_y` blank rows above and below.
`Box::default()` pads one column and one row and has no background. Horizontal padding is
never more than half the viewport width, rounded down, and the children are rendered at
the width that remains, which can be zero. Each row is padded with spaces up to the
viewport width before the background styles it. A child row wider than the viewport is
not cut, so children and backgrounds answer for the width of their own output.

A `Box` holds the same child array its callers can edit; replacing it leaves a running
walk on its original array. `children` returns the array itself, `set_children` makes
another array the one the box renders, `add_child` appends, `remove_child` removes the
first occurrence of a handle and ignores a missing one, and `clear` installs a new empty
array that a handle kept from before no longer reaches. The walk rules of a
[`Container`](components.md) apply: a render or an invalidation walks the array held when
it starts, reading each position when it is reached.

A box with no child row renders nothing, without padding rows and without calling the
background. Otherwise a `Box` rerenders its children and samples a configured background
(the row it returns for the text `test`) before deciding whether to reuse the composed
rows. They are reused when the width, the ordered child rows and the sample are all
unchanged since they were composed. `add_child`, `clear`, a `remove_child` that found its
child, `set_bg_fn` and `invalidate` drop them, and `invalidate` drops them before it
invalidates the children.

A child or background that changes the box while it renders does not make the render
fail. The render finishes with the child rows it collected and with the background in
effect at each call. If `set_bg_fn`, `invalidate`, `add_child`, `remove_child` (that found
its child) or `clear` was called during the render, its rows are not kept for reuse.

```rust
use std::rc::Rc;

use maestro_tui::{Box, Component, Text};

let gray = |row: &str| format!("\x1b[100m{row}\x1b[49m");
let panel = Box::new(1, 1, Some(Rc::new(gray)));
panel.add_child(Rc::new(Text::new("Content".into(), 0, 0, None)));
assert_eq!(
    panel.render(11),
    [
        "\x1b[100m           \x1b[49m",
        "\x1b[100m Content   \x1b[49m",
        "\x1b[100m           \x1b[49m",
    ]
);

panel.set_bg_fn(Some(Rc::new(|row: &str| format!("\x1b[44m{row}\x1b[49m"))));
assert_eq!(panel.render(11)[1], "\x1b[44m Content   \x1b[49m");
```

## Text

`Text::new(text, padding_x, padding_y, custom_bg_fn)` shows `text` between `padding_x`
columns on each side and `padding_y` blank rows above and below; `Text::default()` is
empty, with one column and one row of padding. Text with no non-whitespace scalar renders
no rows at all, padding included; other text is wrapped after each tab becomes three
spaces. Whitespace here is the tab, line feed, vertical tab, form feed, carriage return,
space, no-break space, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F, U+3000
and U+FEFF. The next-line control U+0085, U+180E and the zero-width space U+200B are
text. See [styled text](text.md) for wrapping behavior.

Horizontal padding is never more than half the viewport width, rounded down. The wrap
width is the width that remains, but at least one. A wrapped row wider than the cells
that remain is cut to the whole graphemes that fit, so an overwide grapheme is omitted
instead of split. That happens only when one grapheme is wider than the remaining cells or
when no cell remains, and then the cut row keeps its place in the output without the
omitted grapheme. Every row is padded with spaces up to the viewport width before the
background styles it. The
background receives the content rows in order, then one call for each of the `padding_y`
blank rows, and each blank row is shown above and below.

`Text` reuses its rows for an unchanged width until `set_text`, `set_custom_bg_fn` or
`invalidate` drops them; `set_text` with equal text counts. It does not sample a
background: when the state a background reads changes, call `invalidate`. A setter or
`invalidate` called while the text renders keeps its rows out of the cache, so the next
render shows the new values. The current render continues with the text it started with;
a replaced background applies from the next row on.

```rust
use std::rc::Rc;

use maestro_tui::{Component, Text};

let text = Text::new(
    "Hello World".into(),
    1,
    0,
    Some(Rc::new(|row: &str| format!("\x1b[100m{row}\x1b[49m"))),
);
assert_eq!(
    text.render(9),
    ["\x1b[100m Hello   \x1b[49m", "\x1b[100m World   \x1b[49m"]
);

text.set_text("Updated".into());
assert_eq!(text.render(11), ["\x1b[100m Updated   \x1b[49m"]);
```

## Spacer

`Spacer::new(lines)` renders `lines` empty strings, whatever the viewport width, and
`Spacer::default()` renders one. `set_lines` changes the count. The rows are empty
strings, not spaces. A `Spacer` caches no rows, so invalidating it does nothing.

```rust
use maestro_tui::{Component, Spacer};

let spacer = Spacer::new(2);
assert_eq!(spacer.render(80), ["", ""]);

spacer.set_lines(0);
assert!(spacer.render(80).is_empty());
```

## Input

`Input` edits a single line with horizontal scrolling. It uses the existing
[keybindings](keybindings.md) and [styled-text helpers](text.md).

```rust
use std::{cell::RefCell, rc::Rc};
use maestro_tui::{Input, tui::InputHandler};

let input = Input::new();
input.set_value("initial".into());
assert_eq!(input.get_value(), "initial");
let submitted = Rc::new(RefCell::new(String::new()));
let output = submitted.clone();
input.set_on_submit(Some(Rc::new(move |value| *output.borrow_mut() = value.into())));
input.handle_input("\r");
assert_eq!(*submitted.borrow(), "initial");
```

Default bindings:

- Enter submits.
- Ctrl+A / Ctrl+E move to the start / end.
- Ctrl+W or Alt+Backspace kills the preceding word.
- Ctrl+U kills to the start.
- Ctrl+K kills to the end.
- Ctrl+Left / Ctrl+Right move by word.
- Alt+Left / Alt+Right move by word.
- Left/Right arrows, Backspace and Delete move or delete a grapheme at the cursor.

Submit and cancel callbacks run synchronously; nested calls read the current
callbacks. Replacing the value retains or clamps its logical cursor position and
preserves history and pending paste; changing the value ends an active yank chain.
Edits segment the prefix or suffix at the cursor, including cross-boundary joins.
A completed paste removes CR/LF, expands tabs to four spaces and captures one undo
snapshot before dispatching its suffix. Rendering returns one string; widths at
most two show only the clipped prompt without a cursor, and a focused wider view
emits the hardware cursor marker. Tabs outside recognized terminal escapes display
as three spaces; escape payloads, stored text and the editing position stay
unchanged. A cursor mapped to the selected text's end gets a reserved blank cell. Input has no rendering cache.

### Shared editing history

`kill_ring::KillRing` stores deleted text: `push` ignores empty strings, and its
explicit `KillRingOptions` select prepending or appending to the newest entry when
accumulating. `peek` borrows that entry; `rotate` moves it to the front only when
there are at least two entries; `length` counts entries. Ctrl+Y yanks and Alt+Y
rotates an active yank.

`undo_stack::UndoStack<S>` stores `S::clone()` on `push` and transfers the newest
snapshot on `pop`. `clear` drops snapshots and `length` counts them. Input's
snapshots own their text; an arbitrary shared-handle `Clone` is not a deep copy.
Ctrl+- restores an input snapshot without rewinding the kill ring. Consecutive
non-whitespace typing shares a snapshot; a whitespace-containing chunk starts one.

## Loader

`Loader::new(tui, spinner_color_fn, message_color_fn, message, indicator)` starts an
optional animated indicator beside a message; an absent message is `Loading...`.
The default indicator cycles through ten spinner frames at 80 ms. Explicit
`LoaderIndicatorOptions` display their frames without spinner coloring, even when
both fields are absent. An empty frame list hides the indicator, and a single
frame stays static. Message coloring applies in either mode.

`stop` retains the displayed content. `start` refreshes it and restarts the interval
without resetting the frame. `set_message` refreshes the message even while stopped;
`set_indicator` resets the frame and starts again. Cloned handles share this state.
Rendering prepends one empty row to [Text](#text)'s result, with horizontal padding
1 and vertical padding 0. Render and invalidation do not rerun the color callbacks.

Absent, nonpositive and NaN intervals use 80 ms. Only two or more frames with a
finite representable delay animate; positive delays below the native quantum use
1 ns. Infinite or unrepresentable positive delays leave the initialized display
without a finite tick. The loader uses the writer's existing runtime and ordinary
render requests. Dropping the last handle cancels its animation timer.

```rust
use std::rc::Rc;
use maestro_tui::{Loader, TUI};

fn show_status(tui: TUI) {
    let loader = Loader::new(
        tui.clone(),
        Rc::new(|frame| format!("\x1b[36m{frame}\x1b[39m")),
        Rc::new(str::to_owned),
        None,
        None,
    );
    tui.add_child(Rc::new(loader.clone()));
    loader.start();
    loader.set_message("Still loading...".into());
    loader.stop();
}
```

## `CancellableLoader`

`CancellableLoader::new(loader)` composes a [Loader](#loader) with a shared
cancellation signal. `loader()` exposes the embedded owner for animation and
message operations; rendering and invalidation delegate to it.

A matching current `tui.select.cancel` binding (Escape or Ctrl+C by default)
aborts the signal before calling the installed `on_abort` callback, if any.
Each matching input calls the current callback again; removing it does not
prevent cancellation. `signal()` borrows the retained signal for explicit
cloning, and `aborted()` reads its state. See
[`maestro-cancellation`](../../crates/maestro-cancellation/README.md) for signal
observation. `dispose()` stops the embedded loader without aborting its signal.

```rust
use std::rc::Rc;
use maestro_tui::{CancellableLoader, Loader, TUI};

fn show_cancellable_status(tui: TUI) {
    let loader = Loader::new(
        tui.clone(), Rc::new(str::to_owned), Rc::new(str::to_owned),
        Some("Working...".into()), None,
    );
    let widget = Rc::new(CancellableLoader::new(loader));
    let signal = widget.signal().clone();
    widget.set_on_abort(Some(Rc::new(move || assert!(signal.is_aborted()))));
    tui.add_child(widget.clone());
    tui.set_focus(Some(widget.clone()));
    widget.dispose();
}
```
