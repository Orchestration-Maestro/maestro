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

## Drawing a retained row

Keep one `ToolRenderState` per row outside the frame loop. Given the caller's
`ui`, `host`, `i18n`, `presentation`, `highlighter` and retained `row`:

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
recipe.render(ui, ToolRenderView {
    host, i18n, presentation, highlighter, modal: None,
}, row)?;
```

The recipe owns its content. `is_custom` tells the embedding caller whether to
omit its ordinary outer tool card; the renderer does not draw that wrapper.
Changing the recipe's concrete content type starts fresh widget state. Native
placement follows the row lifetime independently of retained state aliases:
leaving a recipe detaches its native surfaces, while aliases can keep the
underlying owners alive. Nested placements detach with their parent.

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
keyboard-focus outline. Rich captions retain their own colors and executable
content with independent widget state.

[`ConsoleBlock`](../../crates/maestro-web/src/components/console_block.rs) owns
literal preformatted output, an optional error variant, and a copy control.
Successful copies show feedback for 1500 ms; ordinary copy buttons retain their
2000 ms interval. The console control has a 20-point height, a 16-point icon,
12-point success text and a 4-point icon-to-text gap, with 8-point horizontal
and 2-point vertical padding. Ordinary copy controls keep their 32-point height.
Both use direct native clipboard writing. Message links retain
their separate fallback policy. Actual console updates scroll the local body to
its bottom; unchanged frames and presentation-only changes do not reset scroll.

## Operation errors

Tool dispatch, recipes and headers return `tools::types::ToolUiError`.
Localization, shared glyph controls and native effects retain their named leaf
variants. Reading preparation keeps a typed `ReadingUiError`, including its
highlighter or outer-style cause; JSON and operand preparation preserve their
existing literal messages. `Error::source` exposes each typed cause.

Console output shares `components::code_block::ReadingUiError`; its outer-style
Presentation variant retains the original I/O error. Native console and header
factories return `AttachmentHostError`. Rust callers must update return types
and matches; dispatch, rendering, resource ownership and recovery are unchanged.

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
section.render(ui, view)?;
// Reattach the same section with its original children.
section.connected_callback();
section.render(ui, view)?;
```

The literal summary has a leading right/down chevron, an 8-point icon gap and
an 8-point expanded-body gap. Visible pointer activation, Enter and Space toggle
it. Keyboard focus is outlined and scrolled into view. Presentation comes from
the selected brand pack, with native text and clipped canvas controls using the
same retained host as tool rows.
