# Multiline editor

`Editor` is the retained multiline editable terminal component. Construct it
with a writer, border and selection styling, and optional side padding:

```rust
use maestro_tui::{Component, Editor, EditorOptions, EditorTheme, TUI};

fn edit(tui: &TUI, theme: EditorTheme) -> Vec<String> {
    let editor = Editor::new(tui, theme, EditorOptions::default());
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

Rendering wraps at word boundaries and keeps a scrolling viewport sized from
live terminal rows. At positive wrapping widths, an indivisible overwide grapheme stays intact;
a narrow viewport clips only display text. Width zero emits empty rows without
a cursor marker. Both scroll labels are clipped before border styling. Caller
styling may add visible text. Display tabs expand outside recognized escapes;
escape-safe cursor decoration never changes the stored edit position.

History, vertical and word navigation, kill/yank, bracketed paste and completion
are not delivered here. Selection styling is retained, not invoked. See
[text helpers](text.md), [keybindings](keybindings.md) and
[rendering](rendering.md) for shared behavior.
