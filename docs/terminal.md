# Terminal text

Use the provided utilities to ensure lines fit:

```rust
use maestro_tui::{visible_width, truncate_to_width, wrap_text_with_ansi};
assert_eq!(visible_width("Hello 界"), 8);
assert_eq!(truncate_to_width("Hello world", 8, None, None),
           "Hello\x1b[0m...\x1b[0m");
assert_eq!(wrap_text_with_ansi("hello world", 6), ["hello", "world"]);
```

Both visible_width() and truncate_to_width() correctly handle ANSI escape codes:

- visible_width() ignores ANSI codes when calculating width
- truncate_to_width() preserves ANSI codes and properly closes them when truncating

```rust
use maestro_tui::{visible_width, truncate_to_width};
let text = "\x1b[31mHello world\x1b[39m";
assert_eq!(visible_width(text), 11);
assert_eq!(truncate_to_width(text, 8, None, None),
           "\x1b[31mHello\x1b[0m...\x1b[0m");
```

The cell operations retain grapheme boundaries, supported CSI/OSC/APC sequences
and byte indexes. Column slicing does not expand tabs; visible width and
truncation count each tab as three cells. Thai/Lao AM output normalization changes
terminal output only, not logical editor content.
