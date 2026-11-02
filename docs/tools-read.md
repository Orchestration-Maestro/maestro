# Tool output and image helpers

`maestro-tools` delivers output selection, native streamed-output accumulation,
callable image preparation, generic tool definitions, the native `read` tool and, in
[its own page](tools-search.md), the native `ls` tool. The `write` tool and the file
mutation queue are described in [file-changing tools](tools-edit.md).

## Limits and selection

[`output::truncate`](../crates/maestro-tools/src/output/truncate.rs) owns the limits
and accounting. `TruncationOptions::default()` independently selects 2000
newline-separated entries and 51200 UTF-8 bytes. Explicit zero stays zero;
`usize::MAX` leaves the corresponding budget unbounded for native-sized text.
A final newline terminates the last line without adding an empty entry; empty text
has zero lines. CR, BOM, NUL, tabs and ANSI bytes remain text.

`truncate_head` keeps complete leading entries. An oversized first entry returns
empty content with `first_line_exceeds_limit`, even with a zero line quota.
`truncate_tail` keeps trailing entries, or a UTF-8-aligned suffix when the final
line alone exceeds the byte budget. That partial line reports byte truncation. When
a newline ends the oversized line, the suffix uses the full byte budget and omits
the newline. An omitted final newline reports bytes, not lines, when no line was
omitted. A zero line quota selects no entries; a zero byte quota can retain an empty
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

The selection, clipping and size helpers require no filesystem or runtime and
build on native and `wasm32-unknown-unknown` targets. Their typed inputs have no separate validation
or I/O failure path. See [output libraries](libraries/tools-output.md) for the
serialization and number-formatting dependencies.

## Native streaming

[`output::output_accumulator`](../crates/maestro-tools/src/output/output_accumulator.rs)
provides `OutputAccumulator`, `OutputAccumulatorOptions` and `OutputSnapshot` on
native targets. Construction does not create a file. Append accepts raw bytes;
finish flushes pending UTF-8 once without closing the artifact. Raw decoding uses
the shared [streaming text primitive](text.md). Snapshots observe
selected decoded text without finishing decoding; selection is owned by
[`output::truncate`](../crates/maestro-tools/src/output/truncate.rs).

Streamed line counts include completed lines plus one nonempty open line. Empty
streams have zero lines. For 4,000 newline-terminated lines, bash retains lines
2001 through 4000 and reports `[Showing lines 2001-4000 of 4000. Full output: <path>]`.

A snapshot drops a leading fragment of a trimmed line only when more text follows its
newline; a fragment that is the whole retained tail is the end of an oversized line and
stays. A bash tail notice names the oversized line itself when output ends with a newline.

Overflow artifacts preserve original bytes, including invalid UTF-8 and an initial
BOM that the display decoder removes. A snapshot path identifies an artifact;
completeness requires successful explicit `close_temp_file`. Callers stop after
any I/O error. Close flushes and releases the artifact writer without finishing
decoding; snapshots and last-line counts remain readable. Append after artifact
closure returns an error. Close leaves the artifact on disk and does not request
filesystem durability.

```rust
use maestro_tools::output::output_accumulator::{
    OutputAccumulator, OutputAccumulatorOptions,
};

fn main() -> std::io::Result<()> {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(3),
        ..OutputAccumulatorOptions::default()
    });
    output.append(b"abcdef")?;
    output.finish()?;
    let snapshot = output.snapshot(true)?;
    assert_eq!(snapshot.content, "def");
    let path = snapshot.full_output_path;
    output.close_temp_file()?;
    assert_eq!(output.get_last_line_bytes(), 6);
    assert_eq!(output.snapshot(true)?.content, "def");
    if let Some(path) = path {
        assert_eq!(std::fs::read(&path)?, b"abcdef");
        std::fs::remove_file(path)?;
    }
    Ok(())
}
```

## Inline image preparation

[`utils::image_resize`](../crates/maestro-tools/src/utils/image_resize.rs) owns
independent defaults: width and height 2000, base64 byte cap 4718592, and first
JPEG quality 80. It retains authored data and MIME when oriented dimensions fit
inclusive bounds and the authored text is strictly below the byte cap. Otherwise
it scales width before height, with positive half-up rounding and a one-pixel
minimum for positive bounds; a zero dimension bound returns no result.

At each geometry it tries PNG, then distinct JPEG qualities in first-occurrence
order `[requested, 85, 70, 55, 40]`, returning the first payload strictly below the
cap. JPEG is skipped when either axis exceeds 65535 pixels. If no encoding fits,
each non-unit axis shrinks to the floor of three quarters, clamped to one; after
an exhausted 1×1 candidate the result is absent. Each candidate is sampled from
the oriented original. The dimension note is present only after re-encoding,
including re-encoding at unchanged dimensions; it reports the original/displayed
width ratio to two decimal places.

[`utils::image_convert`](../crates/maestro-tools/src/utils/image_convert.rs) passes
exact `image/png` input through unchanged. Other MIME labels use shared byte
decoding and orientation before PNG encoding; an undecodable input has no result.
The shared decoder accepts standard or URL-safe, padded or unpadded base64 with
ASCII whitespace. Fitting input keeps its authored spelling.

[`utils::exif_orientation`](../crates/maestro-tools/src/utils/exif_orientation.rs)
selects the first primary JPEG/WebP orientation retained by the metadata reader,
including optional WebP EXIF prefixes and padded chunks. A retained unreadable or
out-of-range value leaves pixels unchanged. Metadata decoding follows the
[EXIF reader](libraries/tools-output.md#image-libraries).
The eight orientations use `image::imageops` flips and integer quarter-turns.
Pixel decoding, Lanczos3 resampling and encoding are delegated to the
[image libraries](libraries/tools-output.md#image-libraries).

```rust
use maestro_models::ImageContent;
use maestro_tools::utils::image_resize::{ImageResizeOptions, resize_image};

let image = ImageContent {
    data: "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".into(),
    mime_type: "image/gif".into(),
};
let result = resize_image(&image, ImageResizeOptions::default()).unwrap();
assert_eq!((result.width, result.height), (1, 1));
assert!(!result.was_resized);
```

On native targets,
[`utils::mime`](../crates/maestro-tools/src/utils/mime.rs) opens the supplied path
and takes one read of at most 4100 bytes, ignoring its extension. Its supported
results are JPEG (excluding JPEG-LS), GIF, WebP and sampled PNG without an animation
header before image data. Sampled chunk-header decisions do not validate a whole
PNG. BOM/ID3 prefixes retain the detector's 256 re-entry ceiling. Unsupported
content returns absence; file opening/reading failures remain native I/O errors.
Pure image operations build on native and browser targets; the file opener is
native-only.

## Tool definitions

[`types`](../crates/maestro-tools/src/types.rs) declares `ToolDefinition`: an
executable declaration plus prompt text, a render-shell choice and optional call
and result renderers. The caller context `C`, parameters `P`, details `D` and
renderer state `S` are generic, so the type names no application context. The
renderer records carry one call's state. `ToolRenderContext` borrows that call's raw
argument value, preserving missing versus explicit null (rendering runs before schema validation, so it is independent of the typed
execution parameters `P`) and holds the stable call id, invalidation, previous component,
shared row state, working directory, the execution, partial, expanded, image and error flags,
and a shared `ToolRenderEnvironment`: the runtime, absolute README path, published
theme, syntax highlighter and terminal image support. `ToolRenderResultOptions`
carries the result's own expanded and partial flags. Both renderers return the drawn
component or the failure that kept them from drawing; building a context or calling a
renderer is the caller's job. Result callbacks occupy one typed/raw slot; see
[renderer projections](tools.md#renderer-projections) for persisted details and
shared typed state delegation.

[`wrap_tool_definition`](../crates/maestro-tools/src/tool_definition_wrapper.rs)
turns a definition into an `AgentTool`. Once the arguments decode (rejected
arguments follow `AgentTool::typed`), each execution calls the optional context
factory once; a factory error is the execution's error and the body does not run.
Call id, arguments, signal and progress observer pass through, and result details
keep the three states of `AgentToolResult` (absent, explicit null, value).
`wrap_tool_definitions` wraps in order and keeps duplicates.
`create_tool_definition_from_agent_tool` returns a `Result` and snapshots the
current live description once; a supplied callback failure is returned unchanged.
The converted definition and subsequent wrapping retain that snapshot, with prompt
text and renderers absent. Native and browser builds differ only in whether the stored
default execution callbacks must be `Send + Sync`. The `local` factory and wrapper
namespaces instead retain `Rc` callbacks and caller-local contexts across MIME
detection; see [caller-local construction](tools.md#caller-local-construction).

## Display helpers

[`output::render_utils`](../crates/maestro-tools/src/output/render_utils.rs) holds the
display strings tool renderers share. `ToolRenderResultLike` borrows ordered content
and optional typed or raw JSON details, preserving an executed result's optional termination hint.
Its optional error observation is supplied by the rendering host. Conversion from an
execution result supplies no error flag. `str` returns a borrowed string, an empty string for
an absent or null value and nothing for any other type. `shorten_path` (native only) shows
the home directory as `~` for a path equal to it or below it and leaves a sibling whose
name merely starts with the home path unchanged; a home directory that is empty or `/`
abbreviates nothing, so the path stays as given. It reads the home directory through
[`expand_path`](../crates/maestro-tools/src/path_utils.rs) only for a present path.
`replace_tabs` turns every TAB into three spaces and `normalize_display_text` removes every
carriage return. The tool paths forward to the [text leaf](text.md)'s terminal cleaners.
`strip_ansi` removes what one cached expression matches (OSC strings up to
their first terminator, and CSI-style introducers with parameters and a final byte; `a\u001b[2Qb` becomes `aQb`), and [`utils::shell::sanitize_binary_output`](../crates/maestro-tools/src/utils/shell.rs)
drops C0 controls except TAB, LF and CR and U+FFF9 to U+FFFB. `get_text_output` joins the
cleaned text blocks with LF and, when the terminal cannot show images or they are hidden,
appends one fallback line per image in order. `invalid_arg_text` styles `[invalid arg]`
with the error color.

[`keybinding_hints`](../crates/maestro-tools/src/keybinding_hints.rs) reads the current
keybinding registry: `key_text` joins an action's keys with `/` in registry order,
`key_hint` and `raw_key_hint` follow them with a space and the description, styling the
keys dim and the description muted with the published theme. `expand_hint`, on every
target, follows the muted count of hidden lines with the live expand key and a muted
closing parenthesis. The bash preview uses the same framing through `plain_expand_hint`
when a deferred hint cannot resolve its theme.

The crate-private [`output::render_parts`](../crates/maestro-tools/src/output/render_parts.rs) is shared by the native tool renderers: `into_text`
(reuse the row's previous `Text`, or make one), `paint` and `themed` (style text, carrying a
theme failure to the caller) and `num` and `float` (number spelling). The crate-private `utils::diagnostics` holds
the failure records and the cancellation guard that every native tool's operations share.

## Read tool

[`read`](../crates/maestro-tools/src/read/mod.rs) is native-only.
`create_read_tool_definition` and `create_read_tool` take a working directory and
`ReadToolOptions`: `auto_resize_images` (absent means `true`) and optional
`ReadOperations` (`read_file`, `access` and an optional `detect_image_mime_type`
that defaults to no image). Without operations the tool reads the local
filesystem; a supplied implementation sees the same absolute path in the same
`access`, detect, `read_file` order.

The authored path is resolved when the tool is called, before the returned future
is polled; see [`path_utils`](../crates/maestro-tools/src/path_utils.rs). A leading
`@` is dropped, a bare `~` or `~/` becomes the home directory, and the rest keeps
the spelling it was written with. Relative paths resolve
lexically through `maestro-path`, with no canonicalization. On Windows the
per-drive directories (`=X:` variables) are resolution operands. The process
working directory is read only when `cwd` and those directories cannot anchor a
relative result (elsewhere: only for a relative `cwd`), and the home directory
only for `~`. A cancellation observed after the bytes are read, including while
converting them, still settles the read as cancelled, for results and failures.
`resolve_read_path` tries four candidates, in order: the answer of the shared rule
below, the path as written, the space-replaced path, and the macOS variants of each.
The variants are the narrow-space AM/PM form, NFD, curly apostrophes, and NFD with
curly apostrophes. The first candidate on disk wins; with nothing on disk the rule's
answer stands. So a file whose real name holds one of those spaces opens under that
name while a typed name whose Unicode spaces stand for plain spaces still finds its
file.

Space replacement turns U+00A0, U+2000-U+200A, U+202F, U+205F and U+3000 into a
space. `resolve_existing_path`, which the write, edit and search routes use, keeps the
spelling the path was written with when the file system holds that path, and takes
the space-replaced path when it holds that one instead. Otherwise the deepest
directory the file system holds under the written spelling keeps that spelling and
the rest of the path takes plain spaces, unless a directory of the space-replaced
path reaches deeper; then the whole path takes plain spaces. So unless the
space-replaced path reaches deeper, a new file lands inside the directory named with
one of those spaces, not in a space-replaced twin. A relative path keeps the
spelling of the directory it was resolved against, even when the file system no
longer holds that directory (as after the working directory is removed), and that
directory bounds the search. The whole path is reduced lexically first, and each
parent segment it climbs then drops that directory's last component wherever the
climb stands, while a name written after the climb takes plain spaces. A climb that
would pass the root stops there, so an existing directory under the written spelling
still supplies it.
Expansion through `@` or `~` can make a path absolute; such a path was resolved
against no directory, so nothing in its spelling comes from one. A directory the
file system holds under the written spelling keeps it for absolute paths too.

The signal is checked before each of the three operations and watched while one is
pending, including image processing. An abort settles the read with `Operation
aborted` immediately, and a later operation does not start. A started
operating-system call or blocking-pool resize is not interrupted; it finishes in
the background.

Text is decoded lossily as UTF-8, split on LF with trailing empty entries kept,
and selected with `offset` (one-based) and `limit`, both finite numbers handled
like JavaScript slice bounds: fractions truncate toward zero and a negative end
counts from the end. The caller limit applies first, then
[`truncate_head`](../crates/maestro-tools/src/output/truncate.rs) with the default
2000 lines and 50 KB. The result is plain text with a continuation notice when
lines remain, `details` holds the truncation record only when the default limits
cut the text, and an offset past the last line fails with the line count. When the
first selected line alone exceeds the byte limit the text is a `sed` command for
that line, inserting the authored path verbatim without shell quoting.

A detected image becomes `Read image file [<mime>]`, then the delegated dimension
note, then a note when the active model (read from `ReadContext::model` after
detection) lacks image input, and an image block. Fitting uses the image helpers
above; `auto_resize_images: false` sends the original bytes, and an image that
cannot be fitted yields only the text with the omission line. Base64 encoding, decoding,
resizing and re-encoding run on the runtime's blocking pool, so a large image never
stalls the terminal loop or other tasks. An abort does not wait for that blocking
work to finish. Native detection uses the [MIME sniff](#inline-image-preparation)
and ignores extensions.

## Read rendering

The read definition draws its call and its result into the row's previous `Text`
component, or a new one, returning the same handle. Both read the call's raw arguments to
select the path: `file_path` unless it is absent or null, then `path`. The call renderer
also draws the range from the optional `offset` and `limit` (`:start-end` with end
`(start + limit) - 1`, omitted when zero), shows `[invalid arg]` for a non-string path and
`...` for an empty, absent or null one.

[`format_read_line_range`](../crates/maestro-tools/src/output/render_utils.rs) owns
the plain suffix shared with tree summaries, also re-exported from `maestro_app`.
Both arguments absent yields empty text; a missing or nonnumeric offset defaults
to 1, and a nonnumeric limit contributes zero. Read rendering applies warning color
only when the suffix is nonempty.

A read of a `SKILL.md`, of a file inside the README's directory whose relative path is
`README.md` or below `docs/` or `examples/`, or of an `AGENTS.md`/`CLAUDE.md` file (either
extension case) is classified; the path is resolved against the context's working directory
with [`resolve_read_path`](../crates/maestro-tools/src/path_utils.rs), in that priority. A
collapsed row shows a classified call as `[skill] label`, `read docs label` or
`read resource label` with its range and a hint naming the keys bound to `app.tools.expand`,
and hides a successful result entirely; an expanded row shows the ordinary title and
content. An error result shows its content even when classified; the call title does not
depend on the result. A file counts as documentation when its path, relative to the
README's directory, is resolved against the process working directory only for a part the
path leaves open, such as the drive of a Windows path that starts with a separator. The
application supplies the README path and registers the expand action.

An ordinary result shows the cleaned output (see the [display helpers](#display-helpers)),
highlighted in the language that [`get_language_from_path`](theme.md) names for the
selected path when it names one (its matching, the engine's unsupported languages and
failures are described there), otherwise in the tool output color. A collapsed row shows up
to ten logical lines, counted before wrapping and after removing trailing empty entries,
followed by `... (N more lines, <keys> to expand)` when more remain. A truncated read adds
a warning line: the first-line, line or byte limit notice, using the limits recorded in its
details. A theme, home, path or escape-expression failure is returned
and leaves the previous component untouched.

```rust
use maestro_tools::{ReadToolOptions, create_read_tool};

let tool = create_read_tool("/work".into(), ReadToolOptions::default());
assert_eq!(tool.definition.name, "read");
```
