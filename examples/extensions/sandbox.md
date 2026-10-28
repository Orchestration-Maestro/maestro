# Sandboxed bash

An opt-in extension that replaces bash execution with filesystem and network
restrictions. It does not sandbox other tools or installed extensions.

Build the component from the repository root:

```sh
cargo build --locked -p maestro-extensions-wasm --example sandbox \
  --target wasm32-wasip2 --release
```

Load the resulting `sandbox.wasm` with `maestro -e /path/to/sandbox.wasm`.
Linux needs `bwrap`, `socat`, `rg` and `bash` on PATH. macOS needs
`sandbox-exec` and `bash`. Windows is unsupported. Installation is opt-in;
this example does not install system programs.

Use `--no-sandbox` to disable the wrapper, or `/sandbox` to show its current
configuration. Global settings live at `<agent_dir>/extensions/sandbox.json`;
project settings at `<cwd>/.maestro/sandbox.json` take precedence.

The controlled tests run without provider credentials:

```sh
cargo nextest run --locked -p maestro-extensions-wasmtime \
  -E 'test(sandbox_example::)'
```
