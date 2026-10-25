# Shared chat components

`maestro-chat` is a library of interactive components, not an executable.
It reuses the [terminal toolkit](../terminal/components.md) and
[theme styles](../theme.md#component-styles).

- `DynamicBorder` passes a freshly sized rule containing `width.max(1)` `─`
  glyphs to the selected painter and returns its result as one row.
  A supplied painter bypasses theme lookup; otherwise each render uses the live
  theme's [paint helper](../theme.md#component-styles), which owns fallback behavior.
- `CountdownTimer` reports `ceil(timeout_ms / 1000)` immediately without requesting
  a frame. One-second ticks decrement and report the count, request a frame when
  the supplied writer is still alive, then cancel the continuation before notifying expiry
  when the count is at most zero. Disposal cancels future countdown work;
  a tick already executing still finishes its notification, render and expiry
  decisions. The render request does not retain the writer. Dropping the last
  countdown owner also cancels the continuation.
- `BorderedLoader` defaults to cancellable. Its borders, spinner and message keep
  the supplied prepared theme; its hint is built once from the separate live
  theme and active keys. Input cancellation consults the active keys each time.
  The plain form does not build a hint and ignores input and abort callbacks.
  Disposal stops animation without aborting the signal. Container child handles
  remain available through `Deref`.
- `CustomEditor` checks extension shortcuts, image paste, interrupt, empty-editor
  exit, then other registered actions in insertion order. Image paste and
  empty-editor exit consume matching input even without a callback. Interrupt
  delegates to the editor when completion is visible or no handler is installed;
  it never falls through to generic actions. Other unclaimed input reaches the
  editor. Routing retains the supplied live key manager. Dynamic callback
  setters replace properties; action-map handles retain their identity after
  the editor's map slot is replaced. `Deref` exposes the inherent editor API;
  component, focus and editor traits delegate to the same editor, with input
  capabilities using the custom router.

- `FooterComponent` renders three rows at most. The first shows the working
  directory with its home shortened unless that home is empty or `/`, the branch
  in parentheses and the session name after a bullet. The second puts the
  cumulative token and cache totals, the subscription cost and the context
  display on the left, and the model with its provider and thinking level on the
  right, padded to the viewport width.
  When the left side leaves no column for the model after the two-column gap,
  the row holds the left side alone, without additional padding.
  The third shows the extension statuses in the shared locale order of their
  keys, each reduced to one line. Tokens spell as plain below a thousand, one
  decimal below ten thousand, whole thousands below a million, one decimal
  below ten million and whole millions above; costs carry three decimals and a
  subscription marker. The context display warns above 70 percent and errors
  above 90, shows `?` while tokens are unknown, and marks automatic compaction.
  Usage totals count every answer in the retained history, not the selected
  branch: a stored field that is missing suppresses its total, a stored null
  counts as zero, and an unreadable usage or cost record reports through
  `try_render` without a partial frame. Each row re-reads the session, the
  metadata provider and the published theme where it uses them, so a getter
  that changes the model or a theme republished mid-paint shows in the same
  frame. Left text truncates before the right side, the sides stay two spaces
  apart, and the footer registers no callback, so `invalidate` and `dispose`
  change nothing.

The [extension question dialogs](extension-ui.md) provide clamped selection and
text submission with optional countdowns and retained input focus.

The [transcript messages](messages.md) provide user, assistant and expandable
summary widgets, plus retained tool-execution rows over the same toolkit and live theme.

The application exports `key_hint`, `key_text` and `raw_key_hint` from their
[tools owner](../../crates/maestro-tools/src/keybinding_hints.rs). The toolkit exports its shared `Cancellation` type.

- Native [`ExtensionEditorComponent`](extension-ui.md) composes multiline input,
  live borders, captured hints and inherited-terminal external editing.
