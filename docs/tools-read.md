# Tool output and image helpers

`maestro-tools` delivers output selection, native streamed-output accumulation,
callable image preparation, generic tool definitions and the native `read` tool.

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

The selection, clipping and size helpers require no filesystem or runtime and
build on native and `wasm32-unknown-unknown` targets. Their typed inputs have no separate validation
or I/O failure path. See [output libraries](libraries/tools-output.md) for the
serialization and number-formatting dependencies.

## Native streaming

[`output::output_accumulator`](../crates/maestro-tools/src/output/output_accumulator.rs)
provides `OutputAccumulator`, `OutputAccumulatorOptions` and `OutputSnapshot` on
native targets. Construction does not create a file. Append accepts raw bytes;
finish flushes pending UTF-8 once without closing the artifact. Snapshots observe
selected decoded text without finishing decoding; selection is owned by
[`output::truncate`](../crates/maestro-tools/src/output/truncate.rs).

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
renderer records (`ToolRenderContext`, `ToolRenderResultOptions`) carry the row's
shared arguments and state, stable call id, invalidation, previous component,
working directory and the execution, argument, partial, expanded, image and error
flags; building a context or calling a renderer is the caller's job.

[`wrap_tool_definition`](../crates/maestro-tools/src/tool_definition_wrapper.rs)
turns a definition into an `AgentTool`. Each execution calls the optional context
factory once; a factory error is the execution's error and the body does not run.
Call id, arguments, signal and progress observer pass through, and result details
keep the three states of `AgentToolResult` (absent, explicit null, value).
`wrap_tool_definitions` wraps in order and keeps duplicates.
`create_tool_definition_from_agent_tool` is the reverse and leaves prompt text and
renderers absent. Native and browser builds differ only in whether the stored
callbacks must be `Send + Sync`.

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
`@` is dropped, U+00A0, U+2000-U+200A, U+202F, U+205F and U+3000 become a space,
and a bare `~` or `~/` becomes the home directory. Relative paths resolve
lexically through `maestro-path`, with no canonicalization. The process working
directory is read only for a relative `cwd`, and the home directory only for `~`.
`resolve_read_path` then keeps the first existing spelling among the resolved
path, the narrow-space AM/PM form, NFD, curly apostrophes, and NFD with curly
apostrophes.

The signal is checked before each of the three operations and watched while one is
pending. An abort seen there settles the read with `Operation aborted`, and a
later operation does not start. A started operating-system call is not
interrupted.

Text is decoded lossily as UTF-8, split on LF with trailing empty entries kept,
and selected with `offset` (one-based) and `limit`, both finite numbers handled
like JavaScript slice bounds: fractions truncate toward zero and a negative end
counts from the end. The caller limit applies first, then
[`truncate_head`](../crates/maestro-tools/src/output/truncate.rs) with the default
2000 lines and 50 KB. The result is plain text with a continuation notice when
lines remain, `details` holds the truncation record only when the default limits
cut the text, and an offset past the last line fails with the line count. When the
first selected line alone exceeds the byte limit the text is a `sed` command for
that line, quoting the authored path unchanged.

A detected image becomes `Read image file [<mime>]`, then the delegated dimension
note, then a note when the active model (read from `ReadContext::model` after
detection) lacks image input, and an image block. Fitting uses the image helpers
above; `auto_resize_images: false` sends the original bytes, and an image that
cannot be fitted yields only the text with the omission line. Native detection
uses the [MIME sniff](#inline-image-preparation) and ignores extensions.

```rust
use maestro_tools::{ReadToolOptions, create_read_tool};

let tool = create_read_tool("/work".into(), ReadToolOptions::default());
assert_eq!(tool.definition.name, "read");
```
