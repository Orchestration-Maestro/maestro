# Terminal components

`maestro-tui` defines what a renderable component is and ships the pieces that need
no terminal: a container, a single-line text component and the contracts for
overlays, terminals, editors and completion. It performs no I/O and starts no
timers; whatever connects a real terminal supplies a `Terminal` implementation.

## Components

A `Component` renders itself for a viewport width as lines no wider than the width.
Everything else is an optional capability that defaults to absent:

| Capability | Method | Default |
| --- | --- | --- |
| Drop cached rendering | `invalidate` | does nothing |
| Receive input while focused | `input_handler` | `None` |
| Receive key-release events | `wants_key_release` | `false` |
| Hold focus and show a cursor | `focusable`, `focusable_mut` | `None` |

Absence is meaningful: routing code asks whether a capability exists, so a
component without input never silently swallows it. `is_focusable` tests for the
capability, not for the current focus. A focused component emits `CURSOR_MARKER`
where the hardware cursor belongs; the marker is an escape that occupies no cells.

A `Container` renders its children in order and concatenates their lines. Children
are shared handles (`Rc<RefCell<dyn Component>>`), so whoever keeps a handle can
keep editing the child and the container shows the edit. The same child may appear
twice. `remove_child` removes the first occurrence by identity and ignores a child
that is not present; `invalidate` reaches every child in order.

`TruncatedText` shows the first line of its text, truncated to the viewport, between
`padding_y` blank rows. Horizontal padding is `padding_x` on each side but never
more than half the width, so a narrow viewport still shows content, and the content
is cut to what remains. A zero-width viewport renders empty lines. Every line is
filled with spaces to exactly the viewport width.

```rust
use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::components::TruncatedText;
use maestro_tui::tui::{Component, ComponentHandle, Container};

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
`visible` callback receives the caller's viewport width and height unchanged.
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
are required; history, insertion, expanded text, the completion provider, padding
and the visible item count are optional and return `None` when unsupported and
`Some(())` once carried out. Callers fall back to the plain text when expanded text
is `None`. `EditorCallbacks` holds the submit, change and border-colour callbacks;
all start absent and the editor calls them.

An `AutocompleteProvider` suggests items for lines and a cursor. Cursor columns are
UTF-8 byte offsets within a line, never cell or character counts, in requests and in
the `CompletionResult` that applying an item returns. The request borrows the
caller's cancellation signal, whose type the provider chooses, so the toolkit
depends on no cancellation implementation. No suggestions (`None`), an empty list
and a non-empty list are three different answers, and a provider that has no view on
file completion returns `None` where another may return `Some(false)`.
`SlashCommand::get_argument_completions` returns a future that may be ready at once
or suspend.
