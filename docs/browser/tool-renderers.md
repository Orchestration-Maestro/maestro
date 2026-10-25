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
third type parameter. Its default, `Box<dyn ToolContent>`, keeps the canvas
renderers below working unchanged while that family remains; page renderers
produce `Box<dyn DomContent>` (see [page content](frontend.md#page-content)).
`ToolRendererBinding::from_dom` retains a page renderer for its native operand
types. A page binding never answers the canvas lookup and a canvas binding never
answers the page lookup; the mismatched lookup returns the operand-type error.

In the browser build, `tools::dom::render_tool` routes the same operands through
the same registry and JSON mode to the page renderer, or to the default page
content. That content shows the status header, then `Input` and `Output`
sections holding `code-block` listings. The header shows the tool icon and its
caption. The icon takes the text color with a spinning loader while in progress,
the success color when complete and the error color on failure. Input is
pretty-printed JSON, and output is pretty JSON when it parses, otherwise text,
or `(no output)` when empty. The listings receive their text base64-encoded, so
text that happens to look like base64 still shows literally. Before parameters
arrive the caption reads `Preparing tool...`, and while streaming empty
parameters it reads `Preparing tool parameters...`; the result is never a custom
card. Updating the content replaces the header and changes the existing listings
in place, so a selection and the copy feedback survive. Callback-bearing native
parameters and details reach a page renderer by reference, without
serialization. The page `DefaultRenderer` also renders this content directly
from JSON operands.

Native command, calculation and current-time cards use the same branch preparation
as their drawing counterparts. Select `Box<dyn DomContent>` explicitly when
calling these renderers directly; the default registrations still use drawing
content. Native console bodies keep their root and copy control while the
console branch remains selected.

The browser-only `tools::renderer_registry::header::dom` module provides native
plain and collapsible headers. `ToolHeaderText<Box<dyn DomContent>>` accepts
literal text or a mounted child. `ToolHeaderRefs` keeps live content and chevron
references: rebinding either changes the next activation, and missing either
makes activation a no-op. Initial expansion selects the chevron, not the caller's
body state. Rich descendants use native bubbling rather than a second parent
action. These headers are temporarily hidden from generated API documentation;
the existing root header functions remain the drawing API.

## Drawing a retained row

Keep one `ToolRenderState` per row outside the frame loop. Given the caller's
`ui`, `host`, `i18n`, `presentation`, `highlighter`, retained `row` and containing
`overlays` receiver:

```rust,ignore
use maestro_web::{DefaultRenderer, ToolRenderer, register_tool_renderer, render_tool};
use maestro_web::tools::types::{ToolParams, ToolRendererBinding, ToolRenderView};
use std::rc::Rc;

let renderer: Rc<dyn ToolRenderer<serde_json::Value>> = Rc::new(DefaultRenderer);
register_tool_renderer("inspect", ToolRendererBinding::from_raw(renderer));
let params = serde_json::json!({"path": "src"});
let recipe = render_tool::<serde_json::Value, serde_json::Value>(
    "inspect", Some(ToolParams::json(&params)), None, false,
)?;
let view = ToolRenderView::new(host, i18n, presentation, highlighter, None);
let output = recipe.render(ui, view, row);
overlays.extend(output.overlays);
output.response?;
```

The recipe owns its content. `is_custom` tells the embedding caller whether to
omit its ordinary outer tool card; the renderer does not draw that wrapper.
Changing the recipe's concrete content type starts fresh widget state. Native
placement follows the row lifetime independently of retained state aliases:
leaving a recipe detaches its native surfaces, while aliases can keep the
underlying owners alive. Nested placements detach with their parent.
Row drawing takes call-scoped access to its typed state and restores it before
returning either success or an error.

The default renderer chooses result status before streaming status, formats
truthy input as canonical JSON, and preserves ordered raw text output. Output
JSON is highlighted as JSON; other output remains literal text. Its independent
input and output listings retain their native roots while present. Tool text
uses literal listings, without changing the public code block's encoded-input
behavior.

## Headers and console output

`render_header` draws the shared status icon and literal or rich caption.
`render_collapsible_header` shares the caller's `Rc<Cell<bool>>` expansion state.
Pending headers use the frame clock for their spinner; no timer is allocated.
Plain captions stay muted, including when a retained collapsible header becomes
plain. Collapsible headers use a pointer cursor, foreground hover text and an accent
keyboard-focus outline. Labels without their own foreground inside rich captions
inherit the containing header's hover foreground. Nested headers start their own
foreground scope. Independently styled children, including file pills and code
blocks, retain their colors and independent widget state.

[`ConsoleBlock`](../../crates/maestro-web/src/components/console_block.rs) owns
literal preformatted output, an optional error variant, and a copy control.
Successful copies show feedback for 1500 ms; ordinary copy buttons retain their
2000 ms interval. The console control has a 20-point height, a 16-point icon,
12-point success text and a 4-point icon-to-text gap, with 8-point horizontal
and 2-point vertical padding. Ordinary copy controls keep their 32-point height.
Both use direct native clipboard writing. Message links retain
their separate fallback policy. Actual console updates scroll the local body to
its bottom; unchanged frames and presentation-only changes do not reset scroll.

A console built with [`ConsoleBlock::default`](../../crates/maestro-web/src/components/console_block.rs)
mounts as native page content: its header label and copy control sit outside a
scrolling body holding the literal output, so the header never scrolls away. The
first update of a root that is not on the page yet scrolls on the next frame.
Native page content supplies no canvas host, so only a console constructed
with a host draws on a canvas.

## Opened attachments

User, tool and assistant rows, rich headers and expandable children return
`ToolContentOutput`. Move its `overlays` into the containing receiver before
checking `response`: an attachment opened earlier in a frame must survive a later
drawing error. Keep that receiver outside row lifetimes so removing a row does not
close its previews. Preparation still returns `Result<ToolRenderResult, ToolUiError>`.

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

## Operation errors

Preparation returns `tools::types::ToolUiError`; drawing carries it in
`ToolContentOutput.response`.
Localization, shared glyph controls and native effects retain their named leaf
variants. Reading preparation keeps a typed `ReadingUiError`, including its
highlighter or outer-style cause; JSON and operand preparation preserve their
existing literal messages. `Error::source` exposes each typed cause.

Console output shares `components::code_block::ReadingUiError`; its outer-style
Presentation variant retains the original I/O error. Canvas-host console and header
factories return `AttachmentHostError`; native page mounting returns `ToolUiError`.
Rust callers must update return types and matches when adopting the output record.
The named leaf causes are preserved.

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
other truthy field values use the shared compact JSON display. Recipes own their
selected text. An unchanged native body survives updates; disappearing bodies
release their owners.

## Expandable sections

[`ExpandableSection`](../../crates/maestro-web/src/components/expandable_section.rs)
captures its owned children once. Collapsing detaches their native placement
without losing child state; reopening restores the same roots. The first render
applies `default_expanded`. Reconnection reapplies the latest default without
recapturing generated controls. Changing the setting alone leaves expansion
unchanged. Dropping the section releases its children and native summary.

```rust,ignore
use maestro_web::ExpandableSection;

let mut section = ExpandableSection::new(host.clone(), children);
section.summary = "Details".into();
section.default_expanded = true;
let output = section.render(ui, view);
overlays.extend(output.overlays);
output.response?;
// Reattach the same section with its original children.
section.connected_callback();
let output = section.render(ui, view);
overlays.extend(output.overlays);
output.response?;
```

The literal summary has a leading right/down chevron, an 8-point icon gap and
an 8-point expanded-body gap. The chevron stays centered beside wrapped summaries,
and hovering any summary line highlights it. Native summary clicks and canvas
activation respect the enabled, visible button and focus it; Enter and Space
toggle the focused button. Keyboard focus is outlined and scrolled into view.
Presentation comes from the selected brand pack, with native text and clipped
canvas controls using the same retained host as tool rows.

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
The ordinary outer tool card remains in use.

A filename pill captures its literal filename and panel during preparation.
With a panel, its action consumes the click and opens an existing artifact;
unknown filenames do not change selection. Without a panel, clicking the pill
activates its containing disclosure.
The standalone `artifact_pill` helper uses the same retained native filename
owner. Formatted non-string fields are display-only, never artifact identities.

`components::diff::diff` accepts literal old/new text, an optional title and
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
