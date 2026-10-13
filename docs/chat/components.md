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

The application exports `key_hint`, `key_text` and `raw_key_hint` from their
[tools owner](../../crates/maestro-tools/src/keybinding_hints.rs). The toolkit exports its shared `Cancellation` type.
