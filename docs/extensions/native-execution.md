# Native component execution

`maestro-extensions-wasmtime::artifact_factory(path)` creates a local extension
factory for an already-built component. Pass it to
`maestro_extensions::load_extension_from_factory` on a Tokio local task set.
Construction does not discover files or compile source.

Each instance reads the artifact's path as the `MAESTRO_EXTENSION_PATH`
environment variable, so a component can find files installed beside it. The
value is the factory's `path` as supplied. Through `load_extensions` that is the
module loader's lexically normalized absolute path, which keeps
directory-symlink spelling. It is a host path, so keep the host's separator when
deriving sibling paths. The instance does not receive the host's own environment.

Each extension has one component Store and one local request receiver. Native
contexts, UI and session owners stay on their original thread. Typed requests
connect those owners to the Store; synchronous callbacks use that same receiver.
Retained UI and history resources keep their original owners when bindings change.

Events cross to a component as the records the [guest guide](guest.md) lists.
The host keeps provider response headers in a map ordered by name; a component
receives them as name and value pairs in that order, and an edited list goes
back into the map, where a repeated name keeps its last value.

A component that waits for a synchronous host call cannot run again until that
call returns. A native call into that same component in the meantime, such as its
message renderer, a retained component, tool argument preparation, a compaction
notification or a listener, fails at once with `extension cannot be re-entered
while it waits for a synchronous host call`. Calls into other components still
complete. Asynchronous host calls leave the component free, so native work they
start can call back into it.

Component callbacks follow the [event bus](event_bus.md) delivery rules. These
callbacks are a listener prefix, tool argument preparation, a message renderer, a
retained view's render, input, key-release and invalidate methods, and both
compaction notifications. While one runs, its own instance's listeners run at once.
Other instances' listeners run when it returns, before the native caller continues.
A returned error keeps its exact text and those deliveries. Returned values reach
their native consumer unchanged; prepared arguments pass the model's argument
validation before the tool executes. A returned view keeps its own state, outlives
a replaced renderer and is released by its last owner. Removing a subscription
leaves an admitted tail and its captures running until the tail settles. Footer,
header, widget, editor and autocomplete factories and an authored dispose callback
are not part of the component interface yet.

The adapter delegates registrations and actions to
[the extension facade](registration.md). Its fixed UI imports forward notification,
status, working-row, indicator, hidden-thinking-label, widget and title requests
without choosing presentation defaults. Credential commands belong to the supplied
context's model catalog, not the component factory.

When the component's factory returns an error, loading fails with that message as
written, so `load_extensions` reports `Failed to load extension: <message>`, and
the instance is torn down. A trap during initialization is a component fault and
reads `component fault <path>: <cause>`.

Component faults are retained for later calls on that instance. Other instances
keep their own admission and fault state. Final resource destruction runs through
the Store's existing lifecycle, after admitted work and retained consumers finish.
