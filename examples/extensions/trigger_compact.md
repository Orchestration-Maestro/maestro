# Trigger compact

`turn_end` records the context usage the host reports and requests compaction when
a known estimate at or below 100,000 tokens is followed by a known estimate above
it. `/trigger-compact` requests compaction immediately, and its arguments become
the instructions the host passes on, trimmed, or none when they are blank.

Usage the host does not report leaves the last known estimate as it was, and the
command never changes it. Compaction, its notifications and any history change
belong to the host: the extension only asks for them, reports `Compaction started`
before the request and `Compaction completed` or `Compaction failed: <message>`
when the host answers, each one only while a UI is available.

No provider, paid service or extension installation is required for the
controlled run. Loading the built component into an application is opt-in, and a
real compaction also needs that application's model and runtime configuration.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- trigger_compact_component_matches_host_requests
```
