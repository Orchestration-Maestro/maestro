# Input transform

`?quick <question>` returns `Respond briefly in 1-2 sentences: <question>`,
trimming the question's edges with the host's shared whitespace definition.
An empty question warns `Usage: ?quick <question>` and stops. `ping` and `time`
ignore ASCII case and stop after informational feedback: `pong` or the host's
current local date and time. Other text, including extension-injected input,
continues unchanged. The rewrite omits replacement images, leaving the host to
keep the supplied attachments.

Time uses the host's local time zone with the en-US pattern
`M/D/YYYY, h:mm:ss AM`; other locale patterns are not selected. Build with
`--example input_transform` in the [build command](README.md#building-an-example). The controlled `input_transform_`
runs supply whitespace and clock replies, invoke both adapters and need no
provider, credentials or installation. Loading is opt-in and needs a
component-enabled host with notification UI. Quick questions still use that
host's configured model and can incur provider charges; ping, time and empty
quick input do not call a model.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- input_transform_
```
