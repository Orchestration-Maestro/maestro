# Bash spawn hook

Build with `--example bash_spawn_hook` using the [build command](README.md#building-an-example). Loading is
opt-in and needs a native component-enabled Unix host with Bash and a readable
`~/.profile`. The example overrides `bash` through the host's native tool factory;
it keeps the extension's initial working directory, prefixes each command with
`source ~/.profile\n`, and sets `MAESTRO_SPAWN_HOOK=1` in the inherited environment.
It leaves native output handling, truncation, cancellation and timeouts alone.

Sourcing a profile executes its shell code before every command. Use only a
trusted profile; any directory or environment changes it makes also affect the
command. A missing profile reports Bash's source error; the following command
still runs unless shell options stop it. The example installs no profile or
built-in policy. The controlled `bash_spawn_hook_` run loads the built component
and executes the real bash tool in a disposable child with its own profile,
working directory and environment. It needs Bash but no provider or credentials.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- bash_spawn_hook_
```
