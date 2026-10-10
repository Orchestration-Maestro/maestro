# Maestro identity

Conduct every model.

Use [the default brand pack](../assets/brand/brand.json) for brand values.
[Signal](../assets/brand/packs/signal.json) is an alternate pack with the same
shape. This is Maestro's approved brand system. The files define its assets and
usage; they do not install a theme, change product messages or provide a runtime
pack loader.

## Voice

Write crisp, technical prose with rare dry wit. State the result, name the
constraint, give the next action. Use the vocabulary in [CONTEXT.md](../CONTEXT.md).
Prefer "run", "session", "model", "tool", "turn", "fix" and "check" to metaphors
in operational text. "Conduct" belongs in the tagline, not in function names.

Keep text terse unless the reader needs an explanation. A light line is allowed
in a documentation introduction or an empty state. Errors, rustdoc and code
comments stay literal. Do not use emoji, hype or claims such as "revolutionary",
"blazing-fast", "seamless" or "magic". Avoid "just" and "simply" when explaining
work the reader has to do.

Words we use: run, session, model, tool, turn, conduct, fix, check, land, green.
Words we avoid: blazing, revolutionary, magic, seamless, supercharge, just,
simply, leverage, utilize.

Product messages inherited from established behavior keep their wording; only
branding changes. These writing examples are not replacement product strings.
Never invent a command, flag or guarantee to make an example sound helpful.

### Product and documentation tone

| Context | Do | Don't |
| --- | --- | --- |
| Messages Maestro writes | "Patched. Tests green." Only after the patch and tests succeed. | "Awesome!! Everything is working perfectly now!" |
| Errors Maestro writes | "Cannot read settings.json: permission denied. Check the file permissions." | "Oops! Something went wrong. Try again later." |
| Documentation | "Open the session file to inspect its entries." An empty-state example: "No sessions yet. A quiet start." | "Unlock a revolutionary workflow with our seamless session experience." |
| Rustdoc | "Returns the selected model." | "This blazing-fast function supercharges model selection." |

### Contributor tone

| Context | Do | Don't |
| --- | --- | --- |
| Code comments | `// The open file keeps the lock tied to this write.` | `// Increment the counter.` when the next line says exactly that |
| Commit text | `fix(settings): retain keys after a failed write` | `fix: improve things` |
| Pull-request text | "Documents dark and light roles. All documented text pairs pass AA." Include the check command. | "Branding is perfect now." |
| Issue text | "Copper text fails contrast on the dark panel. Expected: at least 4.5:1. Reproduce with the pack's Copper/Oxide pair." | "Colors are broken. Please fix ASAP." |
| Test names | `settings_retain_keys_when_write_fails` | `test_happy_path`, `issue_123_fix` |

## Code identity

Keep established domain names and exported names, with Rust casing. Do not add
musical synonyms to the implementation. `SessionManager` and `SettingsManager`
are domain names; a new `ThingManager` hides its responsibility.

| Item | Pattern | Example |
| --- | --- | --- |
| Crates | `maestro-<noun>[-<role>]`; one job, nouns from the glossary | `maestro-theme`, `maestro-tui-crossterm` |
| Modules and Rust files | `snake_case`; the file names its responsibility | `agent_session.rs`, `session_manager.rs` |
| Types and traits | `PascalCase`; a domain noun | `AgentSession`, `ModelRegistry` |
| Functions and methods | `snake_case`; a concrete operation | `build_session_context` |
| Tests | `subject_behaviour_condition` | `settings_retain_keys_when_write_fails` |

The binary composition root is `maestro`. Crate roles are `-store`, a technology
suffix, or the `maestro-test-` prefix for test support. The
[foundation graph](specs/maestro-port.md#crates-and-delivery-order) owns the allowed
crate names and edges. A new name does not authorize a new crate.

Do not create grab-bag names: `utils`, `helpers`, `misc`, `common` or `manager`.
Retain `manager` only in an established domain name. Keep each mapped source file
in its corresponding Rust module; naming style is not a reason to rearrange the
public interface. Test names describe observable behavior, without leading issue
numbers or implementation details.

Comments explain a reason or invariant that the code cannot express clearly.
Use present tense. Delete comments that narrate syntax. No planning identifiers,
issue references, ticket numbers or delivery notes belong in Rust comments.

Rustdoc starts with what the item does. Use `# Errors` for fallible operations,
with the conditions that produce each documented error. Use `# Panics` only when
there is an actual panic condition; do not invent one to fill the section.
`# Examples` shows a small, valid use of the public interface when it helps the
caller. Explain behavior and constraints, not implementation history. Every
public item and crate root still needs the documentation required by
[AGENTS.md](../AGENTS.md#documentation).

## The conductor's compass

The mark is a ring with four compass ticks, a five-node M and a four-point North
Star. The ring is the stage for the work. The ticks suggest four beats and the
four modes: chat, print, JSON and RPC. The node-M connects models and tools into
one line of work. The North Star is the goal and the live signal above the M.
These are visual meanings, not limits on the number of tools or execution phases.

<img src="../assets/brand/dark/mark.svg" alt="Maestro mark on its dark canvas" width="128" height="128">

<img src="../assets/brand/light/mark.svg" alt="Maestro mark on its light canvas" width="128" height="128">

The [template](../assets/brand/mark.svg) retains the geometry of the
[organization's flat master](https://github.com/Orchestration-Maestro/.github/blob/main/assets/maestro-mark-flat.svg).
The organization's
[dark variant](https://github.com/Orchestration-Maestro/.github/blob/main/assets/maestro-mark-dark.svg)
is the reference for its metal treatment. The shipped renders are flat: the
center node and star use separate roles. Each render includes a pack-colored
canvas so an embedding page cannot change the mark's contrast.

### Construction and use

The view box is 512 by 512 units. The ring is centered at (256, 256), with radius
232 and stroke width 12. Each compass tick is 28 units long. The M's node columns
are 150, 256 and 362; its upper nodes are at y=196, center node at y=326 and lower
nodes at y=386. Bars have width 20; all five nodes have radius 26. The star is
centered at (256, 160). Keep these coordinates when recoloring the master.

Use at least `mark.minimum-size` (32 CSS pixels) for a full mark. Below that,
use the small template down to `mark.small-minimum-size` (16 CSS pixels).
Leave `mark.clear-space` times the rendered width outside the square canvas on
each side: 28/512, or one tick length in master units. Keep the same space between
the mark and the wordmark. These measurements live in each pack.

Keep the mark square and upright. Do not stretch, crop, add a shadow or change
stroke widths. Use the rendered asset for images; unresolved template variables
are not a distributable image. Give a standalone image an accessible name; use
empty alternative text when the adjacent wordmark already supplies the same
name. Do not use the mark alone to report a status.

### Five mark forms

| Form | Use |
| --- | --- |
| Metal | Brass-to-copper treatment with an Ember star and glow on dark hero, banner and splash surfaces. Gradient stops use pack roles. |
| Flat copper | [`mark-flat.svg`](../assets/brand/mark-flat.svg) for docs and interface chrome: ring/ticks 14 units, bars 22, nodes 28, with a mode-colored star. |
| Mono | [`mark-mono.svg`](../assets/brand/mark-mono.svg) for print and single-color contexts: ring/ticks 16 units, bars 26, nodes 30; all color comes from `currentColor`. |
| App icon | The mark centered at `mark.app-icon-scale` (72%) of a background-colored rounded square; use pack radii and the small form when needed. |
| Small size | [`mark-small.svg`](../assets/brand/mark-small.svg) below 32 pixels: no ticks or separate node circles; ring radius 220, ring stroke 40, M stroke 54 and a simplified star. |

The three variant templates are transparent. Place the flat/small forms on the
pack's `background`; resolve the mono foreground from `text`. Their geometry is
fixed. The original role-colored template and its two default renders remain
available. Metal and app-icon treatments are defined here for their owning
surface implementations; this pack does not include a renderer for them.

### Live status and glyphs

The star pulses while working and settles when idle: Ember and Brass in Forge
dark mode. Resolve working through `live` and idle through `accent` in every
pack and mode. The ring shows measured progress; the nodes indicate tool
activity. Reduced motion gives a static mark with the same text status. Do not
invent completion percentages, execution phases or timing guarantees.

The plain-text mark is the North Star, `✦`. Symbols and their color-role aliases
live in `glyphs`, so a pack swap can replace them too.

| Glyph role | Sign and label | Color role |
| --- | --- | --- |
| mark | `✦` with the wordmark | accent |
| working | `✦ working` | live |
| idle | `✦ idle` or `✦ done` | accent |
| passed | `✓ passed` | success |
| failed | `✗ failed` | error |
| tool-step | `┃` beside a named tool step | muted |

These are functional signs, not decorative emoji. Keep a text label, check font
support and terminal cell widths, and never convey status through motion or
color alone. All glyph foreground/background ratios follow their aliases in
the text-contrast table below. The theme and frontend implementations own motion
and status rendering; no product behavior or animation ships in this change.
Established product wording remains unchanged.

## Color roles

Raw colors belong in `palette`; every semantic role in `modes` refers to one
palette key. Consumers select a role, not a swatch. These are the complete
palettes; do not add tinted, translucent or interpolated text colors.

| Forge swatch | Hex | Main use |
| --- | --- | --- |
| Forge Black | `#0E0B09` | Dark canvas, light-mode text |
| Oxide | `#2A1F1A` | Dark panels, light-mode secondary text |
| Rust Copper | `#B7410E` | Mark; accessible accent text on Bone |
| Burnished Brass | `#D9A066` | Dark accent and secondary text |
| Molten Ember | `#FF6A1A` | Dark activity and status text |
| Vector Steel | `#5E7383` | Borders, never body text on dark |
| Bone | `#F2E8DC` | Dark text, light canvas |

| Signal swatch | Hex | Main use |
| --- | --- | --- |
| Carbon | `#0B0D10` | Dark canvas, light-mode text |
| Graphite | `#1A1F27` | Dark panels, light-mode secondary text |
| Frost | `#E8ECF2` | Dark text, light canvas |
| Indigo | `#6C63FF` | Mark and borders |
| Lime | `#B6F05A` | Dark accent and activity text |
| Fault | `#FF5C7A` | Dark warning and error text |

`text` is ordinary content; `muted` is secondary content, not lower opacity.
`accent` identifies emphasis and links; underline links rather than relying on
color. `live`, `success`, `warning` and `error` express status alongside words or
shapes. Some roles share a swatch to preserve contrast within the fixed palette.
Use labels to distinguish them. `border` is for visible non-text boundaries.
For an accent-filled control, pair `accent` with `on-accent`, not with `text`.

Both light modes use the same swatch for `background` and `panel`. Separate
panels with spacing or the border role, rather than inventing a lighter swatch.
Typography and spacing establish hierarchy; avoid nested cards, glowing borders,
gradient text and color-only status indicators.

### Text contrast

All ratios below were computed with Python using the WCAG sRGB relative
luminance method. Normalize each 8-bit channel to c in [0, 1]; linearize with
c/12.92 when c <= 0.04045, otherwise ((c + 0.055)/1.055)^2.4. Luminance is
0.2126R + 0.7152G + 0.0722B. Contrast is (lighter + 0.05)/(darker + 0.05).
Compare unrounded ratios to 4.5:1 for ordinary text; display two decimal places.
This meets [WCAG AA text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html),
not a claim of whole-interface accessibility.

The foreground roles below are valid on both named surfaces. Columns contain
ratios to 1. Forge dark surfaces are Forge Black/Oxide; Forge light surfaces are
Bone/Bone. Signal dark surfaces are Carbon/Graphite; Signal light surfaces are
Frost/Frost.

| Pack and mode | Foreground roles | Palette key | Background | Panel |
| --- | --- | --- | ---: | ---: |
| Forge dark | text | bone | 16.21 | 13.25 |
| Forge dark | muted, accent, warning | burnished-brass | 8.57 | 7.00 |
| Forge dark | live, success, error | molten-ember | 6.85 | 5.60 |
| Forge light | text | forge-black | 16.21 | 16.21 |
| Forge light | muted, warning | oxide | 13.25 | 13.25 |
| Forge light | accent, live, success, error | rust-copper | 4.60 | 4.60 |
| Signal dark | text, muted | frost | 16.41 | 13.95 |
| Signal dark | accent, live, success | lime | 14.45 | 12.29 |
| Signal dark | warning, error | fault | 6.54 | 5.57 |
| Signal light | text, warning, error | carbon | 16.41 | 16.41 |
| Signal light | muted, accent, live, success | graphite | 13.95 | 13.95 |

The only additional text/background pairing is `on-accent` on `accent`:

| Pack and mode | Foreground / background palette keys | Ratio |
| --- | --- | ---: |
| Forge dark | forge-black / burnished-brass | 8.57 |
| Forge light | bone / rust-copper | 4.60 |
| Signal dark | carbon / lime | 14.45 |
| Signal light | frost / graphite | 13.95 |

Rust Copper on Forge Black is 3.53:1; Vector Steel is 3.98:1. Neither is body
text on dark. The 3:1 threshold permits large text (at least 24 CSS pixels, or
18.67 bold) and necessary non-text graphics, but Copper on Oxide is only 2.88:1.
Keep the dark Copper mark on its supplied canvas, not directly on an Oxide panel.
Copper on Bone is 4.60:1 and is valid light-mode body text. Steel on Bone is
4.08:1, so it remains a border rather than body text there too.

Signal's Indigo is 4.51:1 on Carbon, 3.83:1 on Graphite and 3.64:1 on Frost.
It cannot be a text role used across those surfaces. Lime replaces it for dark
accent text; Graphite replaces it for light accent text. None of these fixes
changes a palette color.

### Mark and border contrast

These non-text roles meet the
[3:1 graphics threshold](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)
on their stated surfaces. Mark ratios use the pack's `background`, built into
the default renders. Flat and small use the same role ratios on that background;
mono uses the `text` ratios above. Marks are not text roles. A logo's exemption
does not make its colors safe for text.

| Pack and mode | Mark roles | Canvas ratio |
| --- | --- | ---: |
| Forge dark | mark-ring, mark-m, mark-node | 3.53 |
| Forge dark | mark-live, mark-star | 6.85 |
| Forge light | all five mark roles | 4.60 |
| Signal dark | mark-ring, mark-m, mark-node | 4.51 |
| Signal dark | mark-live, mark-star | 14.45 |
| Signal light | mark-ring, mark-m, mark-node | 3.64 |
| Signal light | mark-live, mark-star | 16.41 |

| Border role | Background ratio | Panel ratio |
| --- | ---: | ---: |
| Forge dark | 3.98 | 3.25 |
| Forge light | 4.08 | 4.08 |
| Signal dark | 4.51 | 3.83 |
| Signal light | 3.64 | 3.64 |

### Terminal roles

`terminal.ansi` contains all 16 named slots. Normal and bright slots deliberately
share accessible foreground roles. The names identify ANSI slots, not literal
hue requirements: the limited palettes do not supply sixteen distinct hues.

| Indices | ANSI slots | Foreground role |
| --- | --- | --- |
| 0, 8 | black, bright-black | muted |
| 1, 9 | red, bright-red | error |
| 2, 10 | green, bright-green | success |
| 3, 11 | yellow, bright-yellow | warning |
| 4, 12 | blue, bright-blue | accent |
| 5, 13 | magenta, bright-magenta | accent |
| 6, 14 | cyan, bright-cyan | live |
| 7, 15 | white, bright-white | text |

`terminal.truecolor` maps the named background, panel and seven foreground roles
to the same mode colors. Thus every ANSI foreground and every truecolor text
role has the ratios in the text table on both supported backgrounds. Arbitrary
ANSI foreground/background combinations, inverse video, user-customized terminal
palettes and 256-color quantization are not covered by those ratios. Check the
actual rendered combinations in the terminal implementation. Keep existing
terminal theme names, color keys, format and custom-theme loading unchanged;
this pack supplies defaults, not a replacement terminal theme format.

The shipped terminal themes derive each of their 51 color keys from one
semantic role, as data in the theme library's `brand-roles.json`:

| Semantic role | Terminal keys |
| --- | --- |
| `accent` | `accent`, `borderAccent`, `customMessageLabel`, `mdHeading`, `mdLink`, `mdCode`, `mdListBullet`, `syntaxKeyword`, `syntaxFunction`, `thinkingLow`, `thinkingMedium` |
| `border` | `border`, `borderMuted`, `mdCodeBlockBorder`, `mdQuoteBorder`, `mdHr`, `thinkingOff` |
| `success` | `success`, `toolDiffAdded`, `syntaxString`, `bashMode` |
| `error` | `error`, `toolDiffRemoved` |
| `warning` | `warning`, `syntaxNumber` |
| `muted` | `muted`, `dim`, `thinkingText`, `toolOutput`, `mdLinkUrl`, `mdQuote`, `toolDiffContext`, `syntaxComment`, `thinkingMinimal` |
| `text` | `mdCodeBlock`, `syntaxVariable`, `syntaxOperator`, `syntaxPunctuation` |
| `live` | `syntaxType`, `thinkingHigh`, `thinkingXhigh` |
| `panel` | `selectedBg`, `userMessageBg`, `customMessageBg`, `toolPendingBg`, `toolSuccessBg`, `toolErrorBg` |
| terminal default (empty) | `text`, `userMessageText`, `customMessageText`, `toolTitle` |

Backgrounds use `panel`, so existing text is not paired with a newly inverted
foreground. Several thinking levels share a color in a small palette.

## Typography and spacing

Forge uses Barlow Condensed for display, Barlow for body text and JetBrains Mono
for code and measurements. Signal uses Space Grotesk for display/body and
JetBrains Mono for code. Each font entry records its SIL Open Font License 1.1
identifier, upstream license URL and ordered fallbacks. Font files are not
bundled here. When distributing them, include their copyright and license
notices. Do not make viewing an exported page depend on a remote font service.
The terminal's font stays under the user's control.

| Type role | Font role | Size (CSS px) | Weight | Line height |
| --- | --- | ---: | ---: | ---: |
| display-xl | display | 56 | 700 | 1.1 |
| display | display | 36 | 700 | 1.1 |
| heading | display | 24 | 600 | 1.2 |
| body | body | 16 | 400 | 1.55 |
| mono | mono | 14 | 400 | 1.7 |

Both packs use this scale. Keep capitals to short display labels, not paragraphs.
Do not force monospace onto ordinary prose. Allow text to reflow and respect
browser zoom. Use the ordered fallback stack if the selected font is unavailable.

Spacing tokens `xs`, `sm`, `md`, `lg`, `xl` are 4, 8, 16, 24 and 32 CSS pixels.
Use small gaps within a group and larger gaps between groups. Forge radii
`none`, `control`, `panel` are 0, 4 and 8; Signal uses 0, 6 and 12. These are
presentation values, not permission to change an established interface's layout.

## Pack shape and swapping

The JSON is plain data with one current shape. There is no inheritance, expression
language or implicit palette fallback. Forge and Signal have identical role
keys; swatch keys and values belong to each pack.

| Field | Shape and meaning |
| --- | --- |
| `name`, `wordmark`, `tagline` | Strings: pack label, displayed product name and tagline |
| `palette` | Map of swatch keys to opaque six-digit sRGB hex strings |
| `modes.dark.colors`, `modes.light.colors` | Map of semantic role names to keys in that pack's palette |
| `fonts` | `display`, `body`, `mono`: each has `family`, `license`, `license-url`, `fallbacks` (ordered string array) |
| `type` | Map of type roles to a `font` key, positive pixel `size`, unitless `line-height` and numeric `weight` |
| `spacing`, `radii` | Maps of role names to nonnegative CSS pixel values |
| `mark` | `template` and `variants` paths relative to the JSON file; `minimum-size` and `small-minimum-size` in CSS pixels; `clear-space` and `app-icon-scale` as fractions of rendered width |
| `glyphs` | Map of glyph roles to a `symbol` string and a `color` mode-role alias |
| `terminal` | `ansi` and `truecolor` maps whose values are mode color role names |

Resolve a color in two lookups: `modes[mode].colors[role]`, then `palette[key]`.
For an ANSI or truecolor entry, first resolve its terminal alias to that role.
Resolve a type entry's `font` through `fonts`, then use its family and fallbacks.
Resolve each glyph's `color` through the same mode roles. Do not branch on pack
names, swatch names or font families.

### Template substitution contract

| Template reference | Pack source | Part |
| --- | --- | --- |
| `{{wordmark}}` | `wordmark`, XML-escaped as text | SVG title |
| `var(--background)` | Resolved `background` color | Backing canvas |
| `var(--mark-ring)` | Resolved `mark-ring` color | Ring and four ticks |
| `var(--mark-m)` | Resolved `mark-m` color | Four bars |
| `var(--mark-node)` | Resolved `mark-node` color | Four outer nodes |
| `var(--mark-live)` | Resolved `mark-live` color | Center node |
| `var(--mark-star)` | Resolved `mark-star` color | North Star |
| `currentColor` | Resolved `text` color | Every stroke and fill in the mono variant |

The default template uses all five mark roles. Flat uses `mark-ring`, `mark-m`,
`mark-node` for all five nodes, and `mark-star`; small uses `mark-ring`, `mark-m`
and `mark-star`. Mono uses only `currentColor`. For inline mono SVG, set `color`
on its host; for a standalone render, substitute the resolved `text` color.
External image elements do not inherit a page's `currentColor`.

Read the template named by the selected pack, or a path in `mark.variants`.
Replace each reference with the selected mode's resolved value, preserving
geometry. Export an SVG with no unresolved references. The default outputs are
[`dark/mark.svg`](../assets/brand/dark/mark.svg) and
[`light/mark.svg`](../assets/brand/light/mark.svg). They are derived files, not
independent color settings. The template contains no hex colors.

### Switch the assets

1. Select `assets/brand/packs/signal.json`, or make a complete custom JSON pack
   with the same keys. Keep the file beside its referenced template, or adjust
   `mark.template` to its new relative location.
2. Resolve both modes and substitute the template references above. Save the
   dark/light renders to the paths used by the README and docs. Resolve variant
   paths from the same JSON directory. This is a data substitution; neither
   template geometry nor consumer code changes.
3. Take the wordmark and tagline from the selected pack when updating static
   README or documentation text. Static files do not read JSON at page-view time.
4. Recompute every allowed contrast pair with the method above, including ANSI
   aliases, accent-filled controls and the mark canvas. Inspect both modes at
   the intended size before publishing.

The repository ships these data files and static renders, and the theme
library reads a selected pack: it derives the shipped dark and light terminal
themes, resolved presentation values, CSS custom properties and filled mark
templates ([theme library](theme.md#brand-pack)). It has no pack-selection
command, renderer or bundled font files. Web and export implementations must read
the same selected pack. Every runtime consumer must read the selected
pack and never hard-code brand colors, fonts, sizes, wordmark, tagline, glyphs or
mark paths. The theme library's tests prove a same-format pack swap reaches its
outputs without a code change. A brand swap must not change behavior, message
wording, custom themes or the user's terminal font.
