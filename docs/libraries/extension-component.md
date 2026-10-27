# Extension component bindings

The custom message-stream provider example uses `base64 =0.23.1` (MIT OR
Apache-2.0, defaults off, `std`) and `sha2 =0.11.0` (MIT OR Apache-2.0,
defaults off) as development dependencies for PKCE. These are the same versions
and features the native authorization owner uses. The guest takes secure random
bytes from WASI; it adds no cryptographic implementation or production API.

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
Wasmtime's safe, content-keyed user cache. Cache setup, read or write failures
fall back to compilation. Each load keeps its own Store. The Git build includes
the host executable's modification time in cache keys, so separate host binaries
do not share entries, but isolated test processes of the same binary do.
