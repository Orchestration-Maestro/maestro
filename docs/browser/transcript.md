# Native copy controls

`maestro_web::components::copy_button::CopyButton` retains a supplied
`AttachmentHost`. Construct it with the current localization owner, set `text`,
`title` and `show_text`, then call `render` with the frame's UI and presentation.
The default text is empty, the title is localized `Copy` and success text is off.
An explicitly empty title stays empty.

Activation starts the native copy immediately, capturing the current original
text. Pending and rejected operations do not show a new success indication.
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
