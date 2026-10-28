# System prompt header

On `agent_start` the example reads the prompt the host supplies at that moment
and sets the `system-prompt` status to `System: N chars`. `N` counts UTF-16 code
units, so whitespace, a byte order mark and combining sequences count as the
string stores them; nothing is trimmed, rebuilt or cached. `session_shutdown`
removes the status under the same key, including when no start came before it.
A host rejection is returned to the host unchanged. Loading the component into
an application is opt-in.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- system_prompt_status
```
