# Browser utility library

`maestro-web` provides caller-owned localization, numeric display text and SVG
icons, plus re-exports of application-owned attachment records and conversion. It is a library, not an application entry point.

## Localization

Create an `I18n` with an implementation of `LanguageHost`; browser callers can
use `BrowserLanguageHost`. A nonempty supported stored language wins. Otherwise,
selection uses the browser locale's prefix before `-`, reading the legacy locale
only when the browser locale is absent or empty. Unsupported selection uses `en`.
Required host read failures propagate. `set_language` writes the exact supplied
code before reload; failed writes prevent reload, while failed reloads retain the
write.

`get_translations`, `translations`, `default_english` and `default_german` return
shared handles. `set_translations` replaces the current table, not existing
handles or the original application table. Editing a default dictionary does not
change the application dictionary constructed earlier. Lookup returns text,
shared nested categories or `Rc` callable leaves without invoking them. Generic
leaf types admit caller-defined signatures. Empty or missing messages report a
diagnostic and return the requested key; missing categories return the nested
key. Missing dictionaries and incompatible borrows fail. There is no per-key
English fallback or placeholder interpolation.

## Formatting

The crate root exports `format_cost`, `format_model_cost`, `format_usage` and
`format_token_count`. Prices and usage take the application's existing
[`ModelCost`, `Usage` and `UsageCost` records](../models/accounting.md).
Model prices read localization only when both effective prices are zero
(including absent or NaN prices). Usage emits nonzero, non-NaN input, output,
cache-read, cache-write and total cost, in that order. Total tokens and component
costs are not used to recompute the supplied total. Number spelling and fixed
rounding use `ryu-js`; token text uses `k`, never `M`.

## Icons

`icons()` loads 27 used glyphs. `icon_svg` serializes ordered primitive nodes
with literal names and attributes, adding no namespace bindings;
[`markup`](../../crates/maestro-text/src/markup.rs) owns XML attribute escaping.
`icon` renders the SVG through usvg/resvg into an egui image, accepting caller
color and device density. Native custom prefixes require caller-declared
namespace bindings; undeclared prefixes return `IconError`. `icon_dom` creates
SVG-namespaced browser elements directly and sets literal attributes without
XML parsing. Custom and empty primitive lists are also accepted.

Sizes xs/sm/md/lg/xl are 12/16/20/24/32 pixels at a 16px root; md is the default.
DOM callers install `ICON_STYLES` and supply their class suffix/color. Canvas
callers resolve colors from the [brand presentation](../theme.md); the
icon utility has no hover, focus, busy, disabled or selected state, font, mark or animation.

Shared CI compiles the library for WebAssembly through its wasm crate registration.
Controlled native tests prove utility results and host decision ordering, not
browser interaction.
Application launch and browser interaction remain separate delivery work.

## Attachments and browser messages

[Attachment tiles and overlays](attachments.md) provide retained browser previews,
text selection, original-byte downloads and the replaceable content host.


The root exports `Attachment`, `load_attachment`, `UserMessageWithAttachments`,
`ArtifactMessage`, `convert_attachments`, `default_convert_to_llm`,
`is_user_message_with_attachments` and `is_artifact_message`. Input enums and
supporting records remain in the public application modules. See
[application attachment policy](../application/browser-attachments.md).

## Input and timers

[Browser input and timers](components.md) reuse the attachment content host for
native editing and owned timer callbacks. The retained browser `Input` is
distinct from the terminal toolkit input.

## Sandbox messages

[Sandbox runtime messaging](sandbox.md) provides registered-window admission,
live runtime-provider dispatch and source generation for iframe and extension
channels. Native tests and WebAssembly compilation are distinct from browser
effect qualification.
