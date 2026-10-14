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

The assistant constructor accepts an optional shared assistant message and
`AssistantMessageOptions`. Defaults show thinking, use the hidden label
`Thinking...` and enable hidden-label italic decoration. Text and thinking are
trimmed with the [shared whitespace predicate](../text.md). Visible blocks retain
their order, with a leading spacer and a gap after thinking only when another
visible block follows. Hidden thinking uses the literal label instead of Markdown.

`update_content` replaces the retained shared handle and rebuilds the inner
content. Both thinking setters and `invalidate` rebuild from that handle, observing
edits through a surviving message alias. Any tool-call block suppresses prompt
markers and failure notices. Without tools, aborted/error messages append their
notice, and nonempty rendering receives the same markers as a user message.
Absent/empty abort errors and `Request was aborted` display `Operation aborted`;
absent/empty error text displays `Error: Unknown error`. Successful stop reasons
ignore error text. Whitespace-only errors are not trimmed.

## Summaries

Branch and compaction summaries start collapsed in a `customMessageBg` panel.
`set_expanded` rebuilds even for an unchanged flag; `invalidate` also rebuilds the
current display. Collapsed hints read the active `app.tools.expand` keys when
rebuilt, using the application's [`key_text`](../../crates/maestro-tools/src/keybinding_hints.rs).
Expanded summaries show a Markdown header and the summary body, without prompt
markers. The compaction constructor formats its token count once with the supplied
[`LocaleNumberFormat`](../text.md#numbers), returning an I/O error on conversion
failure; toggling does not read the environment.

All four widgets expose the existing editable child-array interface through
`Deref`: user/assistant target `Container`, summaries target `Box`. Child identity,
wrapping, empty styled text and narrow-viewport padding follow the toolkit owner.
Background callbacks read the live theme; cached Markdown is refreshed by
invalidation. Summary labels and hints are repainted when rebuilt.

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
expansion keys at completion-display rebuild. Missing themes leave text unstyled.
`invalidate` first invalidates existing children and then rebuilds the content.
The inherited editable `Container` child array remains available through `Deref`.
