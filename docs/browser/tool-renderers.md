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
traits. `from_typed` additionally admits ordinary JSON parameters and details
through the selected types' decoders. A decoder failure prevents the callback;
an incompatible native operand type returns a rendering error.

`ToolParams::json` uses canonical JSON formatting. `ToolParams::typed` borrows a
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

let renderer: Rc<dyn ToolRenderer> = Rc::new(DefaultRenderer);
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
