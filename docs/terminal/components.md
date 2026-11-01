# Terminal components

`maestro-tui` defines what a renderable component is and ships the pieces that need
no terminal: container, box, text, spacer, input, multiline editor, loader, image,
Markdown, command-selection and settings components; contracts for overlays,
terminals, editors and completion; and the `TUI` frame writer that draws
components to a terminal. It performs no terminal I/O and starts no timers; whatever
connects a real terminal supplies a `Terminal` implementation, and the host of the frame
writer supplies a `TuiRuntime` for time, deferral and log files.
See [terminal Markdown](markdown.md) for styled blocks and [widgets](widgets.md)
for padded content, editing, loading, cancellable loading, command selection
and settings, [retained
frames](rendering.md) for the frame writer, [inline images](images.md) for capability
detection and retained image rendering, [key input](keys.md) for decoding and
[keybindings](keybindings.md) for configurable actions.

The crate root re-exports the component, container, widget, overlay, terminal, editor and
completion names used below and in the widget guide, so `maestro_tui::Component` and
`maestro_tui::tui::Component` are the same trait; supporting types such as
`ComponentHandle` stay in their modules.

## Components

A `Component` renders itself for a supplied viewport width. Image escapes do not
guarantee lines within that width.
Everything else is an optional capability that defaults to absent:

| Capability | Method | Default |
| --- | --- | --- |
| Drop cached rendering | `invalidate` | does nothing |
| Receive input while focused | `input_handler` | `None` |
| Receive key-release events | `wants_key_release` | `false` |
| Hold focus and show a cursor | `focusable` | `None` |

`InputHandler::try_handle_input` returns ordinary input failures; its default calls
legacy `handle_input` once and returns `Ok(())`. Focused dispatch uses the fallible
method. `SettingsList` forwards it to its retained submenu without interpreting the
input itself; its main-list behavior is unchanged. Host reporting belongs to the
[frame writer](rendering.md), not to the default handler.

Absence is explicit: a component without input returns `None` from `input_handler`
rather than a handler that ignores its input. `is_focusable` tests for the
capability, not for the current focus. A focusable component owns a `FocusFlag`, which
the frame writer sets when focus moves; the component reads it when it renders, so a
change made during that render counts. `FocusFlag::forwarding` keeps an independent
flag and synchronously forwards every set to a retained focusable target, even
when the outer value is unchanged. Direct target changes do not alter that flag. A focused component is expected to emit
`CURSOR_MARKER` where the hardware cursor belongs; the marker is an escape that
occupies no cells.

`FocusFlag::default()` has no forwarding target. `FocusFlag::forwarding(target)`
starts with its own false flag; each `set` first records that flag, then
synchronously sets the target's flag, even when the value is unchanged. Changing
only the target does not change the parent getter. A composed input can therefore
retain independent parent focus while forwarding cursor placement to its editor.
The retained target still receives focus after removal from a container.
`FocusFlag::forwarding_all(targets)` uses the same owner for multiple targets,
forwarding each update synchronously in supplied order. The parent flag remains
independent of direct changes to any target.

`Component` has `std::any::Any` as a supertrait, so whoever holds a component handle can
downcast the borrowed component to its concrete type, as a tool renderer does to update
the `Text` it returned for a row last time instead of creating another.

Every method takes `&self`: a component is a shared object that a callback running inside
one of its methods, such as a render that moves focus or invalidates the writer, reaches
at once. A component keeps the state it changes in `Cell`s and `RefCell`s, borrows it only
for the statement that needs it and never across a call into the writer, a callback or
another component.

A `Container` renders its children in order and concatenates their lines. Children
are shared handles (`Rc<dyn Component>`), so whoever keeps a handle can keep editing the
child through its own interior state and the container shows the edit. The same child may
appear twice. `children` returns the array itself (`Rc<RefCell<Vec<_>>>`), so whoever
keeps that handle edits the array the container renders, and `set_children` makes another
array the one it renders. `remove_child` removes the first occurrence by identity and
ignores a child that is not present. `clear` replaces the children with a new empty
array, which a handle kept from before no longer reaches. Borrow the array only for the
statement that edits or reads it, because the container borrows it itself to render,
invalidate, add and remove.

`render` and `invalidate` walk the array held when they start, by position, while the
position is below that array's current length. `walk` gives the same traversal to a
caller, fetching each child when the walk reaches it. A child added to that array is visited
only if the walk reaches its position. After `clear`, or `set_children` with another
array, the running walk keeps the array it started on and visits nothing added to the
new one. A removal at or before the position being visited shifts the later children
back, so the next child is skipped. `invalidate` invalidates each child its walk reaches
before it returns, including one that is rendering or handling input, once per position
the walk visits: a child listed twice is invalidated twice when no edit during the walk
changes the array.

`TruncatedText` shows the first line of its text, truncated to the viewport, between
`padding_y` blank rows. Horizontal padding is `padding_x` on each side but never
more than half the width (rounded down), and the first line is cut to the columns
that remain. Those can be none, as at width 2 with padding 1, and then the line is
only spaces. A zero-width viewport renders empty lines. Every line, blank rows
included, is padded with spaces to exactly the viewport width.

```rust
use std::rc::Rc;

use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Component, Container, TruncatedText};

let title: ComponentHandle = Rc::new(TruncatedText::new("Hello world\nignored".into(), 1, 0));
let container = Container::new();
container.add_child(Rc::clone(&title));
container.add_child(Rc::clone(&title));

let lines = container.render(14);
assert_eq!(lines, [" Hello world  ", " Hello world  "]);

container.remove_child(&title);
assert_eq!(container.render(14), [" Hello world  "]);
```

## Overlays

`TUI::show_overlay` retains a component with optional caller-editable
`OverlayOptions` and returns its `OverlayHandle`. See [retained overlays](overlays.md)
for placement, focus, visibility and terminal error ordering.

## Terminals

`Terminal` covers starting with input and resize callbacks, writing, dimensions,
cursor movement and visibility, clearing, the window title, the progress indicator,
draining stale input and registering a handler for native output failures. Effects
return `io::Result` and carry the device's error unchanged. `drain_input` takes
optional limits and returns a future that owns its state, so a scheduled frame may
borrow and render to the same terminal while the drain is pending; an adapter applies
its own defaults, 1000 ms at most and 50 ms idle, when they are omitted.
`on_output_error` returns the operation that removes the handler, and an adapter
without native output failures returns one that does nothing.

`TuiRuntime` carries the same registration for the host's own output, so a terminal
writer and the host that reports its failures each name their own producers.

## Editors and completion

The concrete [Editor](editor.md) provides multiline prompt editing and rendering.
It exposes component, input and focus capabilities and implements `EditorComponent`.

An `EditorComponent` is a component that must also handle input. Its text accessors
are required. History, insertion, the completion provider, padding and the visible
item count are optional: each returns `None` when unsupported and `Some(())` once
carried out. Expanded text is optional too: `get_expanded_text` returns `None` when
unsupported and `Some(text)` with the markers expanded otherwise, so `get_text` is
the fallback. The submit and change callbacks are shared `TextCallback` slots read and
replaced through `&self`, and a retained earlier callable stays callable after
replacement; the border colour is an optional getter and setter. Custom editors keep
their state behind interior mutability.

An `AutocompleteProvider` suggests items for lines and a UTF-8 byte cursor, borrowing
a caller-supplied signal. Suggestions and optional command argument callbacks return
fallible futures. `None`, an empty list and a nonempty list remain distinct contract
results; a provider without an opinion on explicit file completion returns `None`.
See [completion](completion.md) for the command, direct-path and attachment
provider, its host operations and shared fuzzy ranking.

## Host terminal colours

`terminal_theme` builds OSC 10/11 queries for default foreground and background
colours, optionally followed by OSC 4 queries for the 256 palette entries.
Its default and palette reply parsers accept BEL or ST termination, `rgb:` or
three-channel `rgba:` operands, and packed `#` hexadecimal colours. Channels
contain one to four hexadecimal digits and round to the nearest eight-bit value.
Malformed replies return `None`, including non-ASCII packed hex operands.
The module parses supplied replies; it sends no queries and retains no host state.
