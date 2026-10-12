# Session history libraries

`maestro-session` uses the existing agent/model interfaces and the shared request
record owner. It adds no execution runtime or session-file operations. Stored timestamp conversion
uses the existing [message formatting](../sessions/messages.md) owner.

| Dependency | Version/features | Purpose |
| --- | --- | --- |
| chrono | `=0.4.45`, defaults off; `std`, `clock`, `wasmbind` | Constructor and context ISO, date and local timestamp conversion |
| uuid | `=1.27.0`, defaults off; `std`, `v4`, `v7`; browser additionally `js` | Generated session and entry identities |
| ryu-js | workspace `=1.0.3` | Exit-status number spelling; Apache-2.0 license option |
| serde_json | workspace `=1.0.151`; `preserve_order`, `float_roundtrip` | Ordered raw entries and open custom payload |
| serde | workspace `=1.0.229`; `derive` | Explicit record views and test case decoding |
| tokio | native dev only `=1.53.2`, defaults off; `rt` | Controlled agent-constructor witness |

The promoted serde dependency uses already locked serde/serde_core/serde_derive,
proc-macro2, quote, syn and unicode-ident packages; no package version is added. License choices are MIT or
Apache-2.0, plus Unicode-3.0 where applicable. Agent/model/request dependencies
retain their existing dependency closure.

See [message projection](../sessions/messages.md) for the delivered API.

See [stored-entry context](../sessions/session_manager.md) for raw entry parsing
and branch projection.

Memory history uses chrono UTC emission with millisecond precision and a `Z`
suffix. The UUID native closure uses already locked getrandom 0.4.3, cfg-if 1.0.5,
libc 0.2.190 and rand_core 0.10.1. Its browser closure uses wasm-bindgen and its
macro/support/shared packages at 0.2.129, bumpalo 3.20.3, cfg-if 1.0.5,
once_cell 1.21.4, proc-macro2 1.0.107, quote 1.0.47, rustversion 1.0.23,
syn 3.0.6 and unicode-ident 1.0.24. These use MIT or Apache-2.0 license options,
plus Unicode-3.0 for unicode-ident; no new execution runtime is added.
