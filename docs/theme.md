# Theme

`maestro-theme` builds typed terminal themes. A `Theme` holds one prepared ANSI
prefix per plane and color name, a stored `ColorMode`, and optional name and source path.
Every public item lives in the `theme` module and is re-exported at the crate root.

```rust
use maestro_theme::{ColorMode, NativeThemeOperations, ThemeColor, load_theme_from_path};

let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme/dark.json");
let theme = load_theme_from_path(path, Some(ColorMode::Truecolor), &NativeThemeOperations)?;
assert_eq!(theme.fg(&ThemeColor::Accent, "hi")?, "\x1b[38;2;217;160;102mhi\x1b[39m");
# Ok::<(), maestro_theme::ThemeError>(())
```

## Theme files

`assets/theme/dark.json` and `light.json` are the shipped themes, derived from the
[brand pack](#brand-pack); `theme-schema.json`
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
variables and lists directory entries by name, in byte order of the names, without
filtering by entry kind.
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
  A parse failure keeps the decoder's own message, without the custom-file label.
  The documents are cached only when both succeed, so a failure is retried by the
  next query. The cache holds documents, not instances: every unregistered successful
  lookup constructs a new `Theme` and samples the color mode anew.
- **Registration.** `set_registered_themes` replaces all registrations. Themes
  without a name, or with an empty name, are ignored; a repeated name keeps its
  first position and takes the last instance. Clearing registrations does not
  affect `Rc<Theme>` handles that callers still hold.
- **Lookup.** `get_theme_by_name` returns the registered instance itself, without
  any effect. Otherwise `dark` and `light` are built from the shipped data, and any
  other name is read from `<custom dir>/<name>.json` (joined with
  `maestro_path::join`; names are not trimmed or restricted). A new instance carries the document's name and no source
  path. Every loading failure is `None`, never a fallback theme; shipped data without
  a `colors` object fails construction the same way.
- **Inventories.** `get_available_themes` lists each shipped, custom and registered
  name once, ordered by UTF-16 code units. `get_available_themes_with_paths` lists
  each name once with the path of its first owner (shipped, then custom entry, then
  registration), ordered by the operations' locale sort. Custom names are the entries
  ending in a case-sensitive `.json`, directories included; contents are not read. A missing custom
  directory adds nothing; shipped-data and directory-listing failures are errors.

## Live theme and change callback

`ThemeState::theme` returns a `LiveTheme` handle whose `get` returns the
currently published instance; clones read the same slot, and an `Rc<Theme>` already
returned stays what it was. Before the first publication `get` fails with
`Theme not initialized. Call initTheme() first.`

- `init_theme` selects the given name, else `dark` or `light` from the second
  `;`-separated field of `COLORFGBG` (below 8 or no leading decimal digits selects
  `dark`; an empty name is a name, not an omission). A failure to load the name
  selects and publishes `dark` silently. The callback is never invoked.
- `set_theme` selects, publishes and then invokes the callback. A load failure or a
  callback failure publishes `dark` and is reported as `ThemeChangeResult::Failure`
  carrying the original message; only a failure to load `dark` is an error.
- `set_theme_instance` publishes the supplied instance, selects `<in-memory>`,
  stops watching and invokes the callback; a callback failure is returned and none
  of the callback's changes are rolled back.
- `on_theme_change` replaces the one callback. It runs outside every state borrow,
  so it may read the state, replace itself or publish again.

## Watching and reload

The watcher operand of `init_theme` and `set_theme` is `Some(operations)` to start
watching and `None` to leave any existing watch untouched. Starting stops the
previous watch and pending reload first, then watches the custom directory
(non-recursively) when the selected name is not empty, `dark` or `light` and
`<name>.json` exists there. A notification names the file, has no name or an empty
one schedules a reload 100 ms later, restarting any pending one; other names and
stale selections are ignored. The reload rereads the file, registers the new
instance under the selected name, publishes it and invokes the callback. A missing
or invalid file, or a callback failure, keeps the last good theme and is not
reported. A watch failure closes only the watch; a pending reload still runs and
nothing rewatches. `stop_theme_watcher` cancels both and keeps the published theme.

`ThemeWatchOperations` supplies the directory watch and the timer;
`NativeThemeWatchOperations` (not built for browsers) uses operating-system
notifications and Tokio timers on a `LocalSet` that the caller creates and drives.
Dropping the state or closing or dropping a native watch or timer handle cancels
its pending work; a closed or dropped watch dispatches no queued notification. The native
watch resolves the directory against the working directory once, as the notifier
does.

## Resolved colors for export

Loading a custom theme resolves its colors in the same order, so the first
failing alias is the same. `get_resolved_theme_colors` returns every color of a theme in
canonical key order: array-index keys ascending, then the rest in authored order. The
document is the shipped one, else the file of a registration (a registration
without a source path is an error), else the custom file. Authored color text is returned unchanged; palette indices expand to `#rrggbb` from
the 16 basic colors, the 6x6x6 cube and the 24-step gray ramp; an empty color becomes
`#000000` when the name is exactly `light` and `#e5e5e7` otherwise. The name is the
argument, else the selected theme, else the terminal background's theme.
`get_theme_export_colors` resolves the optional `export` fields of the same document
and returns none of them when anything fails; an empty field is absent.
`is_light_theme` tests only the supplied name.

## Brand pack

The shipped `dark.json` and `light.json` are generated from the shared brand pack
(`assets/brand/brand.json`, shape in the [identity guide](identity.md#pack-shape-and-swapping)),
never edited by hand. A same-format pack supplies the outputs below without a code change.

```sh
cargo run -p maestro-theme --example brand_themes -- assets/brand/brand.json crates/maestro-theme/assets/theme
```

The example writes `dark.json` and `light.json` into the given directory. Fewer or more
arguments fail with `usage: brand_themes <pack.json> <output-directory>`.

`load_brand_pack(path, operations)` reads exactly `path` through `ThemeOperations` and
decodes it. It searches for no default pack and reads no template or font. A read
failure keeps its I/O error; text or fields the decoder rejects give
`Invalid brand pack <path>: <cause>`. Every listed field is required and not null;
records are objects, never positional arrays; unknown members are ignored and a repeated
member keeps its last value. A type size of zero or less, or a negative spacing or
radius, gives `Invalid brand measurement: <JSON Pointer>`. Mark paths become the pack's
directory joined with the authored relative path (`maestro_path::join`, lexical).

A `BrandMode` (`Dark` or `Light`, independent of `ColorMode`) selects the mode of every
projection. A projection resolves a role through the mode's alias, then the palette.

- `BrandPack::theme_json` returns the standard theme document: `$schema`, `name` (the
  mode), `vars` (the mode's resolved colors in key order), `colors` (the 51 established
  keys mapped through `brand-roles.json`) and `export`. The four terminal-default keys
  stay empty. Custom themes and the generic export fallback are untouched.
- `BrandPack::presentation` returns the resolved colors, fonts, type, spacing, radii,
  glyphs and ANSI and truecolor aliases, borrowed from the pack.
- `BrandPresentation::css_properties` returns custom properties only:
  `--maestro-color-<role>`, `--maestro-font-<role>`, `--maestro-type-<role>-font`, `-size`,
  `-line-height` and `-weight`, `--maestro-spacing-<role>` and `--maestro-radii-<role>`.
  Size, spacing and radii carry `px`. Named families are quoted, generic family keywords
  stay bare, and fallback order and repeats are kept. Names and strings use CSS escapes,
  with `<` as a hexadecimal escape.
- `BrandPack::mark_svg` reads the default template or the named variant and substitutes
  `{{wordmark}}` (XML-escaped), `var(--role)` and `currentColor` (the `text` color) in one
  pass; inserted text is never scanned again.

Empty colors in exported themes follow [Resolved colors for export](#resolved-colors-for-export);
callers that want the pack's text color read `BrandPresentation::colors["text"]`.

Projection failures are `Missing brand mode: <mode>`, `Missing brand palette color: <key>`,
`Invalid brand color: <value>` (not `#` and six hexadecimal digits), `Missing brand font: <key>`,
`Missing brand color role: <role>`, `Unknown brand mark variant: <name>` (before any read) and
`Unterminated brand template color`. A projection checks the selected mode's colors and the
pack's font, glyph and terminal references, and `mark_svg` reads only the requested
template; nothing falls back to a default.

Font files, pack selection, frontend rendering and the exported-page integration are
separate deliveries.

## Source metadata

`Theme::source_info` and `Theme::set_source_info` expose a replaceable slot holding a
shared `Rc<RefCell<SourceInfo>>` from `maestro-request`. Edits through any holder of the
record are visible through the theme; replacing or clearing the slot leaves the earlier
record unchanged for its other holders.
