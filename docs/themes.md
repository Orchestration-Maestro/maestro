# Select and create terminal themes

Open `/settings`, choose Theme, and select `dark` or `light` to try the shipped
Forge colors. Move through the theme list to preview; confirm to save the name,
or cancel to restore the committed selection. This changes terminal colors,
not your terminal font or the Maestro brand pack.

For the library API, see [Theme](theme.md). For the shared palette
and contrast rules, see [identity](identity.md#terminal-roles) and
[brand projection](brand.md).

## Find a theme and select it

Maestro gathers custom themes from these sources:

| Source | Location or input |
| --- | --- |
| User directory | `<agent>/themes/`; normally `~/.maestro/agent/themes/` |
| Project directory | `.maestro/themes/` in the working directory |
| Installed packages | The package's `themes/` directory or `maestro.themes` entries in `package.json` |
| Settings | The `themes` array of file or directory paths |
| Command line | Repeatable `--theme <path>` file or directory arguments |

`MAESTRO_CODING_AGENT_DIR` changes `<agent>`.
[Resource loading](application/resources.md#resource-loader) describes path
resolution, collisions and provenance; [packages](packages.md) describes package
resource selection. Use a distinct, nonempty document `name`, preferably the
filename without `.json`; avoid the built-in names `dark` and `light`.

`--theme` loads a resource. It does **not** select a theme by name. Select the
loaded name in `/settings`, or merge this property into
`<agent>/settings.json` or `.maestro/settings.json`:

```json
{"theme": "forge-study"}
```

For a theme outside the discovered directories, merge both properties instead:

```json
{"themes": ["./terminal-colors/forge-study.json"], "theme": "forge-study"}
```

Do not replace an existing settings file with these fragments. Project settings
override user settings. The interactive selector saves its choice in user
settings, so remove a conflicting project `theme` if that choice does not survive
restart. See [settings](settings.md).

To disable automatic theme resource loading while keeping an explicit path:

```sh
maestro --no-themes --theme ./terminal-colors/forge-study.json
```

This disables automatic theme resource loading, not the shipped themes or the
user theme directory lookup. An explicit path still loads. Choose `forge-study`
in `/settings` after startup. Without a saved name, Maestro uses the second
semicolon-separated `COLORFGBG` field: values below 8, or no leading decimal
digits, choose `dark`; other values choose `light`. It does not query the screen's
actual background color. A theme that cannot load at startup falls back to `dark`.

## Make a complete file

Create the user theme directory (use your overridden agent directory if set):

```sh
mkdir -p ~/.maestro/agent/themes
```

Save the following as `~/.maestro/agent/themes/forge-study.json`. This complete
example uses Forge's named roles rather than a separate palette. The terminal's
default text color stays unchanged; set it to a readable foreground on the panel
background. The shipped [dark](../crates/maestro-theme/assets/theme/dark.json) and
[light](../crates/maestro-theme/assets/theme/light.json) files are generated from
the [brand pack](../assets/brand/brand.json). Copy a file to customize it; never
edit generated built-ins.

```json
{
  "$schema": "https://raw.githubusercontent.com/Orchestration-Maestro/maestro/main/crates/maestro-theme/assets/theme/theme-schema.json",
  "name": "forge-study",
  "vars": {
    "accent": "#D9A066",
    "background": "#0E0B09",
    "border": "#5E7383",
    "error": "#FF6A1A",
    "live": "#FF6A1A",
    "muted": "#D9A066",
    "panel": "#2A1F1A",
    "success": "#FF6A1A",
    "text": "#F2E8DC",
    "warning": "#D9A066"
  },
  "colors": {
    "accent": "accent",
    "border": "border",
    "borderAccent": "accent",
    "borderMuted": "border",
    "success": "success",
    "error": "error",
    "warning": "warning",
    "muted": "muted",
    "dim": "muted",
    "text": "",
    "thinkingText": "muted",
    "selectedBg": "panel",
    "userMessageBg": "panel",
    "userMessageText": "",
    "customMessageBg": "panel",
    "customMessageText": "",
    "customMessageLabel": "accent",
    "toolPendingBg": "panel",
    "toolSuccessBg": "panel",
    "toolErrorBg": "panel",
    "toolTitle": "",
    "toolOutput": "muted",
    "mdHeading": "accent",
    "mdLink": "accent",
    "mdLinkUrl": "muted",
    "mdCode": "accent",
    "mdCodeBlock": "text",
    "mdCodeBlockBorder": "border",
    "mdQuote": "muted",
    "mdQuoteBorder": "border",
    "mdHr": "border",
    "mdListBullet": "accent",
    "toolDiffAdded": "success",
    "toolDiffRemoved": "error",
    "toolDiffContext": "muted",
    "syntaxComment": "muted",
    "syntaxKeyword": "accent",
    "syntaxFunction": "accent",
    "syntaxVariable": "text",
    "syntaxString": "success",
    "syntaxNumber": "warning",
    "syntaxType": "live",
    "syntaxOperator": "text",
    "syntaxPunctuation": "text",
    "thinkingOff": "border",
    "thinkingMinimal": "muted",
    "thinkingLow": "accent",
    "thinkingMedium": "accent",
    "thinkingHigh": "live",
    "thinkingXhigh": "live",
    "bashMode": "success"
  },
  "export": {
    "pageBg": "background",
    "cardBg": "panel",
    "infoBg": "panel"
  }
}
```

The `$schema` URL points to the [editor schema](../crates/maestro-theme/assets/theme/theme-schema.json).
Use it for editor validation and completion. `name` and `colors` are required;
`vars` and `export` are optional. All 51 tokens below are required, including
ones you leave as `""`. The editor schema rejects unknown fields; the runtime
accepts extra root/export fields and additional color keys. Schema validation
alone does not catch missing variable references or alias cycles; loading does.

### Color values and terminal compatibility

| Value | Effect |
| --- | --- |
| `"#D9A066"` | Six-digit RGB, emitted directly in truecolor mode |
| `173` | An explicit index in the terminal's 256-color palette |
| `"accent"` | Resolve that key in `vars`; variables may reference other variables |
| `""` | Use the terminal's default foreground or background for this token |

Do not use three-digit hex, alpha, CSS color names or `rgb(...)`. An undefined
variable or a cycle prevents the theme from loading. Indices must be integers
from 0 through 255: 0 through 15 are the terminal's configurable basic colors,
16 through 231 form a six-level RGB cube, and 232 through 255 form a gray ramp.

Inspect the environment inherited by Maestro:

```sh
printf 'COLORTERM=%s TERM=%s TERM_PROGRAM=%s WT_SESSION=%s\n' \
  "$COLORTERM" "$TERM" "$TERM_PROGRAM" "$WT_SESSION"
```

Exact `COLORTERM=truecolor` or `24bit`, or a nonempty `WT_SESSION`, selects RGB.
Otherwise an unset/empty `TERM`, `dumb`, `linux`, `screen`, `screen-*`, `screen.*`,
or `TERM_PROGRAM=Apple_Terminal` selects 256 colors. Other environments select
RGB. These are environment-based choices, not a display capability test.
In 256-color mode Maestro approximates hex colors with cube or grayscale entries;
explicit indices stay unchanged. Test the result in both modes, particularly
text on colored panels. See [color modes](theme.md#color-modes) for exact rules.

### Token purposes

Six tokens ending in `Bg` paint backgrounds. The other 45 paint foregrounds.
A token names a purpose, not a required hue. Shared colors still need labels or
shapes that distinguish success, error and pending states.

| Token | Where to check it |
| --- | --- |
| `accent` | Selected controls, cursor and emphasized interface text |
| `border` | Ordinary component boundaries |
| `borderAccent` | Emphasized boundaries |
| `borderMuted` | Subdued boundaries |
| `success` | Successful outcome text |
| `error` | Error and failure text |
| `warning` | Warnings and attention notices |
| `muted` | Secondary descriptions and metadata |
| `dim` | Lower-emphasis interface text |
| `text` | Ordinary content foreground |
| `thinkingText` | Assistant thinking content |
| `selectedBg` | Selected row background |
| `userMessageBg` | User message panel |
| `userMessageText` | User message foreground |
| `customMessageBg` | Custom message panel |
| `customMessageText` | Custom message foreground |
| `customMessageLabel` | Custom message label |
| `toolPendingBg` | Tool panel before completion |
| `toolSuccessBg` | Tool panel after a successful result |
| `toolErrorBg` | Tool panel after an error result |
| `toolTitle` | Tool heading foreground |
| `toolOutput` | Tool output foreground |
| `mdHeading` | Markdown heading text |
| `mdLink` | Markdown link label |
| `mdLinkUrl` | Markdown link destination |
| `mdCode` | Inline code |
| `mdCodeBlock` | Unclassified or fallback code-block text |
| `mdCodeBlockBorder` | Code-block boundary text |
| `mdQuote` | Quoted text |
| `mdQuoteBorder` | Quote boundary |
| `mdHr` | Horizontal separator |
| `mdListBullet` | List markers |
| `toolDiffAdded` | Added diff lines |
| `toolDiffRemoved` | Removed diff lines |
| `toolDiffContext` | Unchanged diff lines |
| `syntaxComment` | Highlighted code comments |
| `syntaxKeyword` | Language keywords |
| `syntaxFunction` | Function names |
| `syntaxVariable` | Variables and attribute names |
| `syntaxString` | String literals |
| `syntaxNumber` | Numeric and language constants |
| `syntaxType` | Type, class and storage-type names |
| `syntaxOperator` | Operators |
| `syntaxPunctuation` | Code punctuation |
| `thinkingOff` | Editor border with thinking off |
| `thinkingMinimal` | Editor border at minimal thinking |
| `thinkingLow` | Editor border at low thinking |
| `thinkingMedium` | Editor border at medium thinking |
| `thinkingHigh` | Editor border at high thinking |
| `thinkingXhigh` | Editor border at extra-high thinking |
| `bashMode` | Editor border for a draft starting with `!` |

Token use depends on the component: a custom tool renderer may supply its own
styles. See [message rendering](chat/messages.md) and
[syntax scope mapping](theme.md#code-highlighting) for the owning behavior.

## Save, reload and recover

1. Start Maestro and select `forge-study` in `/settings`.
2. Change `vars.accent` in the file and save valid JSON. The active user theme
   reloads after a 100 ms notification debounce; the next redraw uses the new colors.
3. Temporarily remove a required color and save. The watcher keeps the last good
   theme without an error notice. Restore the color and save again to recover.
4. If selecting a theme fails, Maestro publishes `dark` and reports the failure.
   Fix the file and select the name again. The requested name is still saved.

Automatic watching targets `<agent>/themes/<selected-name>.json`, not an
arbitrary package, project or `--theme` source path. Keep filename and document
name equal in that directory for this workflow. For other sources, use `/reload`
to reload resources, then select the theme again. Built-ins are not watched.
After a directory-watch failure Maestro does not automatically reattach; select
the theme again to restart watching. See [watching](theme.md#watching-and-reload).

## Check messages and contrast before sharing

Use a disposable project and a controlled provider configured through
[local models](catalog.md), with no real credentials. Do not ask an untrusted
model to run shell or file-changing tools on your normal workspace. Offline
startup is not a model-network sandbox.

1. Check a user message, assistant text and expanded thinking, plus an
   extension's custom message if you use one. Include a long wrapped line,
   headings, links, inline code, a fenced code block, a quote and a list.
2. Have the controlled response call a tool on a disposable file. Inspect the
   pending panel, successful output, an intentional missing-file error, and a
   diff with added, removed and unchanged lines. Expand the output with the
   configured control (Ctrl+O by default).
3. Open `/settings` and inspect selected and unselected rows. Cycle supported
   thinking levels, then type `!` without submitting it to inspect the shell-mode
   border. Check both narrow and wide terminal layouts.
4. Measure actual foreground/background pairs using the
   [contrast method](identity.md#text-contrast): ordinary text needs at least
   4.5:1; necessary non-text boundaries need 3:1. Include every message and tool
   background, selected rows, links and muted text. Repeat with emitted
   256-color values, not the authored hex alone. `""` requires measuring your
   terminal's real defaults; customized ANSI slots also require local measurement.
5. Repeat after a valid edit, a rejected edit and recovery. Keep success/error
   labels readable even if their colors match. A readable theme does not depend
   on color alone to convey an outcome.

For an automated implementation check from this source checkout, run the existing
message-state and theme suites. They use controlled fixtures and disposable files,
not paid model requests:

```sh
cargo test --locked -p maestro-theme
cargo test --locked -p maestro-chat --test integration message_rendering
```

These checks prove library behavior and shipped contrast pairs; they do not
certify your terminal's defaults or your custom file's contrast.

## Check the exported page

The optional `export` object in the complete example supplies `pageBg` for the
HTML canvas, `cardBg` for message cards and `infoBg` for information panels.
These fields accept the same color values and variables as terminal tokens.
An empty field means no explicit override. Without an override, the exporter
derives that background from the resolved `userMessageBg` color. If resolving
the export section fails, none of its explicit colors are used.

HTML export requires a prepared offline viewer bundle. Packaging that bundle
is tracked in [the export asset owner](https://github.com/Orchestration-Maestro/maestro/issues/2847);
a source checkout without it reports a missing-file export error. Once your
installation supplies it, run `/export` in the session and open the generated
HTML file locally. Check message foregrounds against each exported background,
as well as code and diffs.
Export has no terminal default: empty token values resolve to black only for a
theme named exactly `light`, and to `#e5e5e7` for every other name, including a
custom light-colored theme. Give foreground tokens explicit values if you need
the exported page to match your terminal. Read [HTML export](export.md) for
output behavior and viewer controls.
