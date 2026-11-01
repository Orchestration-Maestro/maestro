# Browser transcript

## Message rows

`UserMessage<C>`, `AssistantMessage<D>`, `ToolMessage<D>`, `ToolMessageDebugView`
and `AbortedMessage` are available at the web crate root and under
`components::messages`. They mount through `DomContent` with no attachment host.
`Default` leaves required operands unset and optional ones absent; mounting a row
without its required message or call returns an error.

`UserMessage` holds the original application message. The row selects the first
text block without trimming its source. Only the attachment-bearing user branch
emits tiles, in original order, retaining the original shared handles. Candidate
children are prepared before replacing the contents of the containing root, so a
failed preparation preserves its last content.

`AssistantMessage` retains the original shared assistant and tool collection. It
admits text and thinking with the shared whitespace rules, preserving the
displayed source and content order. Children retain their roots and thinking
expansion by emitted position and kind. `ToolResults::Live` retains the original
mutable standard envelopes, resolving each reached result independently;
`ToolResults::Native` retains standalone native-detail envelopes without cloning
or serialization bounds on their details. Tool visibility, pending visibility and
streaming are independent. Nonstreaming usage delegates formatting and uses the
current cost action; replacing, hiding or releasing that action revokes the old
control. Literal localized errors and aborts remain visible when selected,
including while streaming.

`ToolMessage` holds the original call and result envelope as shared handles. An
empty or absent `tool` name selects the call's own name. An abort replaces any
result with an error result for the same call ID and name, stamped with the
current time, and ends streaming. The selected renderer receives the original
argument object and result details. Only default content gets the ordinary card.
A changed card choice publishes fully prepared content; a failed candidate keeps
the last one.

`ToolMessageDebugView` shows the call arguments as JSON and, when `has_result` is
set, the joined text output and the details. Missing values show empty code,
`null` stays `null`, JSON strings are parsed strictly and other text stays
literal, never decoded as base64. Listings stay in place while the result stays
shown. `AbortedMessage` shows the localized abort text.

Row text is ordinary page text, so the browser's own find and selection reach it.
Rows add no hidden text copy, focus registry or placement system of their own.

## Custom message roles

`register_message_renderer`, `get_message_renderer` and `render_message` use
open role strings. Registering the same role replaces the current callback for
that native message representation. A previously retained callback remains
usable. Unknown roles return no content. Callbacks prepare owned page content.
The registry releases its guards before invoking or dropping user callbacks.

A custom message can carry a native handle without JSON conversion:

```rust,ignore
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use maestro_app::{AgentMessage, CustomAgentMessages};
use maestro_web::{MessageRenderer, register_message_renderer};
use maestro_web::dom::{DomContent, DomRoot, DomView};
use maestro_web::tools::types::ToolUiError;

struct NativeMessage {
    role: String,
    counter: Arc<AtomicU32>,
}
impl CustomAgentMessages for NativeMessage {
    fn role(&self) -> &str { &self.role }
}
struct Label(String);
impl DomContent for Label {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let node = view.document.create_element("span").map_err(|e| ToolUiError::native(&e))?;
        node.set_text_content(Some(&self.0));
        Ok(DomRoot::new(node))
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        root.node.set_text_content(Some(&self.0));
        Ok(())
    }
}
struct CounterRenderer;
impl MessageRenderer<NativeMessage> for CounterRenderer {
    fn render(
        &self, message: &AgentMessage<NativeMessage>,
    ) -> Result<Box<dyn DomContent>, ToolUiError> {
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
counter changes the next prepared content; no registry or message guard spans
mounting. `MessageRenderer<C, T>` names the produced content as its second type
parameter, page content by default in the browser build.
`message_renderer_registry::dom` holds the same two lookups under the browser
module path.

## Message list

`MessageList<C>` shows the completed messages of a session. `messages` is the
shared history (`Arc<RwLock<Vec<AgentMessage<..>>>>`); `tools`,
`pending_tool_calls`, `is_streaming` and `on_cost_click` are forwarded to each
assistant row. Mount it once and call `update` after changing the history or a
setting.

Preparing the list reads the history twice: once to index every tool result by
its exact call ID, keeping the last result for a repeated ID, and once in order to
choose each row. Neither read holds the history while a renderer runs, so a
renderer may edit the history or a record. A result edited after indexing is
still found under its indexed ID, and the row sees the edited envelope.

Artifact entries are skipped before any role renderer sees them. A registered
role renderer comes first for every other entry and receives the original
message handle; its content is used even when it is empty, and its error is
returned unchanged. Otherwise ordinary and attachment-bearing user entries show
user rows and assistant entries show assistant rows. Result entries and unknown
roles show nothing; their results appear inside the assistant row that made the
call. Assistant rows never stream and never hide tool calls; they hide pending
calls only while `is_streaming` is set.

Rows are placed by emitted position, counting only shown entries. A position
keeps its root while it shows the same kind of row, so skipped entries before it
do not move it; a different kind replaces the root, and positions past the last
row are removed.

## Streaming message

`StreamingMessageContainer<C>` shows the assistant message a response is
writing. `set_message(message, false)` keeps only the latest message and shows it
on the next animation frame, as a copy of its content taken then: several writes
in one frame show once, edits made before that frame are included, and later
edits to the original do not change what was shown. `set_message(message, true)`
shows the original message at once, so later edits appear on the next `update`.
Clearing with `None` is immediate too. Either way, a frame queued earlier is
retired and cannot show stale content, and the next deferred message shows on its
own first frame. The frame comes from the shared cancel-on-drop browser timer.

Only assistant messages are shown; user, result and caller-defined messages show
nothing, as the message list shows them. The assistant row receives `tools`,
`pending_tool_calls`, `tool_results_by_id`, `on_cost_click` and `is_streaming`;
it never hides tool calls or pending calls. While `is_streaming` is set, a pulsing
caret in the muted color follows the row, or stands alone before a message
arrives. Updates keep the assistant row's root. A released root receives no
later frames; mounting again shows the current message.

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

`MarkdownBlock::default()` and `ThinkingBlock::default()` are page content: they
implement `DomContent` without a canvas host, and `MountedContent` places them.
Markdown mounts a `div.maestro-markdown` root and puts the prepared document in
the page's light DOM, so its text stays findable and selectable. An update with
the same source and escaping keeps every node and refreshes the listings with the
supplied syntax engine and localization. Switching to the reasoning presentation
changes only the root's class, so listings, copy feedback and selection stay.
Changed source is prepared, highlighted and assembled off the page, then replaces
the root's children. A failed update returns its error and leaves the previous
document in place.

The reasoning disclosure is a native button named `Thinking...` with a chevron.
Pointer, Enter or Space activation toggles it once. While expanded it shows the
untrimmed source as a reasoning `MarkdownBlock`; the body exists only then. Each
root keeps its own expansion through content and streaming updates, and dropping
the root releases its toggle. The header shimmers while streaming. Its color and
the chevron's rotation change through 150 ms transitions, and a reversed toggle
turns back from the current angle. The `new(host)` constructors keep drawing on a
canvas for the artifacts that still use one.

Tool-renderer listings use the same listing pipeline with literal input; see
[tool renderers](tool-renderers.md). Their text is not auto-decoded.

## Provider-card continuation

[Custom provider cards](components.md#custom-provider-cards) mount as native page
content independently of the transcript. The editor and complete chat composition
remain separate continuation work; the card does not deliver those surfaces.
