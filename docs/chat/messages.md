# Transcript messages

`maestro-chat` exports `UserMessageComponent`, `AssistantMessageComponent`,
`BranchSummaryMessageComponent` and `CompactionSummaryMessageComponent` at its root
and in their corresponding `components` modules. Constructors take a live theme,
prepared Markdown callbacks and shared terminal image capabilities. Use
[`get_markdown_theme`](../theme.md#component-styles) for ordinary Markdown styling;
custom callbacks remain supported. Layout delegates to the
[terminal toolkit](../terminal/widgets.md) and [Markdown owner](../terminal/markdown.md).

## User and assistant content

The user widget puts Markdown inside a padded `userMessageBg` panel with
`userMessageText` foreground. Nonempty user rendering prefixes the first row with
OSC 133 A and the last row with OSC 133 B/C, without adding a row.
User-authored Markdown keeps pasted terminal colour sequences unchanged.

The assistant constructor accepts an optional shared assistant message and
`AssistantMessageOptions`. Defaults show thinking, use the hidden label
`Thinking...` and enable hidden-label italic decoration. Text and thinking are
trimmed with the [shared whitespace predicate](../text.md). Their decoded Markdown
content is cleaned with the shared [display helpers](../tools-read.md#display-helpers)
before terminal styling and hyperlink generation. This removes authored escape
sequences and C0 controls other than TAB, LF and CR without removing renderer styling.
Visible blocks retain
their order, with a leading spacer and a gap after thinking only when another
visible block follows. Hidden thinking uses the literal label instead of Markdown.

`AssistantMessageComponent::from_record` builds the same widget from a stored
assistant record. It reads only the content blocks and the stop reason and error
text it displays, so tool-call arguments keep whatever JSON value was stored.

`update_content` replaces the retained shared handle, releasing a stored record,
and rebuilds the inner content. Both thinking setters and `invalidate` rebuild from that handle, observing
edits through a surviving message alias. Any tool-call block suppresses prompt
markers and failure notices, except the output-length notice below. Without tools,
aborted/error messages append their notice, and nonempty rendering receives the
same markers as a user message.
Absent/empty abort errors and `Request was aborted` display `Operation aborted`;
absent/empty error text displays `Error: Unknown error`. Successful stop reasons
ignore error text. Whitespace-only errors are not trimmed.

A message that stopped at the output token limit (stop reason `length`) always
appends `Error: Model stopped because it reached the maximum output token limit.
The response may be incomplete.`, with or without tool calls, because the limit can
cut a tool call short before its own row can report anything.

## Summaries

Branch and compaction summaries start collapsed in a `customMessageBg` panel.
`set_expanded` rebuilds even for an unchanged flag; `invalidate` also rebuilds the
current display. Collapsed hints read the active `app.tools.expand` keys when
rebuilt, using the application's [`key_text`](../../crates/maestro-tools/src/keybinding_hints.rs).
Expanded summaries show a Markdown header and the summary body, without prompt
markers. Both summary kinds enable the same decoded-content
[Markdown cleaning policy](../terminal/markdown.md) as assistant text and thinking.
The compaction constructor formats its token count once with the supplied
[`LocaleNumberFormat`](../text.md#numbers), returning an I/O error on conversion
failure; toggling does not read the environment.

All four widgets expose the existing editable child-array interface through
`Deref`: user/assistant target `Container`, summaries target `Box`. Child identity,
wrapping, empty styled text and narrow-viewport padding follow the toolkit owner.
Background callbacks read the live theme; cached Markdown is refreshed by
invalidation. Summary labels and hints are repainted when rebuilt.

## Custom messages and skill invocations

`CustomMessageComponent` renders an extension-supplied custom message and
`SkillInvocationMessageComponent` renders a parsed skill block; both are exported
at the crate root and in their `components` modules; the skill widget is
native-only, while the parsed record it renders belongs to the application and is
available to browser builds. Constructors take the live theme, prepared
Markdown callbacks and shared terminal image capabilities.

The custom widget keeps one leading spacer and then either the component returned
by its retained renderer or a default panel with `customMessageBg` background, a
bold literal `[<customType>]` label in `customMessageLabel`, a spacer and the body
as Markdown in `customMessageText`. String content is used unchanged; part lists
keep only text parts, in order, joined by newlines, so images never reach the
default display. `display` and `timestamp` are carried for callers and never
filtered here. `set_expanded` rebuilds only when the flag changes; `invalidate`
always rebuilds.

A renderer that returns a component owns its own styling and replaces the panel.
A creation error or a missing component selects the default panel without
reporting an extension error; a failure from the selected child's `try_render`
propagates to the caller unchanged. Supplying a renderer means calling
`MessageRendererCall::new` with the renderer and its event bus, or taking the call
from the runner as described in [registration](../extensions/registration.md).

The skill widget is a `customMessageBg` panel. Collapsed, it shows one row with the
bold `[skill]` label, the skill name and a dim `(<expand key> to expand)` hint read
from the active `app.tools.expand` keys. Expanded, it shows the label and the
Markdown `**<name>**` header followed by the skill content. Its location and
trailing user text belong to the caller and are not displayed. `set_expanded` and
`invalidate` rebuild on every call, including an unchanged flag.

Both widgets expose the existing editable child-array interface through `Deref`:
the custom widget targets `Container` and the skill widget targets `Box`.
Rebuilding replaces only the body the widget owns, so unrelated outer edits and
retained array aliases keep working.

## Direct shell output

`BashExecutionComponent`, exported at the crate root and in
`components::bash_execution`, takes an authored command, writer, live theme and
context-exclusion flag. Its border and bold command header retain `bashMode`, or
`dim` for excluded commands, regardless of completion status. No panel background
is painted.

`append_output` strips ANSI escapes and converts CRLF and remaining CR to LF per
chunk before storing it; stripper failures leave stored output unchanged.
`get_command` borrows the original command, and `get_output` returns the full
cleaned output. Display selection uses the existing [tail truncation owner](../tools-read.md):
expanded rows show that context-limited tail; collapsed rows select its last 20
logical entries and then at most 20 wrapped rows at the current render width.
`set_expanded` rebuilds even for an unchanged value.

The shared loader animates until `set_complete` stops it. Completion hints precede
cancellation or nonzero-exit notices, followed by the full-output path when either
context or supplied truncation is true and the path is nonempty. Cancellation
wins over exit errors; success adds no exit-zero notice. Completion may be replaced,
and output may still be appended without restarting animation.

Borders read the live theme at render time. Header, output and completion hints
capture theme values when rebuilt; cancel keys are captured at construction and
expansion keys at completion-display rebuild. Missing themes omit theme colours
while preserving existing decorations, including the bold command header.
`invalidate` first invalidates existing children and then rebuilds the content.
The inherited editable `Container` child array remains available through `Deref`.

## Tool execution rows

`ToolExecutionComponent::new` accepts a `ToolExecutionParams` record: the tool
name and call ID, optional raw JSON arguments, image options, independently
supplied custom/native `ToolRendererBinding`s, session cwd, render environment
and a render-request callback. Use the writer's `weak_render_request()` for that
callback. The application exports the native read/edit/write definition factories
and their options; call `ToolDef::renderer_binding()` to obtain their projections.
The row composes renderers, never executes a tool.

Each custom callback overrides only its own slot; an absent slot inherits the
native callback. Shell precedence is custom, native, then the padded default.
A self-rendered shell supplies its own framing. Only a row without either
binding displays generic arguments with [`pretty_json`](../json.md).
Callback failures fall back in that slot; failures from a returned child's
`try_render` propagate. Call and result slots share typed state but retain separate
previous components. Result callbacks receive content and optional raw details;
error status is carried only by the render context, not the result payload.
Each rebuild prepares the selected result binding again before rendering once,
using the current details, expansion and streaming flags. Preparation failures
use the same result-slot fallback as callback failures.

`update_args`, `mark_execution_started`, `set_args_complete`, `update_result`,
`set_expanded`, `set_show_images` and `set_image_width_cells` rebuild the retained
shell. Raw details and absent versus null arguments stay distinct. Result updates
may precede complete arguments; streaming and error flags remain independent.
Started/complete updates request rendering. Invalidation rebuilds against the live
theme and retains a formatting failure for `try_render`. The inherited `Container`
interface keeps outer edits and existing child-array aliases.

Images use the [terminal image owner](../terminal/images.md). Defaults show images
at a maximum of 60 cells. Both constructor widths and setter widths are floored
and clamped to at least one integral cell at the image boundary. Visibility and
terminal support are independent. Kitty conversion uses the existing synchronous
PNG converter; successful cache entries match both payload and MIME, and replaced
results discard stale entries. Image conversion schedules no deferred work.
Native edit previews retain their existing runtime and read-only preflight.
