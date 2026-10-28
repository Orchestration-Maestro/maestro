# Tool override

`tool_override` replaces `read` only when you load it. It records each resolved
path in `<agent_dir>/read-access.log` through the host's file-mutation queue,
including blocked attempts and failed reads. Logging failures do not prevent a
read. `/read-log` shows the last twenty records as an informational notification,
or `No access log found` when the log cannot be read.

The case-sensitive patterns block `.env`, `.env.<suffix>`, and paths containing
`/.ssh/`, `/.aws/` or `/.gnupg/`. The secret/credential filename patterns ignore
case and match singular or plural names ending in `.json`, `.yaml`, `.yml` or
`.toml`. These are literal illustrative patterns, not a security sandbox:
symlinks, alternate spellings and names outside them can bypass the checks.

Allowed reads return text, with optional numeric `offset` and `limit`, and the
total line count. The example decodes invalid UTF-8 with replacement characters
and clips selected text at a whole UTF-8 scalar within 50 KiB, followed by
`[Output truncated at 50KB]`. It supplies no custom renderer. This simplified
reader does not preserve the built-in read tool's image and truncation features.

Build with `--example tool_override` in the [build command](README.md#building-an-example). Loading is opt-in and
requires a component-enabled native host with filesystem access; recording also
needs an existing writable agent directory. Log display needs the host's
notification UI. Logs contain paths, timestamps and access decisions, not file
contents. The controlled run uses disposable files and no provider or credentials.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- tool_override_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- tool_override_
```
