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

## Stable transcript list

`MessageList::new(host)` composes the retained message widgets. Pass ordered
browser messages in `MessageListProps` to `render`; empty props produce an empty
list. Tool results are indexed by call ID before rows are prepared, with the last
matching result winning. Standalone results and unknown roles have no builtin
row. Repeated messages remain repeated and in order.

Artifacts are filtered before custom-role dispatch. Other registered roles take
precedence over builtin rows, including user and tool-result roles. Custom
recipes keep native callback handles without a serialization bound; an empty
recipe still occupies its emitted position. Appending retains existing row owners;
a component-kind replacement retires only that position, and truncating releases
only the removed rows. Refreshing the same history still publishes completed rows.

## Streaming transcript row

`StreamingMessageContainer::new(host)` starts empty. Call `set_message(message,
immediate)` to update it, then render with `StreamingMessageProps`. Deferred
updates coalesce on the next native animation frame. That publication snapshots
nested assistant content; later edits to the source cannot change it. Immediate
updates keep the supplied shared handle, so subsequent edits appear on rendering.
Clearing is synchronous and revokes only this instance's pending frame. A new
response can publish on its first frame after a clear.

Only assistant messages render in the streaming row. Empty streaming state shows
a cursor; empty idle state and other roles show nothing. The assistant receives
the supplied tools, pending IDs, completed result map and cost action. Its child
widgets retain disclosure state through updates. Clearing releases those children,
so the next response starts collapsed. Keep returned overlays separately: a later
child failure does not consume an already-opened overlay.

The native cursor is 8 by 16 pixels, inset 16 pixels, with a two-second pulse in
the selected pack's muted color. Editor input, send/abort actions and conversation
statistics belong to the separate editor interface.

## Session transcript view

On browser targets, `AgentInterface::new(host)` binds the transcript to an external
Agent with `set_session`. `session()` returns that retained owner. Replacement and
release explicitly remove the old subscription. Session callbacks observe reduced
state, retain no state guard and hold weak references back to the view and session.
Tool execution events do not directly refresh the transcript.

Binding fills only missing stream and credential callbacks. The default stream
reads the current proxy-enabled setting per request, then reads the current URL
only when enabled. Credential lookup reads the current application key store.
Caller-supplied callbacks stay unchanged, and each reached storage read resolves
the current application store separately.

The containing native viewport keeps every row's text attached while clean scroll
frames draw only intersecting canvas rows. A history refresh prepares the result
index once for both stable and streaming rows. Native resize notifications identify
the affected positional slot; offscreen image growth updates its height before
later rows are revealed. Rows use their planned canvas origin while native slot
sizes are published together, so a preceding row's growth does not offset a child
twice.

`set_auto_scroll` overrides whether later content growth follows the bottom. A
nonzero upward scroll disables following only beyond 50 pixels from the bottom;
a distance below 10 pixels enables it again. Equality at either boundary does not
cross it. A smaller viewport height updates the remembered height without changing
the remembered top or follow preference. All observations retain fractional values.
A later publication preserves a reader scroll made after programmatic following.

## Contained native reading

`AttachmentHost::enter_reading_container` positions nested native frames inside a
retained reading root or one of its slots. Keep the returned placement scope alive
while drawing its children. Its optional canvas origin places a child against the
planned row position before new native slot heights are published. Without that
operand, placement uses the slot's current native bounds. The containing viewport
supplies scrolling and clipping;
contained reading bodies do not add independent scrollers. Standalone reading views
keep their existing fixed frames and bounded viewports.

On browser hosts, `resize_reading_slots` grows or truncates positional slots without
replacing the containing root or untouched slots. Its factory receives the actual
canvas document and returns owned nodes. Factories run without a registry borrow,
and all additions are prepared before publication. A factory or native append
failure leaves existing nodes, selection and scroll intact.

`reading_scroll` reads the actual viewport's fractional top offset, full content
height and client height in CSS pixels. `set_reading_scroll` assigns its vertical
offset without moving the horizontal axis; normal browser clamping still applies.
Unsupported hosts return a host error for these effects and for `animation_frame`,
rather than successful empty observations.

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

Markdown parsing and scoped inline interpretation use the shared
[Markdown owner](../markdown.md), reached through `maestro_app::markdown`.
The browser grammar receives the component's escaping policy and parses authored
source once per document. Display formulas enter the
block phase through the component's tokenizer, before reference resolution.
One document-scoped context retains resolved operands and original label scopes;
formula atoms precede ordinary inline grammar without resolving rewritten labels.
Link labels use inline-only grammar, keeping list, heading and quote punctuation literal.
Tag-like text outside code and angle autolinks is
shown literally when escaping is on, including a tag start that never closes.
Each code span, code block and autolink keeps its text while it remains native after
escaping. A span that loses that status has its tags shown literally; later spans
that remain native keep their text. Formulas written with `$…$`, `\(…\)`,
`$$…$$` and `\[…\]` render through the math library as HTML with errors shown
inline; display delimiters open a block in paragraphs, quotes and loose items,
and a tight item only when it starts with one, while headings and table cells
admit inline formulas. Code, images, plain URLs and angle autolinks remain opaque.
Selected links retain literal destinations and titles while their labels may contain
formulas. A formula that starts before authored link syntax consumes that syntax
rather than creating an anchor. The selected pack's
error role colors formula errors when painted, and the matching math stylesheet
and fonts are inserted once per document.

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
