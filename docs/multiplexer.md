# Workspace multiplexer

`maestro-multiplexer` presents the [workspace daemon](workspace-daemon.md) in a
host terminal. It reaches the daemon and the terminal emulator only through
`maestro_app::daemon` and `maestro_app::terminal`, and it is the only crate that
uses the frame library. The [daemon specification](specs/workspace-daemon.md)
lists what each delivery adds.

## Rendering

A frame goes through three steps:

1. `render::pane::render` paints a pane's viewport into an area of the frame
   buffer.
2. `render::frame::frame_from_buffer` turns the buffer into a protocol
   `FrameData` record.
3. `render::ansi::BlitEncoder` turns that record into terminal bytes.

### Pane cells

`render` and `collect_dirty_patch` take the pane terminal. They read one
refreshed snapshot through `PaneTerminal::with_render_view`, which holds the pane
lock while the cells are converted.

- A grapheme keeps its cell width. A wide cell is followed by an empty tail cell.
  A halfwidth katakana with its voiced mark keeps both characters in one
  two-column cell. A symbol whose measured width disagrees with its cell becomes
  blanks of the cell's width.
- Palette colours stay indexed, so they follow the host theme. Entries a
  program redefined, for example with OSC 4, are sent as RGB. Direct RGB cells
  stay unchanged.
- The default background and foreground stay transparent while the host
  terminal already shows them. When a default differs from the host's reported
  colour, or from the pane's initial default before the host reports one, the
  renderer writes it explicitly.
- Inverse text swaps its colours after resolving a transparent colour to the
  effective default, so the text stays readable. Hidden text takes its
  background colour.
- Underline colour, underline style, bold, italic, faint, blink and
  strikethrough are kept. The underline style travels in the top four modifier
  bits of the frame record.
- While the host draws images, image placeholder cells are blank.
- The renderer resets cells the viewport does not cover to the pane's default
  colours. With `show_cursor`, it places the host cursor at the pane's visible
  cursor when that falls inside the area.

`collect_dirty_patch` returns only the viewport rows that changed since they
were last painted or collected, padded to the area width, and acknowledges
them. It returns `Clean` when nothing changed. It returns `Fallback`, which
asks for a full frame, after a global change such as a resize or a colour
change, or when a changed row contains a hyperlink.

### Encoding

`BlitEncoder::encode` compares a frame with the last committed one and writes
only the cells that differ. The first frame clears the screen and writes every
cell. A forced repaint or a size change writes every cell without clearing.

- Each frame starts synchronized output, hides the cursor and closes any open
  hyperlink before writing cells.
- Adjacent single-width ASCII cells share one cursor move.
- A cell revealed when a wider previous cell shrinks is redrawn. The tail of a
  new wide cell is skipped.
- Hyperlinks use OSC 8 with control characters removed. An empty target means
  no link.
- After the cells, the encoder moves the hidden cursor, sets a changed shape
  and then shows the cursor.
- Outside Windows, the final cursor position and visibility repeat after
  synchronized output ends, so input methods place candidate windows at the
  input position.

`frame_with_drawn_cursor` reverses the cell under a visible cursor, and
`encode_with_suppressed_visible_cursor` keeps the host cursor hidden there.
`commit` records the frame and cursor memory once the bytes are written.

When the daemon's render profiler is enabled (`MAESTRO_RENDER_PROF`), encoding
and dirty-row collection report their counts and durations to it.

## Palettes

`palette::Palette` holds the 19 colours of the multiplexer chrome. There are 18
built-in themes. `Palette::from_name` accepts the daemon's theme names and
aliases.

`resolve_effective_theme` chooses the active theme from the daemon's theme
configuration:

1. With automatic switching off, the manual theme. With it on, the dark theme,
   or the light theme when the host reports a light appearance.
2. An unknown name is logged, then the fallback is used: `catppuccin`, or
   `catppuccin-latte` for the light selection. An unknown fallback uses
   `catppuccin`.
3. Authored colour overrides replace their tokens.
4. The legacy interface accent replaces the accent last.

`parse_color` accepts `reset`, `default`, `none` and `transparent`, `#rrggbb`,
`#rgb`, `rgb(r, g, b)` and the 16 terminal colour names with the `purple`,
`grey` and `darkgrey` aliases, ignoring case and surrounding spaces. It logs
anything else and returns cyan.
