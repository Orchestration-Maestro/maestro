# Tool output helpers

`maestro-tools` currently delivers pure output selection. Read execution is not
part of this interface.

## Limits and selection

[`output::truncate`](../crates/maestro-tools/src/output/truncate.rs) owns the limits
and accounting. `TruncationOptions::default()` independently selects 2000
newline-separated entries and 51200 UTF-8 bytes. Explicit zero stays zero;
`usize::MAX` leaves the corresponding budget unbounded for native-sized text.
A final newline adds an empty entry. CR, BOM, NUL, tabs and ANSI bytes remain text.

`truncate_head` keeps complete leading entries. An oversized first entry returns
empty content with `first_line_exceeds_limit`, even with a zero line quota.
`truncate_tail` keeps trailing entries, or a UTF-8-aligned suffix when the final
entry alone exceeds the byte budget. That partial entry reports byte truncation.
A zero line quota selects no entries; a zero byte quota can retain an empty
partial tail entry. Consequently `output_lines` is not necessarily the split
count of the returned text.

Both operations return original/output counts, applied budgets and selection
flags. Serialization uses camelCase fields, with `truncatedBy` explicitly null
for fitting input. Byte counts include separators between retained entries.

```rust
use maestro_tools::{TruncationOptions, truncate_head};

let result = truncate_head("first\nlast", TruncationOptions {
    max_lines: Some(1),
    ..TruncationOptions::default()
});
assert_eq!(result.content, "first");
assert_eq!(result.total_lines, 2);
```

## Line clipping and size text

`truncate_line` uses a scalar-character budget, defaulting to 500, and appends
`... [truncated]` only on overflow. Combining marks and ZWJ characters count as
separate scalars; this is not terminal-cell or grapheme clipping.

`format_size` selects B below 1024 bytes, KB below 1048576, and MB thereafter.
KB/MB have one decimal: 1280 is `1.3KB`, 1048575 is `1024.0KB`, and 51200 is
`50.0KB`.

These helpers require no filesystem or runtime and build on native and
`wasm32-unknown-unknown` targets. Their typed inputs have no separate validation
or I/O failure path. See [output libraries](libraries/tools-output.md) for the
serialization and number-formatting dependencies.
