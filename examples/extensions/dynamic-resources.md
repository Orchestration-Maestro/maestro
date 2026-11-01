# Dynamic resources

The [`dynamic-resources/main.rs`](dynamic-resources/main.rs) extension contributes
its sibling [skill](dynamic-resources/SKILL.md),
[prompt template](dynamic-resources/dynamic.md) and
[theme](dynamic-resources/dynamic.json) through `resources_discover`.
The paths come from the installed component, not the session working directory.
Startup and reload read the actual files. Move all four files together to keep
discovery beside the component.

## Build and load

Use the pinned toolchain with the `wasm32-wasip2` target:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example dynamic_resources --target wasm32-wasip2 --release
```

Install the built `dynamic_resources.wasm` beside `SKILL.md`, `dynamic.md` and
`dynamic.json`. Loading it is opt-in and requires Maestro's native component
runtime. Pass the installed component's path with `maestro -e <path>`; keep all
four files together when relocating it. The host supplies `MAESTRO_EXTENSION_PATH`
for that instance; do not set it yourself.

Discovery exposes the `dynamic-resources` skill, the `/dynamic` prompt template
and the `dynamic-resources` theme. Discovery and reload need no model call.
Sending the expanded prompt uses the session's configured provider.

## Controlled runs

These tests use disposable installations, unrelated working directories and the
controlled provider catalog. They require no live service or credentials.

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -E 'test(dynamic_resources_component_returns_shared_paths)'
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-app --test integration -E 'test(dynamic_resources_discovery_and_reload_load_shipped_files)'
```

The first run executes the built component at two installed paths. The second
uses that run's shared discovery result through the real session and resource
loader, then changes all three files and checks their reloaded contents and
provenance.
