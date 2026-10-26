# Native tool operations from a component

`ExtensionAPI::create_read_tool_definition(cwd, options)` returns a retained native
read definition. Construction does not read a file. Its metadata, schema,
execution and renderers come from the native tool owner. See [read](../tools-read.md)
for its file, image and output rules.

## Execute without an extension context

`ExtensionAPI::create_read_tool(cwd, options)` returns a `Tool`. Its `execute`
callback takes the call ID, JSON arguments, optional signal and optional progress
callback. It does not read a command or event context, even when called from one.

The executable keeps the native name, label, description, parameter schema,
preparation and execution mode. The parameter schema remains the shared
`ParameterSchema<SchemaCode>`, not a JSON string. Prompt fields and renderers
belong to definitions only.

## Wrap and register a read definition

```rust
use std::rc::Rc;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ReadToolOptions};

fn register_read(api: &ExtensionAPI, cwd: String) -> ExtensionResult<()> {
    let mut read = api.create_read_tool_definition(cwd, ReadToolOptions::default())?;
    let native = read.render_call.take().ok_or("read call renderer absent")?;
    read.render_call = Some(Rc::new(move |args, theme, context, emitter| {
        let mut expanded = context.clone();
        expanded.expanded = true;
        native(args, theme, &expanded, emitter)
    }));
    api.register_tool(read)
}
```

Pass the received render context when delegating. Its opaque `native_row` keeps
the original native environment, state and invalidation capability. Public row
observations may be changed on a clone. Call arguments, result options and the
supplied theme remain independent of those observations.

The previous component is exactly the one passed by the wrapper: a native
component, an authored replacement or none. Returning a native component lets
its next rendering reuse that native object. Wrapping it in another `Component`
does not pretend the wrapper is the native object. Calling a native renderer
without `native_row` returns `native tool renderer requires a native row` and
produces no component.

Result renderers receive `ToolRenderResultLike`. Its flattened `result` keeps the
existing `AgentToolResult` content, details and termination fields; `is_error`
keeps the independent rendering observation, including absence. Execution still
returns `AgentToolResult`.

## Replace file observations

`ReadToolOptions` has independent `auto_resize_images` and `operations` fields.
The image fitting default is enabled. Omitting `operations` uses native file
access. Supplying `ReadOperations` replaces only these observations:

| Callback | Result |
| --- | --- |
| `access(absolute_path)` | Success or a diagnostic that stops the read. |
| `read_file(absolute_path)` | Original bytes, as `Vec<u8>`. |
| `detect_image_mime_type(absolute_path)` | Optional MIME text; omitting the callback, returning `None` or returning an empty string selects text. |

Callbacks return `OperationFuture` with `DiagnosticErrorInfo` failures. The native
owner resolves paths against the construction directory, checks cancellation,
orders the observations, decodes text and applies image policy. The active model
is read from the supplied execution context, not captured during construction or
replaced by a later runner binding. Image fitting is independent of model image
support: a text-only model adds the native warning without disabling fitting.

A pre-aborted signal starts no file observation. An abort while an observation
is pending settles as `Operation aborted`; a later callback failure cannot replace
that outcome. Caller-defined signal ports retain their subscription for the call.

The definition and its callbacks may outlive the factory call and the callback
that constructed it. Keep the returned definition or its callback clones for as
long as they are needed; dropping the last consumer releases their native
resources. These operations require the native component host and a
`wasm32-wasip2` component. The in-process controlled adapter does not simulate
native factories.

## Replace write effects

`ExtensionAPI::create_write_tool(cwd, options)` returns a context-free executable;
`create_write_tool_definition(cwd, options)` returns a registerable definition with
its native metadata and renderers. Neither constructor performs file work or reads
a model. See [write](../tools-edit.md#write-tool) for native behavior.

`WriteToolOptions::operations` optionally supplies `WriteOperations`:
`mkdir(parent)` creates parents recursively, and `write_file(absolute_path, content)`
receives the original string. Both return `OperationFuture<'static, ()>` with
`DiagnosticErrorInfo` on failure. Omitting operations uses the local filesystem.

The native owner resolves paths, waits for mkdir to succeed before starting write,
and reports UTF-8 bytes, not characters. A failed mkdir starts no write. These
calls use the same native per-file mutation queue as other file-changing tools.
Pre-abort starts no effect; cancellation while an effect is pending stops the
remaining stages but cannot undo an effect that already completed.

## Replace edit effects

`create_edit_tool` and `create_edit_tool_definition` retain the native edit's
argument preparation. Its callback accepts `&mut Value`: parsing string-valued
`edits` updates retained assistant arguments, while folding a legacy `oldText` /
`newText` pair changes only the returned preparation. Validation uses that returned
value; saved history and later model requests keep the retained form. Preparation
performs no file observations or writes.

`EditToolOptions::operations` optionally supplies
`EditOperations`: `access(absolute_path)`, `read_file(absolute_path)` returning
original bytes, and `write_file(absolute_path, content)` receiving the native
replacement text. Omitting operations uses the local filesystem.

Callbacks return `OperationFuture` with the original `DiagnosticErrorInfo`, not a
display string. In particular, edit access formats a supplied text or numeric
`code`; without a code it keeps the original message. Read and write failures stop
at their reached stage. Matching, validation, diff construction, BOM and line-ending
policy remain with the [native edit owner](../tools-edit.md).

## Replace search observations

`create_find_tool` / `create_find_tool_definition` and `create_grep_tool` /
`create_grep_tool_definition` return the native factories' executable and
registerable forms. Construction does not inspect files or acquire programs.
See [search tools](../tools-search.md) for native search and output behavior.

`FindToolOptions::operations` supplies `exists(absolute_root)` and
`glob(pattern, absolute_root, FindGlobOptions)`. Glob options retain the ordered
ignore patterns and effective floating-point limit. Return ordered relative or
absolute paths; the native finder resolves them and selects output and notices.

`GrepToolOptions::operations` supplies `is_directory(absolute_root)` and
`read_file(absolute_file)` returning context text. Matching still uses native
ripgrep, including when these observations are replaced. Root errors and unreadable
context retain the native search owner's handling.

All four callbacks return `OperationFuture` with `DiagnosticErrorInfo` on failure.
The host retains them independently of the constructor and invokes them without a
callback-table borrow. Omitted operations select native filesystem access.

Native applications install `ToolConstruction` with `set_tool_construction`
before loading extension factories. It carries the same `Arc<ToolsManager>` and
binary directory used by the application's own tools. The SDK supplies these
services during resource loading and retains them on reload; the component runtime
never discovers a default directory or creates another manager. An unconfigured
host returns a construction error.

## Native helpers

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
