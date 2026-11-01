# Browser tool renderers

[`render_tool`](../../crates/maestro-web/src/tools/mod.rs) selects a named
renderer, or the default input/output renderer when no registration exists.
`set_show_json_mode(true)` forces that default without changing registrations.
Disabling it resumes the registered renderers.

## Registration and native data

`register_tool_renderer` replaces an existing name without changing its position.
`get_tool_renderer::<P, D>` returns a retained handle for those operand types.
The exposed `TOOL_RENDERERS` map supports ordered enumeration and removal.
Dispatch releases its registry borrow before calling custom code.

[`ToolRendererBinding`](../../crates/maestro-web/src/tools/types/binding.rs)
keeps the native renderer. `from_raw` requires no serialization or decoding
traits. `ToolRenderer` defaults to borrowed model `JsonObject` arguments and
JSON-value details. Explicit type parameters keep native operands and non-object
JSON values available. `from_typed` additionally admits both model objects and
ordinary JSON values through the selected types' decoders. A decoder failure
prevents the callback; an incompatible native operand type returns a rendering
error.

`ToolParams::object` borrows the model's original map without wrapping or copying
it. Its default projection uses the shared object writer. `ToolParams::json` uses
canonical JSON formatting for an explicit JSON-value operand. `ToolParams::typed` borrows a
native value with a formatter that runs only when default rendering is selected.
The borrowed result view delegates the original message envelope and content;
custom renderers can retain their own shared data in the returned recipe.

## Page content

`ToolRenderer<P, D, C>` and `ToolRenderResult<C>` take the produced content as a
third type parameter. In the browser build its default is `Box<dyn DomContent>`
(see [page content](frontend.md#page-content)); native builds prepare the same
owned records for type checking and tests, with nothing to paint.
`ToolRendererBinding::from_dom` retains a renderer that names that content type
explicitly. `tools::dom::render_tool` is the same function as `render_tool`.

The default content shows the status header, then `Input` and `Output` sections
holding code listings. The header shows the tool icon and its caption. The icon
takes the text color with a spinning loader while in progress, the success color
when complete and the error color on failure. Input is pretty-printed JSON, and
output is pretty JSON when it parses, otherwise text, or `(no output)` when
empty. The listings show their text literally, so text that happens to look like
base64 is neither decoded nor copied decoded. Before parameters arrive the caption
reads `Preparing tool...`, and while streaming empty parameters it reads
`Preparing tool parameters...`; the result is never a custom card. The section
labels and the empty-output text follow the view's localization; captions stay as
authored. Updating the content replaces the header and changes the existing
listings in place, so a selection and the copy feedback survive; a listing that
disappears releases its root. Callback-bearing native parameters and details
reach a renderer by reference, without serialization. `DefaultRenderer` also
renders this content directly from JSON operands.

The browser-only header module provides plain and collapsible headers, exported
at the crate root as `render_header` and `render_collapsible_header`.
`ToolHeaderText` accepts literal text or a mounted child. `ToolHeaderRefs` keeps
live content and chevron references: rebinding either changes the next
activation, and missing either makes activation a no-op. Initial expansion
selects the chevron, not the caller's body state. Rich descendants use native
bubbling rather than a second parent action.

## Mounting a row

Keep one `MountedContent` per row. Given the caller's `view` and `parent` element:

```rust,ignore
use maestro_web::{DefaultRenderer, ToolRenderer, register_tool_renderer, render_tool};
use maestro_web::dom::MountedContent;
use maestro_web::tools::types::{ToolParams, ToolRendererBinding};
use std::rc::Rc;

let renderer: Rc<dyn ToolRenderer<serde_json::Value>> = Rc::new(DefaultRenderer);
register_tool_renderer("inspect", ToolRendererBinding::from_raw(renderer));
let params = serde_json::json!({"path": "src"});
let prepared = render_tool::<serde_json::Value, serde_json::Value>(
    "inspect", Some(ToolParams::json(&params)), None, false,
)?;
let mut row = MountedContent::default();
row.show(prepared.content.as_ref(), &parent, &view)?;
```

The prepared content owns its operands. `is_custom` tells the embedding caller
whether to omit its ordinary outer tool card; the renderer does not draw that
wrapper. Showing content of the same type updates the root in place; a different
content type replaces it, detaching the old root even if the caller still holds
the node. Nested placements detach with their parent.

The default renderer chooses result status before streaming status, formats
truthy input as canonical JSON, and preserves ordered raw text output. Output
JSON is highlighted as JSON; other output remains literal text.

## Console output

[`ConsoleBlock`](../../crates/maestro-web/src/components/console_block.rs) owns
literal preformatted output, an optional error variant, and a copy control.
Successful copies show feedback for 1500 ms; ordinary copy buttons retain their
2000 ms interval. Both use direct native clipboard writing. Actual console
updates scroll the local body to its bottom; unchanged content and
presentation-only changes do not reset scroll.

A console built with [`ConsoleBlock::default`](../../crates/maestro-web/src/components/console_block.rs)
mounts as native page content: its header label and copy control sit outside a
scrolling body holding the literal output, so the header never scrolls away. The
first update of a root that is not on the page yet scrolls on the next frame.
Tool cards use this page console. Only a console constructed with a host draws on
a canvas, for the artifact bodies that still use one.

## Opened attachments

Returned-file tiles and user attachments open a native
[attachment overlay](attachments.md#native-page-overlay) in the view's document.
The overlay belongs to the page, not to the row: collapsing, updating or removing
the row leaves an opened preview in place, and a later rendering error does not
close it.

## Document tool cards

The `javascript_repl` and `extract_document` renderers register once. Replacement,
removal and JSON mode follow the same policy as the command card. Both accept
model objects, explicit JSON objects and their application-owned native records.

JavaScript cards keep literal code and console output in a collapsed body;
returned files remain accessible outside it. Document cards show the URL and
extraction metadata, literal extracted text on success and a console on failure.
Their bodies retain native children while collapsed. The JavaScript card's
preparing and executing fallbacks are localized. Authored titles and all
document-extraction captions, metadata and the URL label remain literal.

Selected non-string fields use readable JSON. Code and extracted text use literal
code blocks, without encoded-input detection. Returned text files decode UTF-8
for preview while downloads preserve the original base64; malformed base64 keeps
the download without extracted text.

The card puts its header first, then a body that starts collapsed and animates its
height over 300 ms when the header toggles it, then returned files. Updates within
the same pending or result branch keep the body, its expansion, the listing, the
console with its copy feedback and unchanged file tiles. Moving from pending to a
result starts a new collapsed body, as a new branch does.

## Operation errors

Preparation and mounting return `tools::types::ToolUiError`.
Localization, shared glyph controls and native effects retain their named leaf
variants. Reading preparation keeps a typed `ReadingUiError`, including its
highlighter or outer-style cause; JSON and operand preparation preserve their
existing literal messages. `Error::source` exposes each typed cause.

Canvas console output shares `components::code_block::ReadingUiError`; its
outer-style Presentation variant retains the original I/O error.

## Command, calculation and time cards

`BashRenderer` is registered as `bash` once per browser thread. Replacing or
removing it stays effective; JSON mode bypasses it without altering the registry.
`CalculateRenderer` and `GetCurrentTimeRenderer` are SDK exports for explicit
registration. They display results, not executable calculation or clock tools.

The three renderers accept borrowed model objects and ignore result details,
without requiring those details to be serializable or cloneable. The built-in
bash binding also admits JSON-value arguments: falsy values mean absent
parameters; truthy non-objects mean present parameters without a command field.
Explicit registrations retain their original operand checks.

Bash shows the literal command and ordered text output in a console, without an
expansion toggle. Calculation and time put successful output in their captions
and show errors as plain destructive text below. All three use result status
when present and pending status otherwise, regardless of the streaming flag.
Only fixed caption keys are translated. Selected strings remain unchanged;
other truthy field values use the shared compact JSON display. Prepared cards own their
selected text. An unchanged native body survives updates; disappearing bodies
release their owners.

## Expandable sections

[`ExpandableSection`](../../crates/maestro-web/src/components/expandable_section.rs)
holds a literal summary and the children it was built with. Mounting places each
child once; collapsing detaches the details without losing child state, and
expanding reinserts the same roots. Mounting applies `default_expanded`.
`connected_callback` reapplies the latest default to a mounted root without
recapturing anything. Changing the setting alone leaves the current expansion
unchanged, and an update changes the summary while keeping expansion and children.
Dropping the section releases its children.

```rust,ignore
use maestro_web::ExpandableSection;
use maestro_web::dom::DomContent as _;

let mut section = ExpandableSection::new(children);
section.summary = "Details".into();
section.default_expanded = true;
let root = section.mount(&view)?;
parent.append_child(&root.node)?;
// Reattached: apply the current default to the same children.
section.connected_callback(&root)?;
```

The summary is a native button with a leading right or down chevron; pointer,
Enter and Space toggle it. Presentation comes from the selected brand pack.

## Artifact cards

`ArtifactsToolRenderer::new(panel)` captures an optional application artifact
panel when preparing a card. It is not registered automatically. Register a
`ToolRendererBinding::from_raw` handle under `artifacts`, adding the caller's
other parameter/detail pairs with `with_raw`. The renderer accepts model objects,
explicit JSON values and native `ArtifactsParams`; it does not read result
details. Partial JSON arguments are rendered without execution-schema decoding.

```rust,ignore
use maestro_app::{JsonObject, browser::tools::artifacts::ArtifactsParams};
use maestro_web::{ArtifactsToolRenderer, register_tool_renderer,
    tools::types::ToolRendererBinding};
use serde_json::Value;
use std::rc::Rc;

let renderer = Rc::new(ArtifactsToolRenderer::new(panel));
let binding = ToolRendererBinding::from_raw::<JsonObject, Value>(renderer.clone())
    .with_raw::<Value, Value>(renderer.clone())
    .with_raw::<ArtifactsParams, Value>(renderer);
register_tool_renderer("artifacts", binding);
```

Register each native details type the caller supplies in place of `Value`.
`ToolMessage` and `AssistantMessage` use this registration through the ordinary
shared output path. Replacing or removing the binding takes effect on the next
render; JSON mode bypasses it without changing the registration.

Create and rewrite cards show literal highlighted content. Updates show a line
diff. Successful get and logs cards show file content and console output,
respectively, with a localized empty-output fallback. Edit errors retain their
code or diff; lowercase `.html` filenames select an error console, while other
filenames show plain error text. Non-edit errors have no disclosure or file pill.
The ordinary outer tool card remains in use. Its filename pills use native click
propagation; its differences reuse the shared normalized rows and retained
browser nodes.

A filename pill captures its literal filename and panel during preparation, so
changing the renderer's panel or the source arguments afterwards changes no
prepared card.
With a panel, its action consumes the click and opens an existing artifact;
unknown filenames do not change selection. Without a panel, clicking the pill
activates its containing disclosure.
The standalone `artifact_pill` helper prepares the same pill. Formatted non-string fields are display-only, never artifact identities.

`diff` (also `components::diff::diff`) accepts literal old/new text, an optional title and
caller-supplied native CSS classes through `DiffProps`. The retained cache
compares the normalized title, literal classes and selected lines on every
target. An empty title is absent. LF splitting happens after differencing and
omits each run's final empty split item; blank lines, carriage returns and run
order stay significant. The browser derives counts and mounts rows from that
same input. The optional header shows added/removed line counts in the selected
success/error colors. Its selectable
rows preserve line order and carriage returns, with a local scrolling body capped
at 384 CSS pixels. Text updates retain the native root, local scroll and
unchanged rows, including when rows grow or shrink or the title appears or
disappears. Failed native insertion leaves the published diff unchanged.
