# Terminal components

`maestro-tui` defines what a renderable component is and ships the pieces that need
no terminal: a container, text and image components and the contracts for
overlays, terminals, editors and completion. It performs no terminal I/O and starts no
timers; whatever connects a real terminal supplies a `Terminal` implementation.
See [inline images](images.md) for capability detection and retained image rendering,
[key input](keys.md) for decoding and [keybindings](keybindings.md) for configurable actions.

The crate root re-exports the component, container, overlay, terminal, editor and
completion names used below, so `maestro_tui::Component` and
`maestro_tui::tui::Component` are the same trait; supporting types such as
`ComponentHandle` stay in their modules.

## Components

A `Component` renders itself for a supplied viewport width. Image escapes and
untruncated image fallback text do not guarantee lines within that width.
Everything else is an optional capability that defaults to absent:

| Capability | Method | Default |
| --- | --- | --- |
| Drop cached rendering | `invalidate` | does nothing |
| Receive input while focused | `input_handler` | `None` |
| Receive key-release events | `wants_key_release` | `false` |
| Hold focus and show a cursor | `focusable`, `focusable_mut` | `None` |

Absence is explicit: a component without input returns `None` from `input_handler`
rather than a handler that ignores its input. `is_focusable` tests for the
capability, not for the current focus. A focused component is expected to emit
`CURSOR_MARKER` where the hardware cursor belongs; the marker is an escape that
occupies no cells.

A `Container` renders its children in order and concatenates their lines. Children
are shared handles (`Rc<RefCell<dyn Component>>`), so whoever keeps a handle can
keep editing the child and the container shows the edit. The same child may appear
twice. `remove_child` removes the first occurrence by identity and ignores a child
that is not present; `invalidate` reaches every child in order.

`TruncatedText` shows the first line of its text, truncated to the viewport, between
`padding_y` blank rows. Horizontal padding is `padding_x` on each side but never
more than half the width (rounded down), and the first line is cut to the columns
that remain. Those can be none, as at width 2 with padding 1, and then the line is
only spaces. A zero-width viewport renders empty lines. Every line, blank rows
included, is padded with spaces to exactly the viewport width.

```rust
use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Component, Container, TruncatedText};

let title: ComponentHandle = Rc::new(RefCell::new(TruncatedText::new("Hello world\nignored".into(), 1, 0)));
let mut container = Container::new();
container.add_child(Rc::clone(&title));
container.add_child(Rc::clone(&title));

let lines = container.render(14);
assert_eq!(lines, [" Hello world  ", " Hello world  "]);

container.remove_child(&title);
assert_eq!(container.render(14), [" Hello world  "]);
```

## Overlays

`OverlayOptions` records how an overlay is sized, anchored, offset, margined and
shown; every member is optional and sizes keep a percentage's spelling. The
`visible` callback takes the viewport width and height.
`OverlayHandle` is the control surface of a shown overlay. Resolving positions and
managing a stack of overlays belong to the renderer that uses these records.

## Terminals

`Terminal` covers starting with input and resize callbacks, writing, dimensions,
cursor movement and visibility, clearing, the window title, the progress indicator
and draining stale input. Effects return `io::Result` and carry the device's error
unchanged. `drain_input` takes optional limits and returns a future; an adapter
applies its own defaults, 1000 ms at most and 50 ms idle, when they are omitted.

## Editors and completion

An `EditorComponent` is a component that must also handle input. Its text accessors
are required. History, insertion, the completion provider, padding and the visible
item count are optional: each returns `None` when unsupported and `Some(())` once
carried out. Expanded text is optional too: `get_expanded_text` returns `None` when
unsupported and `Some(text)` with the markers expanded otherwise, so `get_text` is
the fallback. `EditorCallbacks` holds the submit, change and border-colour
callbacks; all start absent and the editor calls them.

An `AutocompleteProvider` suggests items for lines and a cursor. Cursor columns are
UTF-8 byte offsets within a line, never cell or character counts, in requests and in
the `CompletionResult` that applying an item returns. The request borrows the
caller's cancellation signal, whose type the provider chooses, so the toolkit
depends on no cancellation implementation. No suggestions (`None`), an empty list
and a non-empty list are three different answers, and a provider that has no view on
file completion returns `None` where another may return `Some(false)`.
`SlashCommand::get_argument_completions` is an optional function that returns a
future, which may be ready at once or suspend.
