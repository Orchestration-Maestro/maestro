# Native helpers

Terminal text operations in `maestro_extensions_wasm::utils` receive an
`&ExtensionAPI` explicitly. They synchronously call the installed host's existing
terminal text owner; the component contains no text algorithms.

- `visible_width` returns terminal cells, not bytes. The host handles Unicode
  clusters, ANSI escapes and tabs.
- `normalize_terminal_output` normalizes Thai and Lao vowels, leaving ordinary
  newlines and tabs unchanged.
- `extract_ansi_code` takes a UTF-8 byte offset and returns the exact escape and
  its byte length. A position without a complete supported escape returns `None`.
- `is_whitespace_char` and `is_punctuation_char` report whether any supplied
  scalar belongs to the host's set. Whitespace includes U+FEFF, not U+0085.

Calls return the host's error rather than choosing a component-local fallback.

## Columns and callbacks

`wrap_text_with_ansi`, `truncate_to_width`, `slice_by_column`, `slice_with_width`
and `extract_segments` use terminal cells and keep native cluster and style
handling. Truncation defaults to `"..."` with no padding. Strict slicing excludes
a whole grapheme that crosses the right edge; segment strictness applies only to
the after side. Width-bearing results report the actual selected cells.

`apply_background_to_line` asks the native owner to pad the complete line, then
calls the supplied `FnOnce` exactly once with that string. It does not clip an
over-width line. The callback may borrow local state and runs before the helper
returns; neither a successful call nor an error retains it.

## Keys

`keys::Key` provides the named literals and forwards every modifier constructor
to the native key owner. `super_key` is the Rust spelling of the super modifier.
Key identifiers remain open strings.

Matching, parsing, printable decoding, repeat and release checks use native
terminal input handling. Optional keys and scalars return `None` when absent.
The enhanced keyboard flag is shared with the native host and other API handles;
setting it is a process-wide effect, not component-local state.
