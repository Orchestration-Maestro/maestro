# Browser transcript

## Retained message rows

`UserMessage`, `AssistantMessage`, `ToolMessage`, `ToolMessageDebugView`,
`AbortedMessage` and `ThinkingBlock` are available at the web crate root and
under `components`. Construct host-backed rows with `new(host)`; tool rows need
no host until rendering. Each render borrows `ToolRenderView` for the current
localization, presentation, syntax engine, native host and modal identity.
Keep the row between frames, not its borrowed view or props.

`UserMessageInput` borrows ordinary user content or content with the original
attachment handles. Only the first text block is displayed. A render returns
independently owned attachment overlays: keep those overlays separately if they
must outlive the row. The user bubble's gradient and border share its fixed
frame; that frame applies the rounded backdrop blur outside the text scroller.
The bubble fits its text and attachments within the parent width, including its
native-resolved border and padding.

`AssistantMessageProps` supplies tool names, pending IDs, result maps, visibility
flags, streaming state and an optional cost action. `ToolMessageProps` borrows
the original call and result. Both preserve caller-defined native result details
without serialization bounds. Assistant children retain state by visible position
and component kind. Thinking disclosures start collapsed and keep expansion
through content and streaming updates. Replacing the cost action replaces the
live callback; removing usage or releasing the row revokes it. Chevron rotation,
hover color and action text use a 150-ms cubic-bezier(0.4, 0, 0.2, 1) transition.

Debug fields distinguish absent arguments/details, explicit JSON null and
result presence. Call and Result headings use medium weight; absent-result text
uses normal-weight small text. Strict JSON strings are parsed for display; other
strings stay literal. All three debug listings use literal code input, so `YWJj` stays
`YWJj` when displayed or copied. The standalone encoded-input `CodeBlock`
constructor keeps its existing behavior.

All text uses the existing native reading host, with its fixed placement frame,
clipped scrolling viewport, fractional canvas projection and modal admission.
Message rows do not add a separate placement or event-listener system.

## Custom message roles

`register_message_renderer`, `get_message_renderer` and `render_message` use
open role strings. Registering the same role replaces the current callback for
that native message representation. A previously retained callback remains
usable. Unknown roles return no content. Callbacks prepare owned `ToolContent`
recipes; draw them with the ordinary `ToolRenderResult` and `ToolRenderState`.
The registry releases its guards before invoking or dropping user callbacks.

A custom message can carry a native handle without JSON conversion:

```rust
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use maestro_app::{AgentMessage, CustomAgentMessages, ToolRendererState};
use maestro_web::{MessageRenderer, register_message_renderer};
use maestro_web::tools::types::{ToolContent, ToolContentOutput, ToolRenderView, ToolUiError};

struct NativeMessage {
    role: String,
    counter: Arc<AtomicU32>,
}
impl CustomAgentMessages for NativeMessage {
    fn role(&self) -> &str { &self.role }
}
struct Label(String);
impl ToolContent for Label {
    fn render(
        &self, ui: &mut egui::Ui, _: ToolRenderView<'_>, _: &ToolRendererState,
    ) -> ToolContentOutput {
        ToolContentOutput { response: Ok(ui.label(&self.0)), overlays: Vec::new() }
    }
}
struct CounterRenderer;
impl MessageRenderer<NativeMessage> for CounterRenderer {
    fn render(
        &self, message: &AgentMessage<NativeMessage>,
    ) -> Result<Box<dyn ToolContent>, ToolUiError> {
        let AgentMessage::Custom(value) = message else {
            return Err(ToolUiError::Presentation("Expected a native counter".into()));
        };
        let value = value.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(Box::new(Label(value.counter.load(Ordering::Relaxed).to_string())))
    }
}
register_message_renderer("counter", std::rc::Rc::new(CounterRenderer));
```

The input remains the caller's `AgentMessage<NativeMessage>`. Changing its
counter changes the next prepared recipe; no registry or message guard spans
recipe drawing.

## Native copy controls

`maestro_web::components::copy_button::CopyButton` retains a supplied
`AttachmentHost`. Construct it with the current localization owner, set `text`,
`title` and `show_text`, then call `render` with the frame's UI and presentation.
The default text is empty, the title is localized `Copy` and success text is off.
An explicitly empty title stays empty.

Activation starts a direct native clipboard write immediately, capturing the current
original text. It does not try a temporary-selection fallback. Pending and rejected operations do not show a new success indication.
Success switches the copy glyph to a check and optionally shows localized
`Copied!` for 2000 ms after completion. A later success replaces that control's
restoring timer. A rejected timer registration clears feedback rather than
leaving a permanent success indication.

The control shares weak clipboard completion and timer ownership with message
links, which retain their separate 1500 ms policy. Dropping the control revokes
its timer and prevents pending work from delivering new UI feedback; it does not
claim to cancel native copying already started. Pointer activation requires both
primary endpoints in the enabled, visible slot on the admitted toolkit layer.
Keyboard activation remains available through the returned response.

## Native Markdown and code

`maestro_web::components::markdown_block::MarkdownBlock` and
`code_block::CodeBlock` retain an `AttachmentHost`; construct each with `new` and
set its public properties. Markdown starts empty, not thinking and with escaping
on; code starts with empty code and language. Both render through
`CodeBlockView`, which borrows one frame's localization, brand presentation,
syntax engine and containing-modal identity and is never retained.
Portable Markdown controls retain code listings; the browser owns the document
HTML and generated listing-slot attribute.

Markdown parsing and scoped inline interpretation use the shared
[Markdown owner](../markdown.md), reached through `maestro_app::markdown`.
The browser grammar receives the component's escaping policy and parses authored
source once per document. Display formulas enter the
block phase through the component's tokenizer, before reference resolution.
One document-scoped context retains resolved operands and original label scopes;
inline formulas are selected at each reached cursor before ordinary inline grammar.
Formula contents remain visible to emphasis matching, and each recursive span admits
formulas only within its selected child window. A consumed link opener cannot be
replayed as an anchor. Retained and newly parsed labels unescape brackets before
formula admission, keeping their original source ranges, destinations and titles.
Link labels use inline-only grammar, keeping list, heading and quote punctuation literal.
Tag-like text outside code and angle autolinks is
shown literally when escaping is on, including a tag start that never closes.
Each code span, code block and autolink keeps its text while it remains native after
escaping. A span that loses that status has its tags shown literally; later spans
that remain native keep their text. Formulas written with `$…$`, `\(…\)`,
`$$…$$` and `\[…\]` render through the math library as HTML. Parse and builtin
macro errors appear inline; malformed environment names instead show escaped
source with its original delimiters. Bare radicals keep their radicand padding,
and explicit size changes preserve inline or display clearance. Repeating the
current explicit size keeps script spacing. With escaping off, an
HTML tag inside an admitted formula remains formula input, not a document element.
Display delimiters open a block in paragraphs, quotes and loose items, and a tight
item only when it starts with one, while headings and table cells
admit inline formulas. Code, images, plain URLs and angle autolinks remain opaque.
Selected links retain literal destinations and titles while their labels may contain
formulas. A formula that starts before authored link syntax consumes that syntax
rather than creating an anchor. The renderer refuses Markdown link targets that
would run script: `javascript:` and `vbscript:` URLs, and `data:` URLs holding
HTML, XML, SVG or script. Markdown images keep every `data:` target, regardless of
its declared media type; they refuse only `javascript:` and `vbscript:` URLs.
A refused link shows its label without an anchor, and a refused image shows its
alternative text. With escaping on, a refused angle autolink keeps its source as
literal text. The spreadsheet preview uses the same link rule.
The selected pack's error role colors formula errors when painted, and the
matching math stylesheet and fonts are inserted once per document.

Each code block is a native listing: a language label and a canvas copy control in
a header outside the body, whose own scroller is at most 24 rem tall. The first
information word is the label, `text` when absent; a standalone block displays
`plaintext`. Standalone code is decoded from base64 when it is valid UTF-8 and kept
whole otherwise. Highlighting and automatic language selection come from the
theme's HTML code profile; the syntax classes are styled through
`maestro_theme::highlight_css`.

Unchanged text keeps its native root, selection and scroll. Changed Markdown
replaces its root and copy controls only after the replacement is published, so a
failed publication keeps the old document and mounted owner; reasoning presentation changes without
replacing content. A standalone block updates its label and body in place and keeps
its copy control and its active success indication. Updates are staged for measurement;
only successful publication reconciles changed native nodes. The first returned
control geometry uses the retained viewport's prospective layout and clamped
fractional scroll on both axes, including presentation-only changes. A replacement syntax
engine is consumed even when Markdown content is unchanged. A control that gains keyboard
focus is scrolled into view. Dropping the owner removes the native root and revokes
pending feedback.

Tool-renderer listings use the same listing pipeline with literal input; see
[tool renderers](tool-renderers.md). Their text is not auto-decoded.
