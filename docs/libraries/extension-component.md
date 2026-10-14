# Extension component bindings

`maestro-extensions-wasm` uses `wit-bindgen =0.62.0` with defaults disabled and
`macros`, `realloc`, `async`, `std` and `inter-task-wakeup` enabled.

`inter-task-wakeup` supports waking a pending author future from another
component-model task, including a new-session setup callback. The generated
component is instantiated by the extension component tests; the feature adds no
package dependency.

The native test host uses `wasmtime =49.0.2` with its `cache` feature to reuse
compiled components across isolated test processes. This feature is enabled only
on the guest-authoring crate's development dependency, not in product builds.
