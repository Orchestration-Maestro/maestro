# Protected paths

`tool_call` inspects only the `write` and `edit` tools and reads their `path`
argument without normalizing it. A path containing `.env`, `.git/` or
`node_modules/` is blocked with `Path "<path>" is protected`; when the host has
UI, the hook first warns `Blocked write to protected path: <path>` with warning
severity. Every other tool and path is left alone and returns no verdict.
Matching is a case-sensitive literal substring test: it does not resolve dot
segments, fold case, or look at the filesystem. The three patterns are this
example's own policy, not a built-in restriction; loading the component into an
application is opt-in.

The example needs a component-enabled host to load. The controlled run needs no
provider, credentials or installation: it delivers every recorded tool call to
the built component and to the in-process adapter.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- protected_paths_
```
