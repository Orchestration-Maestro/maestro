# Multiline editor

`Editor` is the retained multiline editable terminal component. Construct it
with a writer, border and selection styling, and optional side padding:

```rust
use maestro_tui::{Component, Editor, EditorOptions, EditorTheme, TUI};

fn edit(tui: &TUI, theme: EditorTheme) -> Vec<String> {
    let editor = Editor::new(tui, theme, EditorOptions::default());
    editor.add_to_history("previous prompt");
    editor.set_text("first\nsecond");
    editor.insert_text_at_cursor("!");
    editor.render(40)
}
```

Replacement and programmatic insertion normalize line endings and expand tabs
for storage. Raw typed chunks remain literal. Getters return owned text, logical
lines and a byte cursor. Left and Right cross logical lines at their endpoints; Home and End select
endpoints of the current logical line. Character deletion removes the cursor-local edit unit: a grapheme of the prefix or suffix, or an owned marker.
Undo restores text and cursor, coalescing consecutive nonwhitespace typing.

Change callbacks run after edits. The submit and change callbacks are shared
slots: `on_submit` and `on_change` return the current callable, so an earlier alias
stays callable after `set_on_submit` or `set_on_change` replaces or removes it.
The `EditorComponent` implementation of `Editor` reads and writes the same slots,
border colour, history, padding and item maximum. Submission clears the buffer and undo stack
before change notification, then reads the current submit callback. Callbacks
may synchronously edit or replace callbacks; retained callable aliases survive
replacement. Disabled submission leaves text untouched.

Finite side padding is floored and clamped at zero, retained without an upper
bound, and bounded to the available cells only during rendering. Nonfinite
padding uses zero.

Rendering wraps at word boundaries and keeps a scrolling viewport sized from
live terminal rows. Wrapping segments visible graphemes after removing recognized
escapes, retaining their original byte ranges. Escapes inside a grapheme stay with
it; between graphemes they travel with the following one, or the preceding one at
line end. Supplied segments refine only at these boundaries. An indivisible
overwide grapheme stays intact; a narrow viewport clips only display text. Width zero emits empty rows without
a cursor marker. Both scroll labels are clipped before border styling and keep the text helper's reset sequences. Caller
styling may add visible text. Display tabs expand outside recognized escapes;
escape-safe cursor decoration never changes the stored edit position.

With available history, Up starts browsing when the editor is empty. While
browsing, Up and Down select admitted older and newer entries at the first and
last visual rows. Unavailable selections leave the prompt unchanged. Outside
these history-selection branches, they move within the current prompt. Callers add history
explicitly; submission does not add it. Page keys use the live terminal row count;
visual movement uses terminal-cell columns; a column beyond the width of a final row
lands at the line end, after any trailing zero-width text. Literal character jumps search stored
text at scalar boundaries, separately from visual movement.

Directional kills retain deleted text for yank; yank-pop rotates the ring after
notifying the deletion, then notifies the replacement. An immediate nested
yank-pop requested from the deletion notification is ignored while the deleted
insertion remains ineligible. Reentrant edits during yank
notification invalidate that insertion's later replacement eligibility.

Bracketed paste is buffered until its first end marker; input after that
marker is handled as ordinary input and a new start marker discards an
unfinished paste. A paste decodes `ESC [ code ; 5 u` control letters, then
converts CRLF and CR to LF, expands tabs to four spaces and drops other
controls below U+0020. A paste starting with `/`, `~` or `.` gains one leading
space after an ASCII letter, digit or underscore. A nonempty raw payload is one
undoable edit that also leaves history browsing, even when filtering leaves
nothing to insert; an empty frame changes nothing.

A paste above 10 lines or 1000 Unicode scalars is stored and replaced by one
marker, `[paste #ID +N lines]` or `[paste #ID N chars]`; the line label wins
when both limits apply. Marker identifiers count stored pastes from 1 and
restart after submission. Stored pastes survive undo, deletion, replacement and
history, so a marker typed again later still names its content. A marker whose
identifier names a stored paste, including leading zeros, is one edit unit for
character, word and vertical movement and for deletion, and wrapping may split
it across rows without splitting the unit; other marker-like text is ordinary
text. When a movement or deletion starts at a marker boundary, graphemes that
intersect the marker (a combining mark or a joiner after it, or a prepend
mark before it) join its unit; a literal character jump may still place the
cursor inside a marker. `get_expanded_text`
and submission replace each canonical marker (leading-zero spellings stay unexpanded) with its content, one literal pass
per stored paste in creation order, so text inserted by a pass is eligible only
for later passes.

```rust
use maestro_tui::{Editor, tui::InputHandler};

fn paste(editor: &Editor) -> String {
    let lines = (1..=11).map(|n| format!("line {n}")).collect::<Vec<_>>();
    editor.handle_input(&format!("\x1b[200~{}\x1b[201~", lines.join("\n")));
    assert_eq!(editor.get_text(), "[paste #1 +11 lines]");
    editor.get_expanded_text()
}
```

## Completion

`set_autocomplete_provider` installs an `AutocompleteProvider` whose cancellation
signal is `maestro_cancellation::Cancellation`; replacing it cancels admitted
completion and clears the menu. Typing `/` when the first line before the cursor is
empty or only `/` (ignoring surrounding Unicode whitespace) requests suggestions. So
does typing `@` or `#` when it is the first character before the cursor or follows an
ASCII space or tab. Any other typed chunk requests them when it contains an ASCII letter,
digit, `.`, `-` or `_` and either, on the first line, the text before the cursor starts
with `/` after leading Unicode whitespace, or, on any line, it ends in an `@`/`#` token
that starts the text or follows Unicode whitespace. Backspace and forward delete retrigger from the same two contexts, or refresh
an open menu in the mode it was requested in. Tab requests
regular completion for a first-line slash command without a literal space and forced
completion otherwise. A forced request first asks the provider whether file completion
applies and returns without cancelling anything when it says no. An automatic request
waits 20 ms for further typing while the text before the cursor ends in an `@` token
(a quoted `@"` token keeps waiting through whitespace) or an unquoted `#` token, either
one starting the text or following an ASCII space or tab; every other request, including
Tab and forced requests, starts at once.

Requests run on the host's `spawn_local`, one at a time: a newer request waits for the
running one to settle, superseded waiting requests are skipped, and the newest starts
from the buffer and provider live at that moment. Cancellation is cooperative, so a
provider that ignores its signal still occupies the slot. A result is applied only
when its signal is live, it belongs to the newest started request, and the text, logical
line and byte column still equal those at its start; undo and cursor movement do not
cancel a request, so returning to the same text and cursor keeps its result eligible.
A provider failure leaves the menu as it was, lets the next waiting request start and
is returned unchanged to the host. A current `None` or empty result clears the menu
and requests a frame; a stale one returns without clearing.

Only an explicit forced request with exactly one item applies at once; otherwise
the items fill a menu below the editor, rendered by [`SelectList`](widgets.md#selectlist)
in the editor's side padding; the menu takes the configured item count current when it
is built and keeps it until the next result replaces the menu. With a non-empty prefix
the selection starts at the item whose value equals it, else the first whose value starts
with it (case-sensitive, values only, provider order kept); otherwise it starts on the
first item. While the menu is open, copy and undo keep their usual meaning, then
cancel closes it, up and down move through it, Tab applies the selection, and confirm
applies it. Afterwards a non-slash prefix ends the edit with one change notification,
and so does a menu that an application callback closed. When the menu still
carries a slash prefix, input continues as ordinary input: submission follows the
disabled-submit rule (with submission disabled no notification follows the application),
and a rebound confirm key takes its ordinary action with its own undo snapshot. Every
application takes one undo snapshot, and the hardware cursor marker is withheld while
the menu is open.

Selection styling comes from the theme. See
[text helpers](text.md), [keybindings](keybindings.md) and
[rendering](rendering.md) for shared behavior.
