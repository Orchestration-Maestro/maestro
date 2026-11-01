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

`icons()` loads 35 used glyphs. `icon_svg` serializes ordered primitive nodes
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

### Sample-session replay data

The [session fixture](../../crates/maestro-web/tests/fixtures/test_sessions.json)
keeps two complete offline conversations. The
[replay tests](../../crates/maestro-web/tests/sample_sessions_browser.rs),
`simple_html_session_replays_existing_browser_owners` and
`long_session_replays_existing_browser_owners`, mount their user and assistant
rows as page content, expand every tool card, inspect the card bodies and compare
complete reconstructed artifact files.
Missing timestamps become zero and historical tool outputs become text blocks
only in the tests' owned copies. Recorded browser commands and model requests
are not executed. HTML is observed through source/diff cards and reconstructed
file data, not a specialized preview.

Both tests run in the existing loading family with the `just browser-test`
command below.

## Input, timers and dialogs

[Browser input, timers and dialogs](components.md) mount native fields and dialogs as page
content, including the model picker, the provider and key dialogs, the settings dialog, the saved
session list and the storage permission request. The browser `Input` is distinct from the terminal
toolkit input.

## Sandbox messages

[Sandbox runtime messaging](sandbox.md) provides registered-window admission,
live runtime-provider dispatch and source generation for iframe and extension
channels. Native tests and WebAssembly compilation are distinct from browser
effect qualification.

See [artifact views](artifacts.md) for independently embeddable generic reading
bodies, progressive PDF pages and original-byte header downloads.

## Message links

The [offline viewer's retained copy-link control](session-viewer.md#message-links)
uses the shared browser effects host for native clipboard success, with owned
feedback and caller-measured message slots. It does not render message content.

## Page content

[`DomContent`](../../crates/maestro-web/src/dom.rs) is page content: `mount`
creates its own `DomRoot` and `update` brings that root up to date. A `DomRoot`
holds the root node and, optionally, an owner for native resources that live
exactly as long as the root, such as an isolated frame. `DomView` carries the
document, localization, syntax engine, brand pack and mode, plus view-local tool
renderer bindings that take precedence over page registrations.
`MountedContent` keeps one placed root. Content of the same type and `key`
updates the root in place. Content of another type, or of another key (module
elements use their tag), mounts a new root in place of the old one and releases
the old root's owner. A failed mount returns its error and keeps the
previous root. `show_before` places a first root before a given sibling instead
of last, and `clear` removes the root and releases its owner.

`ContributionHost::new` and `ChatPanel::new` share one per-page registration of
the built-in `copy-button`, `code-block`, `markdown-block`, `thinking-block`,
`maestro-slot` and native artifact elements. They install the same stylesheet:
the selected pack's properties on the document
root, then the rules for listings, Markdown, reasoning disclosures, buttons, tool
headers and contribution failures. A definition is permanent: its
constructor callback lives as long as the page, while views connect and
disconnect. A changed definition needs a fresh element name or a page reload.

`code-block` shows its `code` attribute under its `language` label (`plaintext`
when empty). It decodes the attribute as base64 UTF-8 text and shows anything
else literally. It uses the same listing as the canvas code block, with a scroller bounded
at 24rem, and a copy control. Later changes of `code` or `language` change only
the parts that differ, so a selection elsewhere in the listing survives.
`copy-button` copies its `text` attribute, shows success (with `Copied!` when
`show-text` is present) and restores after two seconds; changing `title` or
`show-text` renders it again. The `code`, `language`, `text` and `showText`
properties set the matching attributes.

`markdown-block` shows its `content` attribute as Markdown. `is-thinking` selects
the reasoning presentation and `unescaped-html` lets authored HTML through. The
`content`, `isThinking` and `escapeHtml` properties set them; `escapeHtml` is true
by default, and setting it to false adds `unescaped-html`. `thinking-block` shows
a collapsed `Thinking...` disclosure of its `content` attribute, and
`is-streaming` (the `isStreaming` property) makes the header shimmer. A detached
`thinking-block` drops its toggle listener; when it is attached again it binds
the toggle once and keeps its expansion.

`components::button::button` creates a native button whose `type` comes from
`ButtonType` (`button` by default), with `maestro-button
maestro-button-<variant> maestro-button-<size>` classes followed by the
caller's `class_name`, an optional title and a spinning loader while loading.
Disabled and loading buttons refuse activation. Each variant paints from the
pack: `default` on the accent color, `destructive` on the error color,
`outline` with a border, `secondary` on the panel color inside a border-colored
ring, `ghost` without a background and `link` in the accent color. Only `outline`
carries a border width, so every other variant measures its padding and glyph alone.

## Contributions

A package lists browser contribution files under `maestro.contributions` in its
`package.json`, or places them in a `contributions/` directory. Package
resolution treats them as the fifth resource kind beside extensions, skills,
prompts and themes, with the same ordering, deduplication and `contributions`
settings filter. `maestro_app::contributions::load_contributions` reads each
enabled file as one descriptor. An unreadable or invalid file yields one warning
naming its path, and the other files still load. `DefaultResourceLoader` reads
them on each reload, beside the other four kinds. Explicit package sources come
first; the first source for each canonical contribution file supplies its package
identity. `ResourceLoader::get_contributions` returns the accepted records and
warnings.
The records live in `maestro-protocol::contributions`. Delivering them to
browser pages as plain JSON belongs to the shell's protocol operation.

| Field | Meaning |
|---|---|
| `id`, `kind`, `target` | Required strings: the identifier within its package, an extensible kind and the tool name, message role, slot name or API name the kind selects |
| `route` | Optional route of a page |
| `input` | Optional data contract the contribution consumes |
| `slots` | Child slots it supplies, each `{name, input?}` |
| `requires` | Host API names it needs |
| `module` | Optional `{entry, wasm, register}` of a separately built browser module |
| `document` | Optional package-relative document shown in an isolated frame |

The `tool-renderer`, `message-renderer` and `api` kinds register with the tool
registry, the message registry and the host API table. Every other kind attaches
to the slot its `target` names. `register_contribution` registers built-in
content; the same package and identifier replace an entry in place. Each
registration owns its mount identity: replacing an entry replaces its mounted
content even when its input is unchanged. `ContributionRegistration::unregister`
removes it only while that registration still owns it. `slot_contributions`,
`route_contribution`, `slot_input` and `admit` read the registry. A route claimed by two contributions, or a child
slot two contributions declare with different contracts, makes
`route_contribution` or `slot_input` return a conflict.

A contribution that declares `input` admits only where its slot supplies that
contract (the supplier's `slots` entry); one without `input` admits anywhere.
A slot mounts every attached contribution through one caller: `SlotMount`, or a
connected `<maestro-slot name="…">` element whose `maestroInput` property holds
its input. Each contribution gets its own wrapper and keeps its root across
updates. Registration changes refresh every connected slot once. An incompatible
input, a missing required API, a conflicting slot contract or content that fails
to mount shows its sentence in a `role="alert"` element in place of that
contribution, and the rest of the slot still mounts. `SlotMount::show` returns
the sentences it showed, and clearing removes every wrapper:

- `<id>: missing host API <api>`
- `<id>: expects <input>, slot supplies <input or "no input">`
- `<id>: not declared by its package`
- `<entry>: <cause>`, for a module or document that failed to load
- `<id>: <cause>`, for content that failed to mount
- `<name>: declared by <first> and <second>`, for a conflicting route or slot

## Runtime modules

A browser module is built separately from the host with
`wasm-bindgen --target web`. Its `entry` is the generated module, `wasm` its
matching binary and `register` an exported function. `ContributionHost::load`
groups contributions by module, imports each entry once, initializes it with
`{module_or_path: wasm}` and calls `register` with an interface object:

- `register(id, tag)` binds a declared contribution to an element the module
  defined; tool and message renderers receive `maestroInput`
  (`{params, result, isStreaming}` or `{message}`) and a `maestro-input` event on
  each update. Registering the same identifier with a new tag replaces the
  mounted element. A tool or message renderer whose required API is missing
  shows that failure instead of its element.
- `provide(id, value)` provides a declared API; a second provider is refused.
- `api(name)` returns a provided API the module declared as required.

Each method throws `<method>: <argument> is not a string` when an identifier,
tag or API name is not a string.

The host imports modules through a function the embedding page supplies. The
page bootstrap passes `url => import(url)` as the `import_module` argument of
`ContributionHost::new`:

```js
await start(url => import(url)); // the page's Rust entry calls ContributionHost::new
```

The embedding page is trusted: modules run in the page with its authority. The
host calls `import_module` only for entry and binary URLs below `base_url`, which
names the package directory and must end with `/`; it refuses any other URL
before importing anything. `load` returns `ModuleRegistrations`. Dropping it
releases each module's interface first, so callbacks that run during the
release cannot register again. It then releases, newest first, exactly the
registrations each module invocation still owns, so a later explicit
replacement keeps serving. A module that fails to initialize
releases what it acquired and reports one failure; the executed code stays
loaded, so only a fresh element name or a page reload replaces its definitions.

Untrusted or generated UI uses `document` instead. The host fetches it from
below the package base and shows it in an isolated frame (`sandbox="allow-scripts
allow-modals"`): an opaque origin without cookies, storage or access to the host
page. Its input is the frame global `maestroInput`, and the runtime message
router admits only messages from the frame registered under the contribution
identifier. A changed input loads the document again with the new
`maestroInput`. Each load registers the frame anew, so a message consumer is
added after each load. The mounted root owns the frame and removes it when the root is replaced or
cleared. When a descriptor names both, the module wins and the host reports the
document as ignored.

Pages may run under a content security policy without `unsafe-eval`. Modules
need `script-src 'self' 'wasm-unsafe-eval'`, and built-in elements set inline
style attributes. Isolated frames under such a policy need the sandbox document
URL (the `sandbox_url` argument), because a `srcdoc` frame inherits the page's
policy and its inline scripts would be refused. The browser tests run the
contribution family under
`default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'`.

Run them with the pinned driver, which the shared Chromium job also uses:

```sh
PLAYWRIGHT_MODULE=/absolute/path/to/playwright CHROMIUM_PATH=/absolute/path/to/chrome \
  CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just browser-test
```

See [browser integration qualification](../development.md#browser-integration-qualification)
for the tool versions and the run's log lines.
