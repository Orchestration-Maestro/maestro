# Extension component bindings

`maestro-extensions-wasm` uses `wit-bindgen =0.62.0` with defaults disabled and
`macros`, `realloc`, `async`, `async-spawn`, `std` and `inter-task-wakeup` enabled.

`inter-task-wakeup` supports waking a pending author future from another
component-model task, including a new-session setup callback. The generated
component is instantiated by the extension component tests; the feature adds no
package dependency.

`async-spawn` provides `spawn_local`. When a host call made from inside a waiting
component calls one of its asynchronous callbacks, the callback's synchronous start
runs before that host call returns. If the callback then waits, the rest of it keeps
running in the same component task. The feature uses `futures 0.3.34`, which the
lockfile already contains.

The runtime and native test host pin `wasmtime` and `wasmtime-wasi` to unmodified
upstream commit `ffff2e2db24a291f02429f123aba1631fa8eea67` for its queued-event
delivery fix. Replace the Git pin with the first published release containing
that fix. The manifests and lockfile select the exact commit; the Git-source
allowlist names its repository.

The native test host enables `cache` to reuse compiled components across isolated
test processes. This feature is enabled only on the guest-authoring crate's
development dependency, not in product builds.
