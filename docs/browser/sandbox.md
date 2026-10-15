# Sandbox runtime messaging

The browser sandbox modules provide `RuntimeMessageRouter`, `MessageConsumer`
and `RuntimeMessageBridge`. The crate root exports `RuntimeMessageBridge` and,
on WebAssembly, the browser-local `RUNTIME_MESSAGE_ROUTER` singleton.

## Injected bridge

Generate source for the iframe or extension user-script channel with the
module's typed options. The following example is also checked in the generator's
public rustdoc:

```rust
use maestro_web::RuntimeMessageBridge;
use maestro_web::components::sandbox::runtime_message_bridge::{
    RuntimeMessageBridgeOptions, RuntimeMessageContext,
};

let source = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
    context: RuntimeMessageContext::SandboxIframe,
    sandbox_id: "execution",
})?;
assert!(source.contains("window.sendRuntimeMessage"));
# Ok::<(), maestro_app::DiagnosticErrorInfo>(())
```

The source installs `sendRuntimeMessage` and ordered `onCompleted` callbacks.
Iframe requests correlate replies by message id and reject after 30,000 ms;
user scripts await the extension's native reply. Identifiers are quoted without
trimming or extra HTML escaping. Document/script-tag preparation belongs to the
embedding host.

## Router ownership and sender admission

On WebAssembly, register a sandbox before creating its iframe, then call
`set_sandbox_iframe`. Retain the live provider collection if providers will
change. Providers run before the live identity-deduplicated consumer set;
the first callback failure stops that delivery. An admitted message retains its
context even when the registry entry is removed or replaced.

A window message is dispatched only if its nonempty string `sandboxId` names a
registered context and its source is that iframe's current `contentWindow`.
Missing sources, unbound frames and other windows are ignored. Origin text is
not an admission criterion. Extension messages instead select a registered id;
they do not have an iframe-source requirement.

Window replies use the retained context's current iframe slot, with
`type: "runtime-response"`, the original message id and sandbox id followed by
provider response fields, which may override those fields. Extension replies
copy response fields followed by the sandbox id, which wins. Raw missing
message ids remain undefined.

Call `unregister_sandbox` explicitly when a sandbox is destroyed. The final
removal detaches the global channel listeners; registration and removal errors
are returned as diagnostics. Dropping a router also attempts removal. Listeners
hold the registry weakly, and response callbacks retain only the frame slot,
not the provider graph.

`browser::sandbox_value::BrowserSandboxValue` owns a native JavaScript handle.
Clones share that value: callbacks do not JSON-round-trip it. Its application
formatter uses native two-space JSON display for objects/arrays/null and native
string conversion otherwise, preserving serializer failures.

## Qualification boundary

Native tests exercise the shared registry, admission decision, live dispatch,
retained frame slots and exact injected source. WebAssembly builds and compile
witnesses check binding and SDK signatures. They do not certify actual browser
listener effects, window identity, extension transport or native formatting;
those remain tracked in [browser qualification](https://github.com/Orchestration-Maestro/maestro/issues/825).
