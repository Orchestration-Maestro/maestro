# Session projection libraries

`maestro-session` uses the existing agent/model interfaces and the shared request
record owner. It adds no execution runtime or filesystem operations.

| Dependency | Version/features | Purpose |
| --- | --- | --- |
| chrono | `=0.4.45`, defaults off; `std`, `clock`, `wasmbind` | Constructor and context ISO, date and local timestamp conversion |
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
