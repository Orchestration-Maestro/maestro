# Session history libraries

`maestro-session` uses the existing agent/model interfaces and the shared request
record owner. Native session files use standard owned files; listing drives them through the caller's Tokio runtime and adds no runtime of its own. Stored timestamp conversion
uses the existing [message formatting](../sessions/messages.md) owner.

| Dependency | Version/features | Purpose |
| --- | --- | --- |
| chrono | `=0.4.45`, defaults off; `std`, `clock`, `wasmbind` | Constructor and context ISO, date and local timestamp conversion |
| uuid | `=1.27.0`, defaults off; `std`, `v4`, `v7`; browser additionally `js` | Generated session and entry identities |
| ryu-js | workspace `=1.0.3` | Compact JSONL and exit-status number spelling; Apache-2.0 license option |
| maestro-path | workspace utility, no dependencies | Lexical session filenames, directory joins and the single native home reader |
| maestro-lock | workspace owner | Write locks beside canonical targets, recoverable in-place publication and atomic publication |
| sha2 | `=0.11.0`, defaults off; MIT OR Apache-2.0 | Existing SHA-256 for session comparisons and shortened lock and record names |
| serde_json | workspace `=1.0.151`; `preserve_order`, `float_roundtrip` | Ordered raw entries and open custom payload |
| serde | workspace `=1.0.229`; `derive` | Explicit record views and test case decoding |
| tokio | native `=1.53.3`, defaults off; `fs`, `rt` | Asynchronous directory, read and stat calls for session listing; tests also build the caller's runtime |
| indexmap | `=2.14.2`, default `std` | Ordered summary-request headers on all targets |
| futures-util | workspace `=0.3.34`, defaults off; `std`, `sink` | All targets: join split-summary observations; native listing: drive candidate reads together |

The promoted serde dependency uses already locked serde/serde_core/serde_derive,
proc-macro2, quote, syn and unicode-ident packages; no package version is added. License choices are MIT or
Apache-2.0, plus Unicode-3.0 where applicable. Agent/model/request dependencies
retain their existing dependency closure.

See [message projection](../sessions/messages.md) for the delivered API.

See [stored-entry context](../sessions/session_manager.md) for raw entry parsing
and native history operations.

Memory history uses chrono UTC emission with millisecond precision and a `Z`
suffix. The UUID native closure uses already locked getrandom 0.4.3, cfg-if 1.0.5,
libc 0.2.190 and rand_core 0.10.1. Its browser closure uses wasm-bindgen and its
macro/support/shared packages at 0.2.129, bumpalo 3.20.3, cfg-if 1.0.5,
once_cell 1.21.4, proc-macro2 1.0.107, quote 1.0.47, rustversion 1.0.23,
syn 3.0.6 and unicode-ident 1.0.24. These use MIT or Apache-2.0 license options,
plus Unicode-3.0 for unicode-ident; no new execution runtime is added.

The native listing closure adds Tokio 1.53.3, futures-util, futures-core, futures-sink
and futures-task 0.3.34, pin-project-lite 0.2.17 and slab 0.4.12, all already locked
(MIT, or MIT OR Apache-2.0). See [discovery](../discovery.md).

Summary generation moves the existing futures-util declaration from native-only to
all targets, retaining workspace features and version. Ordered headers use the already
locked indexmap 2.14.2, equivalent 1.0.2 and hashbrown 0.17.1 closure (MIT OR Apache-2.0).
Completion, cancellation and provider work remain with the existing model owner;
no runtime, transport or workspace dependency edge is added.
