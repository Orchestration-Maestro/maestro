# Theme

`maestro-theme` builds typed terminal themes. A `Theme` holds one prepared ANSI
prefix per color name, a stored `ColorMode`, and optional name and source path.
Every public item lives in the `theme` module and is re-exported at the crate root.

```rust
use maestro_theme::{ColorMode, NativeThemeOperations, ThemeColor, load_theme_from_path};

let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme/dark.json");
let theme = load_theme_from_path(path, Some(ColorMode::Truecolor), &NativeThemeOperations)?;
assert_eq!(theme.fg(&ThemeColor::Accent, "hi")?, "\x1b[38;2;138;190;183mhi\x1b[39m");
# Ok::<(), maestro_theme::ThemeError>(())
```

## Theme files

`assets/theme/dark.json` and `light.json` are the shipped themes; `theme-schema.json`
is the published editor schema and rejects unknown fields. Loading admits a looser
runtime schema (`runtime-schema.json`) that ignores unknown root and `export` fields
and resolves extra entries under `colors`.

A file has a required `name`, optional `$schema`, `vars`, `colors` and `export`.
`colors` holds all 51 required tokens (45 foreground, 6 background: `selectedBg`,
`userMessageBg`, `customMessageBg`, `toolPendingBg`, `toolSuccessBg`,
`toolErrorBg`). A color, and every variable, is one of:

| Form | Meaning |
|------|---------|
| `""` | The terminal default (`39` for foreground, `49` for background) |
| `"#rrggbb"` | Exactly six hexadecimal digits |
| `0`–`255` | An explicit palette index; integral spellings such as `1.0` count |
| another string | A variable name, followed through `vars` until it reaches one of the above |

Each color follows its own alias chain; a name met twice in one chain is a cycle.
All aliases resolve before any ANSI prefix is built, so a missing variable is
reported ahead of a malformed hex color. When a member appears twice in the file,
the last one wins.

## Color modes

`load_theme_from_path` takes an optional `ColorMode`. When it is `None` the
operations' environment decides, read in this order and compared case-sensitively:
`COLORTERM` of `truecolor` or `24bit`, then a nonempty `WT_SESSION`, select
truecolor; an empty, `dumb` or `linux` `TERM`, `TERM_PROGRAM=Apple_Terminal`,
and a `TERM` of `screen`, `screen-…` or `screen.…` select 256 colors; anything else
is truecolor. An explicit mode, or any earlier failure, reads no environment.

In 256-color mode a hex color becomes the nearest entry of the six-level cube,
or a gray-ramp entry when its channel spread is below 10 and the gray is closer
(weighted distance 0.299, 0.587, 0.114; lower index wins ties). Explicit
indices are never requantized.

## Errors

`ThemeError` messages are:

- `Invalid hex color: <value>`: the text after `#` is not six hexadecimal digits.
- `Invalid color value: <value>`: not empty, hex or an index.
- `Variable reference not found: <name>` and `Circular variable reference detected: <name>`.
- `Unknown theme color: <name>` / `Unknown theme background color: <name>`: lookup of
  a key the theme was not built with.
- `Failed to parse theme <path>: <cause>`: the file is not JSON.
- `Invalid theme "<path>":` followed by `Missing required color tokens:` (sorted)
  and `Other errors:` (`  - <json path>: <detail>`).

A file that cannot be read keeps its I/O error and is never relabelled as a parse
failure.

## Replaceable operations

`ThemeOperations` supplies the file read and environment lookup. `NativeThemeOperations`
(not built for browsers) reads bytes, decodes them as UTF-8 with replacement
characters, and reads process variables. Tests and browser callers supply their own.
