# Session documents

`maestro-export` serializes retained session trees to HTML with a **supplied,
prebuilt offline viewer bundle**. It does not supply or compile the viewer.
The document template is in
[`assets/document/template.html`](../crates/maestro-export/assets/document/template.html).

## Entry points

`export_session_to_html` accepts a
[`ReadonlySessionManager`](../crates/maestro-session/src/session_manager/types.rs)
in a `RefCell`, optional `ExportState`, optional `ExportOptions` and an
`ExportOperations` adapter. It uses retained entries, including inactive branches,
and the selected leaf. It borrows the session only to read it, so a renderer may
change the session; the document keeps the entries read before rendering.
`ExportState` borrows a prompt and tool descriptors; omitted fields stay omitted,
and empty strings/arrays stay present. Tool metadata contains name, description
and parameters. Its input presence check and document write need a native file
system, so a browser build cannot write a document.

Native `export_from_file` opens through
[`SessionManager`](sessions/session_manager.md), omitting state and custom
rendering even when options contain a renderer. Session opening can recover or
rewrite its input; this is not a read-only file operation.

`None` omits options. `Some("out.html".into())` selects an output path.
`ExportOptions` also selects a theme name and a `ToolHtmlRenderer`.
An absent or empty output path selects
`<app_name>-session-<basename without final .jsonl>.html`; basename semantics
belong to the [path owner](../crates/maestro-path/src/lib.rs). The adapter supplies the application name.
The return value retains an explicitly supplied path's spelling and is returned
only after writing succeeds.

## Bundle and theme effects

`ExportOperations` supplies asset reads and theme observations.
Native `NativeExportOperations` reads, in order:
`template.html`, `template.css`, `template.js`, `vendor/marked.min.js` and
`vendor/highlight.min.js`, from its supplied directory. Native decoding replaces
invalid UTF-8. The export checks the input's presence and writes the completed
document directly through the native file system, overwriting an existing output
without creating parents.

The serializer resolves colors and explicit backgrounds for CSS properties,
then observes both again for background slots. Explicit page, card and info
values override their derived counterparts independently, including resolved
empty strings. Resolution and brand projection belong to the
[theme owner](theme.md#resolved-colors-for-export) and
[brand owner](brand.md), not the serializer.

The payload uses [canonical JSON](json.md), UTF-8 and standard base64.
Each fixed slot replaces its first occurrence in the original template or CSS;
inserted values are literal and never scanned for more slots. Duplicate and
unknown slots remain untouched.

## Custom output and errors

`ToolHtmlRenderer` borrows raw call arguments and result content/details.
Assistant calls other than `bash`, `read`, `write`, `edit` and `ls` are eligible.
Nonempty call output replaces the call-ID record. A present result merges with
call output, replacing both optional result fields; an absent result leaves the
record unchanged. Empty result strings remain present. IDs are own string keys,
including reserved-looking names. An empty resulting map is omitted without
filtering session entries.

`ExportError` distinguishes an in-memory session, an unpublished session and a
missing file from I/O, theme, JSON and direct renderer failures. Selected invalid
session fields fail only when custom traversal reaches them. Direct callback
errors propagate rather than triggering a standard-renderer fallback.

The serializer does **not** sanitize supplied scripts, CSS or custom renderer
HTML. Use trusted bundle/render providers. Base64 protects the session-data
carrier; it is not a sanitization guarantee for the rest of the document.

The exported Markdown viewer sanitizes link and image targets. It removes ASCII
control characters from a target, then keeps it only when it has no scheme (a
relative target) or names `http`, `https`, `mailto`, `tel` or `ftp`, ignoring
case. Any other target, such as `javascript:`, `vbscript:`, `data:` or a custom
scheme, leaves a link as its label text and an image as its alternative text.
The emitted target is the one with control characters removed.

## Terminal text and standard tool rendering

`ansi_to_html` escapes literal text once and converts ASCII SGR sequences into
inline color, background, bold, dim, italic and underline spans. Other escape
sequences stay literal. `ansi_lines_to_html` resets styles per line and emits
adjacent `ansi-line` divs without separators; only an empty converted line becomes
`&nbsp;`. Numeric colors use the [theme palette](theme.md#resolved-colors-for-export),
not brand roles.

`create_tool_html_renderer` implements the existing `ToolHtmlRenderer` contract
through [tools-owned renderer bindings](tools.md#renderer-projections). Supply live lookup,
the live theme handle, cwd and host environment. A renderer resolves that handle when it
reads a color, so one that never reads it still renders without a published theme, while
one that reads it keeps the standard rendering. Each call ID owns a `ToolRendererState`;
custom state uses `Default`, and JSON state starts as an empty object. Width defaults
to 100 only when omitted. The adapter retains arguments, shared mutable state and
independent call/result component slots per call ID. Missing arguments stay missing.
It executes no tool.

Results render collapsed before expanded, retaining each returned component before
calling its fallible `Component::try_render` method. Composite components propagate
child rendering errors; reconstruction from a document supplies no termination hint.
Result-edge blank lines are
trimmed after SGR removal using [text whitespace](text.md); call padding and interior
blank lines remain. Expanded output stays present even when empty; collapsed output
is included only when nonempty and different. Ordinary lookup, callback, component
and typed-input failures return absence without rolling back earlier mutations.
Direct user-supplied renderer errors still propagate through the serializer.

Lookup returns `ToolRendererBinding`, so native factories and custom tools with
different parameter, details and state types share one registry. The adapter
prepares the selected result binding once per operation. Typed callbacks decode present
details once and share that value across collapsed and expanded phases; raw callbacks
borrow persisted JSON directly, so sparse builtin metadata does not require decoding
an execution record. Both phases use the same per-ID typed state owner, available via
`ToolRenderContext::renderer_state` for native delegation with `with_state`.

For example, a caller can populate one binding registry with
`ToolDef::renderer_binding()` for native tools and
`ToolRendererBinding::from_definition(&custom_definition)` for typed custom tools,
then supply `get_tool_definition: |name| Ok(bindings.get(name).cloned())`.
Use `ToolRendererBinding::from_raw(&custom_definition)` for a custom raw slot
whose execution details need no decoder. Neither lookup nor preparation executes
the tool. Both result phases retain the selected binding even if callbacks change
the registry; subsequent operations perform a new lookup.
