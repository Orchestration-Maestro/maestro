---
name: Maestro Forge
description: Design roles on the approved Maestro brand
colors:
  dark-background: "#0E0B09"
  dark-panel: "#2A1F1A"
  dark-text: "#F2E8DC"
  dark-muted: "#D9A066"
  dark-accent: "#D9A066"
  dark-on-accent: "#0E0B09"
  dark-live: "#FF6A1A"
  dark-success: "#FF6A1A"
  dark-warning: "#D9A066"
  dark-error: "#FF6A1A"
  dark-border: "#5E7383"
  dark-mark-ring: "#B7410E"
  dark-mark-m: "#B7410E"
  dark-mark-node: "#B7410E"
  dark-mark-live: "#FF6A1A"
  dark-mark-star: "#FF6A1A"
  light-background: "#F2E8DC"
  light-panel: "#F2E8DC"
  light-text: "#0E0B09"
  light-muted: "#2A1F1A"
  light-accent: "#2A1F1A"
  light-on-accent: "#F2E8DC"
  light-live: "#2A1F1A"
  light-success: "#2A1F1A"
  light-warning: "#2A1F1A"
  light-error: "#2A1F1A"
  light-border: "#5E7383"
  light-mark-ring: "#B7410E"
  light-mark-m: "#B7410E"
  light-mark-node: "#B7410E"
  light-mark-live: "#B7410E"
  light-mark-star: "#B7410E"
---

# Design system: Maestro

## Overview

The approved Forge identity governs presentation, not interaction. Read the
[identity guide](docs/identity.md) and [brand pack](assets/brand/brand.json)
for normative values, voice, mark geometry and contrast requirements.
This inventory records existing terminal widgets and the browser components
to be ported; it does not implement those browser components. Screen layouts
and behaviour remain unchanged.

## Colors

Frontmatter is a checked projection for design tooling, not a second token source.
Each `colors.<mode>-<role>` resolves from
`modes.<mode>.colors.<role>` through `palette` in the brand pack. The role names
are the semantic token layer; components choose roles rather than swatches.
Resolve the selected pack at use time; do not copy the frontmatter into code.

| Role | Component use |
| --- | --- |
| background, panel | Page canvas and message/tool/selection surfaces |
| text, muted | Content and secondary descriptions |
| accent, on-accent | Emphasis, links and accent-filled controls |
| live, success, warning, error | Activity and labelled outcomes |
| border | Non-text boundaries |
| mark-ring, mark-m, mark-node, mark-live, mark-star | Supplied mark parts only |

Select `dark-*` or `light-*` together. Contrast rules and allowed pairs belong to
[Text contrast](docs/identity.md#text-contrast) and
[Mark and border contrast](docs/identity.md#mark-and-border-contrast).

## Typography

Browser roles resolve `type.display-xl`, `type.display`, `type.heading`,
`type.body` and `type.mono`; each entry selects its font via `fonts[entry.font]`.
Use `fonts.display` for headings, `fonts.body` for prose and `fonts.mono` for
code/details, including their ordered fallbacks. The scales and font licence
requirements live in [Typography and spacing](docs/identity.md#typography-and-spacing).

The OAuth page currently resolves those font families, but keeps its established
heading/paragraph/details metrics in
[oauth_page.rs](crates/maestro-models/src/oauth/oauth_page.rs). This document
does not claim it already consumes the type or spacing scales.

Terminal font selection remains the user's. Bold carries heading/selection
emphasis, dim carries secondary treatment where the existing widget uses it,
and italic carries existing emphasis/thinking treatment; these attributes do
not add a new state or replace the owning widget's formatting. The theme's
[terminal role table](docs/identity.md#terminal-roles) defines the ANSI and
truecolor aliases. `terminal.ansi` and `terminal.truecolor` are pack projections
exposed as presentation data, not rendering paths. Runtime truecolor and
256-color rendering are owned by
[the theme library's color modes](docs/theme.md#color-modes), not a new pack palette. Quantized and
user-controlled ANSI combinations need rendered contrast checks; the pack's
sRGB contrast table does not certify them.

## Layout

Browser spacing aliases are `spacing.xs`, `spacing.sm`, `spacing.md`,
`spacing.lg` and `spacing.xl`. Resolve them from the pack only where the
existing surface has spacing; they do not authorize a layout change. Terminal
padding, rows, columns, wrapping, overlays and cursor placement remain with
[the terminal toolkit](docs/terminal/components.md).

## Elevation & Depth

Use existing background/panel roles and border treatment. The light pack's
shared canvas/panel colour is not permission to invent another swatch.
The OAuth completion page is flat; this inventory adds no shadows or layers.

## Shapes

Browser shape aliases are `radii.none`, `radii.control` and `radii.panel`.
Mark aliases are `mark.template`, `mark.variants.flat`, `mark.variants.mono`,
`mark.variants.small` and the measurements under `mark`. Follow
[Construction and use](docs/identity.md#construction-and-use); terminal
glyph symbols and colour aliases resolve through `glyphs`.

## Components

### State notation

In the tables below, a colour role means `colors.<selected-mode>-<role>`
resolved through the pack. These are presentation mappings for the states the
component actually supports, not a promise of new interactions.

| State | Token mapping |
| --- | --- |
| Default | text on background; existing boxed/message surfaces use panel and border |
| Focus | accent and existing cursor/border emphasis |
| Hover (browser interactive controls only) | accent; preserve existing hover affordance, no new tint |
| Selected | accent emphasis; existing filled selections use panel, or on-accent on accent |
| Disabled (only where supported) | muted with existing disabled affordance |
| Error | error with the existing error text/sign; tool background remains panel |
| Streaming/busy | live with existing indicator/text; pending background remains panel |

The component rows below override the generic notation where their existing
presentation differs. Within grouped families, states apply only to components
that support them;
component-specific outcomes are listed in their rows below. Focus/hover/disabled
styling is not invented for passive components. Status signs use `glyphs` only where the
surface already has a sign; wording and animation stay with their owners.

### Landed terminal toolkit

Toolkit themes are caller-supplied style callbacks. This table tells a branded
caller which pack roles to use, without changing the widget APIs.

| Component | Supported presentation states and roles | Owner |
| --- | --- | --- |
| Container | Default: child-owned styling; no own visual state | [Components](docs/terminal/components.md) |
| Box | Default: panel background; child-owned text; no input state | [Widgets](docs/terminal/widgets.md#box) |
| Text, TruncatedText | Default: text; optional caller background panel | [Widgets](docs/terminal/widgets.md) |
| Spacer | Default: empty rows; no colour token | [Widgets](docs/terminal/widgets.md#spacer) |
| Input | Default text; focus uses existing cursor; no hover/disabled/error state | [Widgets](docs/terminal/widgets.md#input) |
| Editor | Default text and border, including scroll indicators; inverse-video cursor, emits a cursor-position marker when focused with completion closed; hardware-cursor visibility belongs to [rendering](docs/terminal/rendering.md); completion selection accent, descriptions/count/no-match muted | [Editor](docs/terminal/editor.md) |
| SelectList | Default text, description/scroll muted; selected accent; no-match muted | [Widgets](docs/terminal/widgets.md) |
| SettingsList | Default label text and value muted; selected cursor/label/value accent; description/hint/scroll/empty/no-match muted; optional search uses Input, submenu keeps its component styling | [Widgets](docs/terminal/widgets.md) |
| Markdown | Default text; headings/links/inline code/list bullets accent, link URL muted; quotes muted/border and horizontal rules border; code text/border with caller highlighting, bold/italic/underline retained | [Markdown](docs/terminal/markdown.md) |
| Image | Default image protocol output; text fallback muted; no recolouring of image content | [Images](docs/terminal/images.md) |
| Loader, CancellableLoader | Busy live; message text; cancellation keeps existing input treatment | [Widgets](docs/terminal/widgets.md) |
| TUI overlays | Default background/panel; focus belongs to overlay child, border if already present | [Overlays](docs/terminal/overlays.md) |

### Terminal application components to port

These are not yet implemented terminal application surfaces. Reuse toolkit
components above; the theme-key adapter stays in
[brand-roles.json](crates/maestro-theme/assets/theme/brand-roles.json).

| Component | States and role mappings |
| --- | --- |
| Footer | Path, usage, model/thinking and ordinary context statistics muted; context usage above 70% warning, above 90% error; extension status text keeps its supplied styling |
| Keybinding hints | Keys and descriptions muted; no own busy/error state |
| UserMessage | Text on panel; Markdown styling belongs to the toolkit |
| AssistantMessage | Text; visible/hidden thinking muted and italic; aborted/error text error; streaming updates content without an extra status colour |
| SkillInvocationMessage | Collapsed label accent, name text, expand hint muted; expanded label accent and content text on panel |
| BranchSummaryMessage, CompactionSummaryMessage | Collapsed/expanded label accent and content text on panel; collapsed expand key muted |
| ToolExecution | Default/generic shell partial, successful and error backgrounds panel; fallback title text, output/image fallback muted; collapsed/expanded content keeps the tool renderer's styling; self-shell renderers own framing, including visible content; hidden content renders no surface |
| BashExecution | Initial command header, border and spinner success, or muted when excluded from context; rebuilt command header success even while running and excluded; running message/output muted; cancelled/truncated notices warning, nonzero exit error; collapsed hidden-line hint muted, expanded output muted |
| Diff | Additions success, removals error, context muted |
| CustomMessage | Default label accent and content text on panel; collapsed/expanded custom rendering belongs to the registered renderer |
| DynamicBorder | Border by default; caller-supplied colour when overridden; no own input/busy state |
| BorderedLoader | Border; cancellable/noncancellable spinner accent, message muted; cancellation adds no outcome colour |
| CountdownTimer | No visual surface; countdown text uses the owning dialog's title style |
| CustomEditor | Toolkit Editor styling; app keybindings add no visual state |
| ExtensionEditor | Border and accent title; child Editor text/cursor/completions use toolkit styling |
| ExtensionInput | Border and accent title/countdown; child Input text/cursor |
| ExtensionSelector | Border and accent title/countdown; selected arrow/option accent, other options text |
| ConfigSelector | Text and muted filter/hints/no-results/count; group headers accent, subgroup headers muted; selected name bold; enabled mark success, disabled mark muted (resource enabled state, not disabled input) |
| ModelSelector | Text; selected arrow/model and active scope accent; inactive scope, provider badges, model description, scroll count and no-match muted; current-model sign success; configured-provider hint warning; model-loading error text error |
| OAuthSelector | Accent title and selected provider; other names text, scroll/no-match/unconfigured muted; matching configured credentials and environment/runtime/fallback/model-file auth signs success; different configured auth warning with muted separator |
| ScopedModelsSelector | Accent title and selected arrow/model; other models text; provider badges, hints, count/no-match muted; enabled mark success, disabled mark muted, marks omitted when all enabled; unsaved warning |
| SessionSelector | Accent borders, selected cursor and current session; selected row panel with bold message; named noncurrent session warning; prefixes/metadata/hints/no-results muted; active scope/sort/name filters and loading progress accent; delete confirmation/message/metadata error; other status accent or error |
| SessionSelectorSearch | Nonvisual search parsing/filtering; no own component tokens; SessionSelector owns search Input and results |
| TreeSelector | Selected cursor, active-path and fold marks accent; selected row panel with bold content; connectors/timestamps/search hints/count/no-entries muted; search query accent; attached labels and branch-summary label warning; user/custom/compaction labels accent; assistant label success with text content, muted aborted/no-content, or error text; tool results, bash, model/thinking changes, custom records, label changes and session titles muted; label editor prompt/hints muted with child Input cursor |
| UserMessageSelector | Selected cursor accent and message bold; other messages text; metadata/count/empty/help muted; borders use their existing caller styling |
| SettingsSelector | Accent title, muted description/hints; child SettingsList selection and submenu styling |
| ShowImagesSelector, ThinkingSelector | Border; toolkit SelectList text/selected accent/description muted; no own loading/error outcome |
| ThemeSelector | Border; toolkit SelectList text/selected accent/description muted; preview retains the selected theme's colours |
| LoginDialog | Border and accent title/URL; optional instructions warning; manual prompts/placeholders, waiting/progress and cancel/open-link hints muted, prompt text text; child Input cursor; no own rendered success/error or spinner state |

### Browser and exported-viewer inventory to port

These names identify the existing frontend contract, not delivered Rust UI.
The browser author build is tracked in [browser scaffolding](https://github.com/Orchestration-Maestro/maestro/issues/113);
[document tools](https://github.com/Orchestration-Maestro/maestro/issues/237)
and [browser storage](https://github.com/Orchestration-Maestro/maestro/issues/236)
retain their own application policy.

Browser controls retain their existing focus, hover, disabled and pressed
structure. Map primary filled controls to on-accent on accent, secondary surfaces
to text on panel, muted descriptions/placeholders to muted, focus rings to accent
and destructive controls to error. Preserve existing opacity, visibility and
selection cues; these mappings add no controls or outcome branches. Code uses
`type.mono`; prose uses `type.body`. Document/image pixels and sandbox-authored
content are not brand colours.

| Component | States and role mappings |
| --- | --- |
| ChatPanel | Background/text; missing-agent text muted; artifacts-open badge on-accent on accent with existing count/hover treatment; child AgentInterface and ArtifactsPanel own their states |
| AgentInterface | Background/text; missing-session and statistics muted; clickable cost hover text; streaming visibility and editor controls belong to their children |
| MessageList | Child-owned message styling; no own loading/error surface |
| UserMessage | Text on panel; attachments and Markdown keep child styling |
| AssistantMessage | Content text; nonstreaming usage muted with text hover when clickable; error and aborted messages error; thinking/tools child-owned |
| ToolMessage | Default card panel/text/border, or no wrapper for custom rendering; pending/success/error and expansion belong to the selected tool renderer; debug mode uses ToolMessageDebugView |
| ToolMessageDebugView | Call/result labels and absent-result text muted; JSON/text code uses mono; no own outcome colour |
| AbortedMessage | Error text, italic |
| StreamingMessageContainer | Streaming placeholder/cursor muted with existing pulse; idle without a message renders nothing; message content uses its own component styling |
| Input | Text, muted placeholder, border; selected text on-accent on accent; focus accent ring; disabled retains existing opacity/pointer treatment; required sign and error text/border error; no own busy state |
| MessageEditor | Text on panel/border, muted placeholder; drag-over accent border/message with existing overlay; file-processing spinner muted; idle send control disabled for empty input/no attachments or processing files; streaming replaces send with abort; thinking/model/attachment controls keep existing selected/hover/focus treatment; no own rendered error outcome |
| ThinkingBlock | Collapsed header muted, hover text; streaming keeps existing shimmer in muted/text roles; expanded Markdown thinking content muted; chevron retains rotation; no error state |
| ExpandableSection | Collapsed/expanded toggle muted, hover text; expanded children own content styling |
| ConsoleBlock | Border and panel header, muted label/copy control; hover on-accent on accent; copied check retains control colour; output text or error by variant on background, mono |
| AttachmentTile | Preview image unchanged; filename/icon fallback muted on panel/border; PDF badge muted on background; thumbnail hover keeps opacity cue; optional remove control muted, hover text on panel; no loading/error indicator |
| AttachmentOverlay | Background backdrop and header/border/text; preview/extracted-text toggle and download/close controls keep focus/hover/selected treatment; extracted text text on panel, empty-text fallback text; rendered load error error text/border on panel; selected spreadsheet tab accent text/border, others muted with text/border hover; document pixels unchanged; no loading spinner |
| CustomProviderCard | Name text, metadata/model count/status description muted, border; auto-discovery checking sign warning, connected success, disconnected error; refresh/edit/delete ghost controls keep existing hover/focus treatment; no separate validation outcome |
| ProviderKeyInput | Text label; testing badge muted on panel; saved-key check success, invalid badge error; child password Input focus; save disabled while testing, empty, or when a key is saved and input has not been edited since initialization or the last successful save; no delete control |
| ModelSelector | Text model names, muted metadata/badges and unsupported capability icons with existing opacity; current-model check success; selected row panel; mouse-hover panel; active thinking/vision filters on-accent on accent, inactive text on panel; search Input focus; no own rendered loading/error outcome |
| ProvidersModelsTab | Text headings, muted descriptions/empty state, border separator; add/child refresh/edit/delete controls keep their styles; provider status belongs to CustomProviderCard, keys to ProviderKeyInput; load/delete failures are not rendered outcomes |
| CustomProviderDialog | Text headings and border; child inputs/select focus; test disabled while testing or missing base URL, save disabled for missing name/base URL; testing button text and discovered models muted; connection error error; manual-provider guidance muted; save failure uses existing alert, not a new inline outcome |
| ApiKeyPromptDialog | Text header and dialog surface; key testing/saved/invalid/disabled presentation belongs to ProviderKeyInput; no separate busy/error state |
| SettingsDialog | Background/panel/text/border; active desktop sidebar text on panel, active mobile tab text with accent border; inactive tabs muted with text/panel hover; selected child tab owns content; no own loading/error state |
| ApiKeysTab | Muted introduction; ProviderKeyInput owns each key's states |
| ProxyTab | Text label, muted explanation/hint; enabled switch on-accent on accent, disabled switch panel; proxy URL Input disabled while proxy off with existing opacity, otherwise text/focus; no rendered save/error outcome |
| PersistentStorageDialog | Text header, muted explanation; visible warning sign warning on panel with warning boundary; grant/continue controls keep focus/hover treatment; denial, unavailable API and request failure are logged, not rendered outcomes; no reachable requesting/disabled state |
| SessionListDialog | Text titles on panel/border, muted metadata; loading and empty text muted; row hover panel; delete affordance error and revealed by row hover; load/delete failures log only, no rendered error outcome or persistent selection state |
| SandboxIframe | Sandbox-authored content owns its pixels; host-authored HTML validation-error document uses background/text, error heading and panel error body in mono; no busy indicator |
| ConsoleRuntimeProvider, ArtifactsRuntimeProvider, AttachmentsRuntimeProvider, FileDownloadRuntimeProvider | Nonvisual runtime providers; no component colour/state mapping; consumer console, artifact or attachment owns presentation |
| Shared tool headers | Caption muted; pending tool sign/spinner live, complete sign success, error sign error; collapsible caption/toggle muted with text hover, chevron muted; embedded children retain their styling |
| BashRenderer | Shared tool header; command/output ConsoleBlock text or error; no expansion toggle |
| DefaultRenderer | Shared tool header: pending while streaming without a result, complete for a non-error result or nonstreaming call, error for an error result; muted labels, input/output code mono; no expansion toggle |
| CalculateRenderer, GetCurrentTimeRenderer | Shared tool header; successful output embedded in its muted caption, error output error below it; no expansion toggle |
| JavaScript REPL renderer | Shared collapsible tool header; code mono, console text/error by result, returned attachments child-owned |
| ExtractDocument renderer | Shared collapsible tool header; URL/metadata muted; nonempty successful extracted text in CodeBlock mono, nonempty error output in error ConsoleBlock |
| ArtifactsPanel | Background/text, tab-bar border; active tab accent text and underline, inactive tab muted with text hover and transparent underline; active artifact supplies header controls/content; close control keeps hover/focus treatment; collapsed or empty panel hidden; overlay has no side border and uses transparent modal admission; the chat screen's overlay fills the viewport above the shared dialog backdrop, inline uses border; no own busy/error outcome |
| ArtifactElement | Nonvisual base contract for content/header controls; concrete artifact owns presentation |
| ArtifactPill | Text filename on panel/border; hover panel and pointer only when a panel is supplied; no selected/loading/error state |
| Artifacts tool renderer | Shared tool header, collapsible where details exist; code mono, diff additions success/removals error, error text/console error; file pills child-owned |
| Console | Collapsed/expanded summary muted, hover text; expanded logs muted or error by log type, mono; autoscroll enabled on-accent on accent, off retains panel hover; copy control keeps hover treatment |
| HtmlArtifact | Preview sandbox owns pixels; code view mono on background; source/preview toggle keeps existing selected/hover/focus control treatment, run/copy/download controls keep focus/hover and copied cue; console shown only when logs exist, console owns errors |
| SvgArtifact, MarkdownArtifact | Preview SVG pixels unchanged or Markdown child-owned; source view mono; source/preview toggle keeps existing selected/hover/focus control treatment; copy/download controls keep focus/hover and copied cue; no own loading/error outcome |
| ImageArtifact | Image pixels unchanged on background; download control focus/hover; image-load failure displays its existing fallback image, not an error panel |
| TextArtifact | Text/code mono, syntax styling from code renderer; copy/download controls keep focus/hover and copied cue; no own loading/error outcome |
| GenericArtifact | Text filename and muted icon/no-preview guidance on background; download focus/hover; no own loading/error outcome |
| PdfArtifact, DocxArtifact | Background with document-owned pixels and border/separators; download focus/hover; rendered load error error text/border on panel; no loading indicator |
| ExcelArtifact | Background and text/border cells, panel headers/alternating rows; active sheet accent text/border, inactive muted with text/border hover; download focus/hover; rendered load error error text/border on panel; no loading indicator |
| Export viewer sidebar | Background/panel/text/border; usage/footer muted; selected entry panel, active path accent, entry labels accent/success/warning/error/muted by entry kind; mobile open/close controls muted with text/panel hover, resizer hover/drag panel; mobile backdrop background; no streaming state |
| Export viewer search/filter | Text and muted placeholder/hints; focus accent; active filter on-accent on accent, inactive muted; filter/close hover text and text border; no disabled/busy/error outcome |
| Export viewer tree | Text entries, muted connectors/metadata; selected/hovered panel with bold selected content, active-path accent treatment; off-path dimmed until hover; labels warning, user/skill/custom/compaction accent, assistant success with muted aborted/no-content or error; tool and state-change summaries muted |
| Export viewer content | Text on panel; tool output muted, success/error outcomes success/error; thinking muted; code mono and diff success/error/muted; expanded/collapsed controls keep hover treatment; document content unchanged |
| Export viewer image modal | Image mount only; image pixels unchanged; no modal styling, open/close handlers or loading/streaming outcome in the viewer |

### Landed OAuth completion page

Default canvas resolves `modes.dark.colors.background`; heading resolves text,
paragraph/details muted, and mark parts resolve their named mark roles.
Success and failure use their existing headings/messages, not a new colour
state. There are no controls, hover, selection, disabled or streaming states.
The decorative mark has an accessible title from the pack wordmark.

## Do's and Don'ts

- Do resolve brand values through the selected pack and link the owning rules.
- Do preserve the established layout and interaction states.
- Don't add colours, generated ramps, interpolated text, new motion or controls.
- Don't treat this inventory as evidence of shipped browser/application UI.
