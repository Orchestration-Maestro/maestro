# maestro-agent

Owns streamed text-conversation execution through `Agent`.
Embedding callers inject `maestro-models`, the only internal production dependency.
Private modules own admission, event reduction, FIFO policy and awaited settlement;
there is no alternate model invocation interface or executable-tool layer.

See [the agent guide](../../docs/agent.md) for configuration, errors and a compiling
credential-free example, [the glossary](../../CONTEXT.md) for execution terms,
and [the execution contract](../../docs/specs/2026-10-05-engine-core.md).

Focused checks:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo test -p maestro-agent
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just check
```
