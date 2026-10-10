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
endpoints of the current logical line. Character deletion uses graphemes of the cursor-local prefix or suffix.
Undo restores text and cursor, coalescing consecutive nonwhitespace typing.

Change callbacks run after edits. Submission clears the buffer and undo stack
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
a cursor marker. Both scroll labels are clipped before border styling. Caller
styling may add visible text. Display tabs expand outside recognized escapes;
escape-safe cursor decoration never changes the stored edit position.

With available history, Up starts browsing when the editor is empty. While
browsing, Up and Down select admitted older and newer entries at the first and
last visual rows. Unavailable selections leave the prompt unchanged. Outside
these history-selection branches, they move within the current prompt. Callers add history
explicitly; submission does not add it. Page keys use the live terminal row count;
visual movement uses terminal-cell columns. Literal character jumps search stored
text at scalar boundaries, separately from visual movement.

Directional kills retain deleted text for yank; yank-pop rotates the ring after
notifying the deletion, then notifies the replacement. Reentrant edits during yank
notification invalidate that insertion's later replacement eligibility.

Bracketed paste and completion are not delivered here. Selection styling is
retained, not invoked. See
[text helpers](text.md), [keybindings](keybindings.md) and
[rendering](rendering.md) for shared behavior.
