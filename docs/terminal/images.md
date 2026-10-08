# Inline images

Renders images inline for terminals that support the Kitty graphics protocol
(Kitty, Ghostty, `WezTerm`) or iTerm2 inline images. Falls back to a text placeholder
on unsupported terminals.

Dimensions are read automatically from PNG, JPEG, GIF and WebP headers.
Kitty payloads are sent unchanged with `f=100`, so they must already be PNG.
Converting other formats is the caller's responsibility; the later
`chat-tool-execution` delivery owns conversion for tool output.

```rust
use maestro_tui::{Component, Image, ImageOptions, ImageTheme, TerminalImage};

let terminal = TerminalImage::new(|_| None, || 1);
let mut image = Image::new(
    "R0lGODdhLAHIAA==".to_owned(),
    "image/gif".to_owned(),
    ImageTheme { fallback_color: Box::new(|text| format!("styled:{text}")) },
    ImageOptions::default(),
    terminal,
);
assert_eq!(image.render(40), ["styled:[Image: [image/gif] 300x200]"]);
```

## Terminal state

Clones of `TerminalImage` share cached capabilities and cell measurements.
`Default` creates independent state, reads native environment hints and samples
uniform Kitty IDs in `1..=0xffff_fffe`. Browser defaults have no environment hints.
`new` replaces both input sources without changing rendering callers; supplied ID
sources must honor that range. Callbacks run without a terminal-state borrow.

Detection is case-insensitive without trimming. A nonempty `TMUX` or a `TERM`
starting with `tmux` or `screen` disables images and hyperlinks. Otherwise Kitty,
Ghostty and `WezTerm` hints precede iTerm2 hints. `VSCode` and Alacritty enable true
color and hyperlinks without image support. Unknown terminals enable true color
only for exact `COLORTERM` values `truecolor` or `24bit`.

`get_capabilities` caches detection; `detect_capabilities` does not. Override with
`set_capabilities`, or clear only that cache with `reset_capabilities_cache`.
Cells start at 9×18 pixels. `set_cell_dimensions` updates the pair only when both
members are positive.

## Placement and fallback

`render_image` defaults to 80 cells, preserves aspect ratio for iTerm2, and allows
Kitty's terminal-side cursor motion. Direct calls do not allocate IDs. `Image`
defaults to at most 60 cells, constrained by viewport width minus two, and disables
Kitty's terminal-side cursor motion. Empty reserved rows precede the final line,
which moves the cursor up to draw; Kitty also restores it down. A single-row image
needs no cursor motion.

Explicit dimensions override headers. Unknown or invalid header dimensions become
800×600. Zero geometry, zero maximum width and viewports of width 0–2 take the
styled text fallback before allocating an ID. The fallback is not truncated to the
viewport. Geometry uses an exact integer-ratio ceiling, at least one row for
positive inputs; an unrepresentable row count returns absence.

Same-width output is cached even when terminal state changes. `invalidate` drops
only cached output, retaining dimensions and the supplied or first allocated Kitty
ID. Resize also re-renders. Invalidate explicitly after changing terminal support
or cell measurements.

## Header and wire helpers

Header inspection is not full image validation or decoding. PNG checks its first
four signature bytes and a 24-byte minimum; JPEG recognizes SOF0/1/2 in bounded
segments; GIF checks exact version signatures; WebP recognizes VP8, VP8L and VP8X
with their respective header minima. MIME dispatch accepts only exact
`image/png`, `image/jpeg`, `image/gif` and `image/webp` literals. Zero parsed
sizes, malformed segments and truncated headers return absence.

Readers accept standard or URL-safe base64, padded or unpadded, Unicode whitespace
and unused trailing bits. Other garbage, excess padding, incomplete quartets and
embedded byte-order marks return absence. Protocol encoders leave the supplied
already-encoded ASCII payload unchanged. Kitty chunks contain at most 4096 bytes;
iTerm2 encodes only a supplied nonempty UTF-8 filename. Quiet Kitty delete helpers
free image data. `hyperlink` wraps verbatim text and URL in OSC 8 sequences without
capability checks or sanitization.
