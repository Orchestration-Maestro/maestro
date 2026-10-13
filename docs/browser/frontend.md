# Browser utility library

`maestro-web` provides caller-owned localization, numeric display text and SVG
icons. It is a library, not an application entry point.

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

`icons()` loads 27 used glyphs. `icon_svg` constructs their ordered primitive
nodes; [`markup`](../../crates/maestro-text/src/markup.rs) owns XML escaping.
`icon` renders the SVG through usvg/resvg into an egui image, accepting caller
color and device density. `icon_dom` parses the same SVG into a browser SVG
element. Custom and empty primitive lists are also accepted.

Sizes xs/sm/md/lg/xl are 12/16/20/24/32 pixels at a 16px root; md is the default.
DOM callers install `ICON_STYLES` and supply their class suffix/color. Canvas
callers resolve colors from the [brand presentation](../theme.md); this library
has no hover, focus, busy, disabled or selected state, font, mark or animation.

The library is compiled for WebAssembly by `just check`. Controlled native tests
prove utility results and host decision ordering, not browser interaction.
Application launch and browser interaction remain separate delivery work.
