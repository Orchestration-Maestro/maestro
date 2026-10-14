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
