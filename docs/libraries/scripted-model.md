## Approved scripted-model libraries

The models crate uses rand `=0.10.3` with defaults. Native scheduling uses
Tokio `=1.53.3`, defaults off, with `rt-multi-thread`, `sync` and `time`.
Browser scheduling uses js-sys `=0.3.106`, wasm-bindgen `=0.2.129` and
wasm-bindgen-futures `=0.4.79` with defaults; getrandom `=0.4.3` enables
`wasm_js`. Compact serde_json `=1.0.151` retains `preserve_order` and the
workspace-wide `float_roundtrip`; the models crate adds `raw_value` to keep
number syntax as written, and `arbitrary_precision` stays off because it changes
every `Number`. These MIT/Apache-2.0 libraries add no internal crate edge or
public runtime API.
