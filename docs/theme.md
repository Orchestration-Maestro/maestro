# Theme

`maestro-theme` builds typed terminal themes. A `Theme` holds one prepared ANSI
prefix per plane and color name, a stored `ColorMode`, and optional name and source path.
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

A file requires `name` and `colors`; `$schema`, `vars` and `export` are optional.
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
- `Failed to parse theme <path>: <cause>`: the native JSON decoder rejected the text;
  what it accepts is that decoder's, see `serde_json`.
- `Invalid theme "<path>":` followed by either or both of
  `Missing required color tokens:` (sorted) and `Other errors:`
  (`  - <json path>: <detail>`), according to the errors present.

A file that cannot be read keeps its I/O error and is never relabelled as a parse
failure.

## Replaceable operations

`ThemeOperations` supplies the file read, environment lookup, existence check,
directory listing and name sorting. `NativeThemeOperations` (not built for browsers)
reads bytes, decodes them as UTF-8 with replacement characters, reads process
variables and lists directory entries by name without filtering by file kind.
It sorts with a stable locale collator built once per call from the first present of
`LC_ALL`, `LC_MESSAGES` and `LANG` (default `en_US`); encoding and modifier suffixes are
dropped, `C` and `POSIX` mean `en-US`, and an unusable locale selects the library default.
Tests and browser callers supply their own.

## Registration and discovery

`ThemeState` holds the application-supplied `ThemeDirectories` (the shipped themes
directory and the custom themes directory), the operations and the registrations.
Constructing it reads nothing.

- **Shipped data.** The first query that needs it reads and parses `dark.json`,
  then `light.json`, from the themes directory without the custom-file admission.
  The documents are cached only when both succeed, so a failure is retried by the
  next query. The cache holds documents, not instances: every lookup constructs a
  new `Theme` and samples the color mode anew.
- **Registration.** `set_registered_themes` replaces all registrations. Themes
  without a name, or with an empty name, are ignored; a repeated name keeps its
  first position and takes the last instance. Clearing registrations does not
  affect `Rc<Theme>` handles that callers still hold.
- **Lookup.** `get_theme_by_name` returns the registered instance itself, without
  any effect. Otherwise `dark` and `light` are built from the shipped data, and any
  other name is read from `<custom dir>/<name>.json` (joined lexically, names are not
  trimmed or restricted). A new instance carries the document's name and no source
  path. Every loading failure is `None`, never a fallback theme.
- **Inventories.** `get_available_themes` lists each shipped, custom and registered
  name once, ordered by UTF-16 code units. `get_available_themes_with_paths` lists
  each name once with the path of its first owner (shipped, then custom file, then
  registration), ordered by the operations' locale sort. Custom names are the entries
  ending in a case-sensitive `.json`; contents are not read. A missing custom
  directory adds nothing; shipped-data and directory-listing failures are errors.

## Source metadata

`Theme::source_info` and `Theme::set_source_info` expose a replaceable slot holding a
shared `Rc<RefCell<SourceInfo>>` from `maestro-request`. Edits through any holder of the
record are visible through the theme; replacing or clearing the slot leaves the earlier
record unchanged for its other holders.
