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

## Fuzzy matching

`fuzzy_match` returns the native match and score. `fuzzy_filter` accepts any
borrowed item slice and a borrowing `FnMut` getter, with no serialization bounds.
Empty and whitespace-only queries call no getter. Other queries project each item
once in input order; native ranking returns checked positions into the original
slice. Returned references retain item identity and stable score ties.

The slice's membership stays fixed during the call. Interior-mutable payloads
remain live: an earlier getter may edit what a later getter reads. Getters are
never retained, including on error. Cancellation policy stays with the caller.

## Metadata

`frontmatter::parse_frontmatter` returns a complete `FrontmatterValue` tree,
including scalar and sequence roots, nested nulls, signed zero, NaN and both
infinities. Numbers cross a typed floating-point boundary, not a JSON number.
Mappings enumerate canonical array-index keys first, then other keys in insertion
order. YAML parsing remains entirely native.

`strip_frontmatter` parses before returning the body. Both operations preserve
native parser causes and coordinates. Ordinary text is not trimmed; a present
header selects the native trimmed body and newline normalization.

## Conversation and output

`convert_to_llm` delegates application-message projection to the native session
owner. `compaction::utils::serialize_conversation` uses its existing transcript
serializer, including tool-result clipping and argument JSON spelling.

`tools::truncate` provides head and tail selection with complete accounting,
scalar-aware line clipping and size formatting. Missing limits select the host's
defaults: 2000 lines, 51200 bytes and 500 line characters. Size formatting accepts
the native floating-point domain, including signed zero and nonfinite values.

## Paths and installation identity

`tools::path_utils::{expand_path, resolve_to_cwd, resolve_read_path}` use the
native host's home and filesystem, not the component's environment. They keep
the native authored-path policy and existing-spelling fallback order.

`config::version(&api)` reads the application installation identity supplied at
host composition. It does not use the component package version. Native hosts
supply this value to `artifact_factory(path, installation_version)`.
