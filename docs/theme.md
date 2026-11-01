# Theme

For selecting a theme and writing a complete custom file, read
[Select and create terminal themes](themes.md).

`maestro-theme` builds typed terminal themes. A `Theme` holds one prepared ANSI
prefix per plane and color name, a stored `ColorMode`, and optional name and source path.
Every public item lives in the `theme` module and is re-exported at the crate root.

```rust
use maestro_theme::{ColorMode, ThemeColor, load_theme_from_path};

let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme/dark.json");
let theme = load_theme_from_path(path, Some(ColorMode::Truecolor))?;
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
process environment decides, read in this order and compared case-sensitively:
`COLORTERM` of `truecolor` or `24bit`, then a nonempty `WT_SESSION`, select
truecolor; an empty, `dumb` or `linux` `TERM`, `TERM_PROGRAM=Apple_Terminal`,
and a `TERM` of `screen`, `screen-…` or `screen.…` select 256 colors; anything else
is truecolor. An explicit mode, or any earlier failure, reads no environment.

`supports_color` decides whether plain terminal messages, such as warnings, are
styled at all. The checks run in this order:

1. A `FORCE_COLOR` of `0` or `false` turns color off. `true`, an empty value or a
   positive level asks for color.
2. An Azure Pipelines agent (`TF_BUILD` and `AGENT_NAME` both set) gets color.
3. Without `FORCE_COLOR`, standard output must be a terminal.
4. A `TERM` of `dumb` gets color only when `FORCE_COLOR` asked for it.
5. Windows gets color.
6. With `CI` set, color comes only from GitHub Actions, Gitea Actions, CircleCI,
   Travis CI, AppVeyor, GitLab CI, Buildkite, Drone, `CI_NAME=codeship` or
   `FORCE_COLOR`.
7. With `TEAMCITY_VERSION` set, version 9.1 or later gets color.
8. Any `COLORTERM`, a `TERM_PROGRAM` of `iTerm.app` or `Apple_Terminal`, or a
   `TERM` that is `wezterm`, ends in `-256` or `-256color`, starts with `screen`,
   `xterm`, `vt100`, `vt220` or `rxvt`, or contains `color`, `ansi`, `cygwin` or
   `linux` (in any case) gets color.
9. Anything else gets color only when `FORCE_COLOR` asked for it.

In 256-color mode a hex color becomes the nearest entry of the six-level cube,
or a gray-ramp entry when its channel spread is below 10 and the gray is closer
(weighted distance 0.299, 0.587, 0.114; lower index wins ties). Explicit
indices are never requantized.

The shipped Forge light theme selects the brand pack's Oxide for accent, live,
success and error text. Nearest-color conversion is unchanged. The ANSI contrast
regression checks those text keys at 4.5:1 and border keys at 3:1 on all six
panel outputs, in both shipped themes and both color modes, using standard
256-color palette RGB values. Terminal-default text and user-customized palettes
are outside that measurement; see the [identity guide](identity.md#terminal-roles).

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

## Files, environment and sorting

Theme loading reads files and process variables directly. It decodes file bytes as
UTF-8 with replacement characters and lists a directory through the shared directory
listing (`maestro-listing`): by name, in byte order of the names, without filtering
by entry kind. Browser builds have no file system, so every read fails there and no
directory is listed.
Inventories sort with a stable locale collator (`maestro_theme::Collator`, a pure re-export of the text crate's comparator, also at `maestro_theme::theme::Collator` outside browser targets) built once per call; locale selection and fallback are
decided by the [text collator](text.md).

`Collator` (not built for browsers) is that comparison on its own: `Collator::new` takes an
environment lookup and applies the same locale selection, and `compare` orders two strings.
Other native code that needs the process locale's ordering, such as the
[directory listing](tools-search.md#listing), builds one with it.

## Registration and discovery

`ThemeState` holds the application-supplied `ThemeDirectories` (the shipped themes
directory and the custom themes directory) and the registrations.
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
  registration), ordered by the process locale's [collator](#files-environment-and-sorting). Custom names are the entries
  ending in a case-sensitive `.json`, directories included; contents are not read. A missing custom
  directory adds nothing; shipped-data and directory-listing failures are errors.

## Live theme and change callback

`LiveTheme::default()` creates an unpublished slot.
`ThemeState::theme` returns a `LiveTheme` handle whose `get` returns the
currently published instance; clones read the same slot, and an `Rc<Theme>` already
returned stays what it was. Before the first publication `get` fails with
`Theme not initialized. Call initTheme() first.`
`LiveTheme::from_theme` wraps a prepared instance in its own fixed slot, so a caller
can pass that exact instance wherever a `LiveTheme` is expected; it never follows
later publication.

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

## Decorations and border callbacks

`Theme::bold`, `italic`, `underline`, `inverse` and `strikethrough` take the text and an
explicit `enabled` flag that stands for the caller's terminal style capability; the library
detects nothing. Empty text and disabled calls return the text unchanged. Otherwise the text is
wrapped in the style's opening escape and its own closing escape (`22`, `23`, `24`, `27`, `29`).
Each matching closing escape inside the text is followed by the opening escape again, and each
LF or CRLF is preceded by the closing and followed by the opening escape. Other escapes,
including reset `0`, and a lone CR stay literal. `fg` and `bg` keep their literal
prefix, unchanged text and plane reset. `red`, `yellow`, `green` and `gray` take the same flag
and close with the foreground reset `39`, and `dim` closes with `22`.

`get_thinking_border_color` maps `off`, `minimal`, `low`, `medium`, `high` and `xhigh` to
their `thinking*` keys and any other level to `thinkingOff`; `get_bash_mode_border_color`
uses `bashMode`. The returned callbacks keep the instance they were created from, whatever
is published later, and return their text unstyled when that instance lacks the key.

## Code highlighting

`SyntaxHighlighter` classifies code in a named language into `SyntaxSpan` runs: byte ranges
of the original text, in order, each with one of the nine syntax colors or none. Spans carry
no styling, so one interface serves the terminal and HTML callers, and a controlled
implementation can replace the native one. `SyntectHighlighter` is the native adapter. It
parses the whole snippet with one state, so a block comment or string spans its lines, and
joins adjacent runs of one color.

Labels are matched without case against the bundled syntax names and file extensions, then
`assets/syntax-aliases.json` names the exceptions: labels whose extension would select another
grammar (`fs` is F#, `h` is C) and labels with no grammar in the set, which stay unsupported
instead of borrowing a similar one. A scope maps to a color by its longest matching rule
prefix, and the innermost scope with a color wins:

| Color | Scope prefixes |
| --- | --- |
| `syntaxComment` | `comment` |
| `syntaxKeyword` | `keyword` |
| `syntaxOperator` | `keyword.operator` |
| `syntaxFunction` | `entity.name.function`, `support.function` |
| `syntaxVariable` | `variable`, `entity.other.attribute-name` |
| `syntaxString` | `string` |
| `syntaxNumber` | `constant.numeric`, `constant.language` |
| `syntaxType` | `storage.type`, `entity.name.type`, `entity.name.class`, `support.class` |
| `syntaxPunctuation` | `punctuation` |

`highlight_code` splits the code at every LF, keeping CR. It admits only labels its language
table names, aliases included, and leaves every other label to the code color: a missing,
empty or unnamed label never reaches the engine and colors every row with `mdCodeBlock`, so
that call fails when no theme is published. A named label whose grammar the engine lacks
colors the rows with `mdCodeBlock` through the published theme; with no theme published it
returns them unstyled and does not fail. Text the engine leaves unclassified,
such as explicit `plaintext`, is returned unchanged. An engine failure returns
the rows unstyled. Classified runs are styled from the theme published when the call runs,
once per nonempty row piece.
`get_markdown_theme`'s code callback colors the rows with `mdCodeBlock` after a failure
instead.

`highlight_html` returns an escaped fragment of `<span class="hljs-*">` runs (`comment`,
`keyword`, `function`, `variable`, `string`, `number`, `type`, `operator`, `punctuation`)
and plain escaped text for unclassified runs; `&`, `<`, `>`, `"` and `'` (`&#x27;`) are
escaped. A missing, empty or unsupported label returns `None` and an engine failure is
returned as the error. This explicit-only operation does not select a language.

`highlight_css()` projects these same nine class names through the existing
brand-role aliases as `--maestro-color-*` references. It emits the nine syntax selectors without adding a palette; the native host supplies the selected pack's custom properties.

`highlight_auto` adds automatic selection for HTML callers. `HighlightProfile::ExportViewer`
and `HighlightProfile::CodeBlock` retain separate ordered language sets. A registered label
is lowercased and resolved to its canonical grammar without trimming or splitting; missing,
empty or unregistered labels trigger detection. The viewer admits `c` and `plaintext`
explicitly but excludes them from detection. The browser admits only its own registrations,
even when another grammar is available to the native painter.

Detection uses shared immutable grammar data with invocation-local scoring and embedded
frames. Each mode searches a combined, ordered terminator expression, preserving rule-order
ties and original captures. Ignored begins resume the remaining rules at that position
before the full matcher continues. Combined expressions are compiled once during grammar
preparation, including only reachable resumption suffixes. First detection prepares only the
chosen profile's reachable modes and standalone keyword/end patterns; both profiles reuse
already prepared expressions and modes. Compilation failures propagate before scanning.
An explicit registered label bypasses detector preparation. The viewer uses version 11.9.0
data and browser code blocks use 11.11.1; identical patterns, keyword tables and modes are shared. Derived grammar data carries the BSD-3-Clause
notice in [third-party notices](../THIRD-PARTY-NOTICES.md). The selected language is painted
by the same `SyntaxHighlighter` and HTML span emitter, preserving the complete original
input and existing token classes. A plain result or an unavailable selected native syntax
returns escaped original text. Painter failures propagate; a frontend owns recovery,
surrounding markup, the authored label and copy text. Terminal highlighting is unchanged.

`get_language_from_path` compares the lowercase text after the last `.` of the whole string
with the file-extension table. A name without a dot is compared whole, so extensionless
`Dockerfile` and `Makefile` match only without a directory; a suffixed name such as
`DIR/FILE.DOCKERFILE` matches by its suffix.

## Component styles

`paint(theme: Option<&Theme>, key, text)` applies a prepared foreground or returns
unstyled text when the supplied theme or key is absent. Live consumers supply
an observation of their live handle at invocation; captured consumers supply
their retained instance.

`get_markdown_theme`, `get_select_list_theme`, `get_editor_theme` and
`get_settings_list_theme` return the terminal toolkit's style records. Their coloring callbacks
read the published theme on every call and return their text unstyled when no theme is published
or the key is missing. Strikethrough and unselected settings labels do not read the theme. The Markdown decorations apply when `styles_enabled` is set and first
check that a theme is published; strikethrough does not. The settings cursor is the only value
fixed at creation: the accent-colored arrow and space of the theme published then.

## Watching and reload

The watcher operand of `init_theme` and `set_theme` is `Some(operations)` to start
watching and `None` to leave any existing watch untouched. Starting stops the
previous watch and pending reload first, then watches the custom directory
(non-recursively) when the selected name is not empty, `dark` or `light` and
`<name>.json` exists there. A notification names the file, has no name or an empty
one schedules a reload 100 ms later, restarting any pending one; other names and
stale selections are ignored. The reload rereads the file, registers the new
instance under the selected name, publishes it and invokes the callback. A missing
or invalid file keeps the last good theme and is not reported; a callback failure
is not reported either and nothing the callback changed is rolled back. A watch failure closes only the watch; a pending reload still runs and
nothing rewatches. `stop_theme_watcher` cancels both and keeps the published theme.

`ThemeWatchOperations` and `NativeThemeWatchOperations` are same-type aliases of
[the shared filesystem-watch effects](filesystem-watch.md). Dropping the theme
state cancels its watch and pending reload.

## Resolved colors for export

`ansi256_to_hex` is the shared numeric palette expansion used by theme export and
terminal-text HTML conversion. Indices above 255 retain authored grayscale arithmetic.

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

`load_brand_pack(path)` reads `path`, then decodes it with `BrandPack::parse(path, text)`,
which applies [shared brand admission](brand.md#admission). Callers that already hold
the text, such as browser builds with an embedded pack, call `BrandPack::parse` directly;
`path` then only places the marks and labels errors. Loading searches for no default pack
and reads no template or font. Read failures retain the I/O error; decoder failures are
wrapped as `Invalid brand pack <path>: <cause>`. Mark paths become the pack's directory
joined with the authored relative path ([lexical path joining](../crates/maestro-path/src/lib.rs)).

`BrandMode`, `BrandFont`, `BrandType`, `BrandMark`, `BrandGlyph` and
`BrandPresentation` remain re-exported through `maestro_theme` and
`maestro_theme::theme`. Their [pure projections](brand.md#projection) belong to
`maestro-brand`.

- `BrandPack::presentation` delegates to the shared pack's selected-mode projection.
- `BrandPack::theme_json` returns the standard theme document: `$schema`, mode `name`,
  `vars`, the 51 ordered `colors` keys mapped through `brand-roles.json`, and `export`.
  The four terminal-default keys stay empty. Custom themes and the generic export
  fallback are untouched.
- `BrandPack::mark_svg` selects the default template or named variant, resolves the
  presentation, reads that template, then delegates to
  [`BrandPresentation::fill_mark`](brand.md#projection). An unknown variant fails
  before projection or reading.

Empty colors in exported themes follow [Resolved colors for export](#resolved-colors-for-export);
callers that want the pack's text color read `BrandPresentation::colors["text"]`.
Font files, pack selection and frontend rendering are separate deliveries.

## Source metadata

`Theme::source_info` and `Theme::set_source_info` expose a replaceable slot holding a
shared `Rc<RefCell<SourceInfo>>` from `maestro-request`. Edits through any holder of the
record are visible through the theme; replacing or clearing the slot leaves the earlier
record unchanged for its other holders.
