# Native component execution

`maestro-extensions-wasmtime::artifact_factory(path)` creates a local extension
factory for an already-built component. Pass it to
`maestro_extensions::load_extension_from_factory` on a Tokio local task set.
Construction does not discover files or compile source.

Each extension has one component Store and one local request receiver. Native
contexts, UI and session owners stay on their original thread. Typed requests
connect those owners to the Store; synchronous callbacks use that same receiver.
Retained UI and history resources keep their original owners when bindings change.

A component can wait in a synchronous host call while that call's native code
calls back into the same component: its message renderer, a retained component,
tool argument preparation, a compaction notification or a listener. The callback
runs inside the waiting component, and its result reaches the native caller
before the host call returns, as it would for an idle component. Host calls whose
native code may call back return a route that the component drives until the
call's own result arrives. Calls into other components are unaffected.

The adapter delegates registrations and actions to
[the extension facade](registration.md). Its fixed UI imports forward notification,
status, working-row, indicator, hidden-thinking-label, widget and title requests
without choosing presentation defaults. Credential commands belong to the supplied
context's model catalog, not the component factory.

Component faults are retained for later calls on that instance. Other instances
keep their own admission and fault state. Final resource destruction runs through
the Store's existing lifecycle, after admitted work and retained consumers finish.
