# Extension component bindings

The custom message-provider example uses workspace-inherited `httpdate =1.0.3`
(MIT OR Apache-2.0) as a portable development dependency to read HTTP-date retry
hints in WASI components. The guest library does not depend on the parser.
Controlled transport tests cover numeric and dated hints, invalid hints and the
fallback backoff. HTTP attempts and waits use the existing host operations.

The shipped `custom_footer` example uses the workspace's `ryu-js =1.0.3`
(Apache-2.0) as a development dependency to format token counts and dollar costs.
For example, 1,250 tokens display as `1.3k`, and a cost of `$0.0625` displays as
`$0.063`.
The formatter is not a normal dependency of the guest library.

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

The runtime and native test host enable `cache` to reuse compiled bytes through
Wasmtime's safe, content-keyed cache, which lives in `component-cache` below the agent
directory. Cache setup, read or write failures
fall back to compilation. Each load keeps its own Store. The Git build includes
the host executable's modification time in cache keys. The test setup aligns
executable timestamps before warming every exported component, so separate test
binaries reuse compatible entries.

The runtime and native test host enable `gc` and `gc-drc` for WebAssembly
exceptions. This lets separately built C engines return stack-unwinding failures
through their component interface without poisoning the instance.
