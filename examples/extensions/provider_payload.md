# Provider payload

Load this example only when you need a local record of provider traffic. It
appends request payloads as two-space-indented JSON, followed by response status
and compact JSON headers, to `.maestro/provider-payload.log` under the working
directory. Each record ends with a blank line. Existing records stay in place;
requests are unchanged.

The log can contain private prompts, tool results and sensitive response headers.
It is not redacted or rotated. Keep it out of version control, restrict access,
and delete it when you finish inspecting the traffic. Directory and write errors
are reported through the extension host; the example does not create directories.

## Build and load

Create `.maestro` in the working directory first. Use a native host with guest
filesystem access. Building needs the repository's pinned Rust toolchain and
`wasm32-wasip2` target:

```sh
mkdir -p .maestro
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example provider_payload \
  --target wasm32-wasip2 --release
maestro --extension target/wasm32-wasip2/release/examples/provider_payload.wasm
```

Loading the extension is opt-in. An interactive prompt uses your configured
provider and may incur charges. The controlled test uses only a loopback provider,
cleared credentials and disposable files:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- provider_payload_
```

## Optional replacement

The request hook can return a replacement payload. In the registered callback,
replace `map(|()| None)` with `and_then(...)` so a successful append can return a
result. Match `ExtensionEvent::BeforeProviderRequest`, parse `event.payload` as
`serde_json::Value`, set `payload["temperature"] = serde_json::json!(0)`, then return
`Ok(Some(ExtensionEventResult::BeforeProviderRequest(payload.to_string().into())))`.
Import `ExtensionEventResult` from `maestro_extensions_wasm`. Return `Ok(None)` for
response events. The shipped example only logs; it does not change model settings.
