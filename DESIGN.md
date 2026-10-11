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
  light-accent: "#B7410E"
  light-on-accent: "#F2E8DC"
  light-live: "#B7410E"
  light-success: "#B7410E"
  light-warning: "#2A1F1A"
  light-error: "#B7410E"
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
to be ported; it does not implement those browser components. Screen layouts,
wording and behaviour remain unchanged.

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

Within grouped families, states apply only to the components that support them;
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
| Editor | Default text, border; focus cursor; selected/completion accent, descriptions muted | [Editor](docs/terminal/editor.md) |
| SelectList | Default text, description/scroll muted; selected accent; no-match muted | [Widgets](docs/terminal/widgets.md) |
| SettingsList | Default text, description/hint muted; selected accent; submenu keeps its component styling | [Widgets](docs/terminal/widgets.md) |
| Markdown | Default text; headings/links/inline code accent; quotes muted/border; code text/border | [Markdown](docs/terminal/markdown.md) |
| Image | Default image protocol output; text fallback muted; no recolouring of image content | [Images](docs/terminal/images.md) |
| Loader, CancellableLoader | Busy live; message text; cancellation keeps existing input treatment | [Widgets](docs/terminal/widgets.md) |
| TUI overlays | Default background/panel; focus belongs to overlay child, border if already present | [Overlays](docs/terminal/overlays.md) |

### Terminal application components to port

These are not yet implemented terminal application surfaces. Reuse toolkit
components above; the theme-key adapter stays in
[brand-roles.json](crates/maestro-theme/assets/theme/brand-roles.json).

| Component family | States and role mappings |
| --- | --- |
| Footer, keybinding hints | Default text/muted; selected model/thinking accent; busy live; errors error; Footer context usage warning warning, critical error |
| User, assistant, skill-invocation, branch-summary, compaction-summary messages | Default text; user panel; thinking muted; streaming live; errors error |
| ToolExecution, BashExecution, diff | Default title text/output muted; pending live/panel; success success/panel; error error/panel; BashExecution cancellation and output-truncation notices warning; diff additions success, removals error, context muted |
| CustomMessage | Default panel/text and label accent; other states supplied by registered renderer |
| DynamicBorder, BorderedLoader, CountdownTimer | Default border; busy live/accent; existing cancellation/focus treatment only |
| CustomEditor, ExtensionEditor, ExtensionInput | Default text/border; focus accent/cursor; editor completions selected accent |
| ExtensionSelector, ConfigSelector, ModelSelector, OAuthSelector, ScopedModelsSelector | Default text/muted; focused/selected accent; errors error; busy live where loading exists; ModelSelector current-model sign success; OAuthSelector configured/matching auth success, differing auth warning, unconfigured muted |
| SessionSelector, SessionSelectorSearch, TreeSelector, UserMessageSelector | Default text/muted; focus/selected accent; search no-match muted; SessionSelector loading accent, deletion confirmation error, status accent/error |
| SettingsSelector, ShowImagesSelector, ThemeSelector, ThinkingSelector | Default text/muted; focus/selected accent; preview keeps selected theme's own colours |
| LoginDialog | Default text/muted; focus/selected accent; error error; busy live; login instructions warning, URL accent, open-link hint muted |

### Browser and exported-viewer inventory to port

These names identify the existing frontend contract, not delivered Rust UI.
The browser author build is tracked in [browser scaffolding](https://github.com/Orchestration-Maestro/maestro/issues/113);
[document tools](https://github.com/Orchestration-Maestro/maestro/issues/237)
and [browser storage](https://github.com/Orchestration-Maestro/maestro/issues/236)
retain their own application policy.

| Component family | States and role mappings |
| --- | --- |
| ChatPanel, AgentInterface | Default background/text; busy/streaming live; errors error; child controls focus/hover accent |
| MessageList, UserMessage, AssistantMessage, ToolMessage, ToolMessageDebugView, AbortedMessage, StreamingMessageContainer | Default text/panel; streaming live; tool success success, errors error; interactive controls focus/hover accent |
| Input, MessageEditor | Default text/panel/border; focus accent; hover accent on controls; disabled muted; sending busy live; errors error; MessageEditor drag-over accent border/text on panel, file-processing indicator muted |
| ThinkingBlock, ExpandableSection, ConsoleBlock | Default text/muted; toggle focus/hover accent; expanded content text/panel; errors error where output carries errors |
| AttachmentTile, AttachmentOverlay | Default panel/text/border; selected/focus/hover accent on controls; loading live; errors error |
| CustomProviderCard, ProviderKeyInput | Default panel/text/muted; focus/hover accent; disabled muted; validation error error; CustomProviderCard checking warning, connected success, disconnected error with muted status text; ProviderKeyInput testing muted, saved-key sign success |
| ModelSelector, ProvidersModelsTab | Default text/muted; focus/hover/selected accent; loading live; errors error |
| CustomProviderDialog, ApiKeyPromptDialog, SettingsDialog, ApiKeysTab, ProxyTab | Default background/panel/text/border; focus/hover accent; disabled muted; validation error error; busy live where saving/checking exists; SettingsDialog selected sidebar tab text/panel, selected mobile tab text/accent border, inactive tabs muted |
| PersistentStorageDialog, SessionListDialog | Default text/panel; focus/hover/selected accent on controls; denied/error error; loading live where supported |
| SandboxIframe, ConsoleRuntimeProvider, ArtifactsRuntimeProvider, AttachmentsRuntimeProvider, FileDownloadRuntimeProvider | Frame border/background; default content stays sandbox-owned; busy live/error error in host status only |
| Tool renderers: BashRenderer, DefaultRenderer, CalculateRenderer, GetCurrentTimeRenderer, JavaScript REPL, ExtractDocument | Default text/panel/border; pending live; result success/error; code uses mono, descriptions muted; expand controls focus/hover accent |
| ArtifactElement, ArtifactPill, artifacts tool renderer, Console; HtmlArtifact, SvgArtifact, ImageArtifact, MarkdownArtifact, TextArtifact, GenericArtifact, PdfArtifact, DocxArtifact, ExcelArtifact | Default container panel/text/border; source/preview/download and sheet controls focus/hover/selected accent; loading live; errors error; document colours stay document-owned |
| Export viewer sidebar, search/filter, tree, content, image modal | Default background/panel/text/border; focus/hover/selected accent on controls; tool/error content uses success/error; no streaming state in a stored session |

### Landed OAuth completion page

Default canvas resolves `modes.dark.colors.background`; heading resolves text,
paragraph/details muted, and mark parts resolve their named mark roles.
Success and failure use their existing headings/messages, not a new colour
state. There are no controls, hover, selection, disabled or streaming states.
The decorative mark has an accessible title from the pack wordmark.

## Do's and Don'ts

- Do resolve brand values through the selected pack and link the owning rules.
- Do preserve the established layout, wording and interaction states.
- Don't add colours, generated ramps, interpolated text, new motion or controls.
- Don't treat this inventory as evidence of shipped browser/application UI.
