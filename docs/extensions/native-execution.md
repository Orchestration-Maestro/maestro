# Native component execution

`maestro-extensions-wasmtime::artifact_factory(path, widgets, installation)` creates a
local extension factory for an already-built component whose host widgets `widgets`, an
`ExtensionWidgets`, builds. Pass it to
`maestro_extensions::load_extension_from_factory` on a Tokio local task set.
The `Installation { version, agent_dir }` value comes from the application
configuration owner. Construction does not discover files or compile source.

Compiled artifacts use Wasmtime's user cache (`$XDG_CACHE_HOME/wasmtime`, or
`~/.cache/wasmtime` by default on Linux); cache failures fall back to compilation,
and each load creates a fresh Store.

On x86-64, extension code is compiled for the baseline feature level, not for the exact CPU
that loads it. Compiled code is therefore the same on every x86-64 host, and a cache filled
on one machine serves the others. Code that relies on newer instructions, such as SIMD-heavy
loops, runs slower than code tuned to the host. Other architectures keep Wasmtime's defaults.

Each instance reads the artifact's path as the `MAESTRO_EXTENSION_PATH`
environment variable, so a component can find files installed beside it. The
value is the factory's `path` as supplied. Through `load_extensions` that is the
module loader's lexically normalized absolute path, which keeps
directory-symlink spelling. It is a host path, so keep the host's separator when
deriving sibling paths. Other native environment entries are inherited unchanged;
see [component operating environment](host_io.md) for the delivered access and
remaining filesystem limitations.

Each extension has one component Store and one local request receiver. Native
contexts, UI and session owners stay on their original thread. Typed requests
connect those owners to the Store; synchronous callbacks use that same receiver.
Retained UI and history resources keep their original owners when bindings change.

Events cross to a component as the records the [guest guide](guest.md) lists.
The host keeps provider response headers in a map ordered by name; a component
receives them as name and value pairs in that order, and an edited list goes
back into the map, where a repeated name keeps its last value.

A component can wait in a synchronous host call while that call's native code
calls back into the same component: its message renderer, a retained component,
tool argument preparation, a compaction notification or a listener. The callback
runs inside the waiting component, and its result reaches the native caller
before the host call returns, as it would for an idle component. Host calls whose
native code may call back return a route that the component drives until the
call's own result arrives. Asynchronous callbacks run through their first suspension
before the native caller continues; their tails settle afterward without blocking
that caller. A callback that does not suspend returns its result immediately.
Calls into other components are unaffected.

The adapter delegates registrations and actions to
[the extension facade](registration.md). Its fixed UI imports forward notification,
status, working-row, indicator, hidden-thinking-label, widget and title requests
without choosing presentation defaults. Dialogs run on the originally acquired UI
and keep the component free while they wait. Credential commands belong to the supplied
context's model catalog, not the component factory.

When the component's factory returns an error, loading fails with that message as
written, so `load_extensions` reports `Failed to load extension: <message>`, and
the instance is torn down. A trap during initialization is a component fault and
reads `component fault <path>: <cause>`.

Component faults are retained for later calls on that instance. Other instances
keep their own admission and fault state. Final resource destruction runs through
the Store's existing lifecycle, after admitted work and retained consumers finish.
