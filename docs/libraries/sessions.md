# Session projection libraries

`maestro-session` uses the existing agent/model interfaces and the shared request
record owner. It adds no execution runtime or filesystem operations.

| Dependency | Version/features | Purpose |
| --- | --- | --- |
| chrono | `=0.4.45`, defaults off; `std`, `clock`, `wasmbind` | Constructor ISO, date and local timestamp conversion |
| ryu-js | workspace `=1.0.3` | Exit-status number spelling; Apache-2.0 license option |
| serde_json | workspace `=1.0.151`; `preserve_order`, `float_roundtrip` | Default open custom payload |
| serde | workspace `=1.0.229`; dev only | Recorded test case decoding |
| tokio | native dev only `=1.53.2`, defaults off; `rt` | Controlled agent-constructor witness |

The normal external all-target/build dependency closure contains 48 packages,
all already present in the workspace lockfile. License choices are MIT or
Apache-2.0, plus Unicode-3.0 where applicable. Agent/model/request dependencies
retain their existing dependency closure.

See [message projection](../sessions/messages.md) for the delivered API.
