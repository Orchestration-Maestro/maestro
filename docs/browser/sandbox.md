# Sandbox loading and runtime messaging

The browser sandbox modules provide `RuntimeMessageRouter`, `MessageConsumer`
and `RuntimeMessageBridge`. The crate root exports `RuntimeMessageBridge` and,
on WebAssembly, the browser-local `RUNTIME_MESSAGE_ROUTER` singleton.

## Retained HTML loading

On WebAssembly, `maestro_web::SandboxIframe` retains a caller-supplied native
parent. `load_content` replaces its previous frame and registration, prepends a
fresh console provider and binds the frame before appending it. Authored-content frames allow
scripts and modals, without same-origin access. A live `sandbox_url_provider`
selects URL loading; otherwise the document uses srcdoc. Only the current
frame's readiness sends the prepared document, once. Its bootstrap error reaches
the same registered providers and consumers as ordinary runtime errors.

`remove` detaches the adapter's single message subscription, unregisters its
current sandbox and removes the frame. It is repeatable; failed native detachment
returns a diagnostic and retains resources for another removal attempt. Drop
attempts the same cleanup. Removing one adapter leaves other registered sandboxes
active. Failed loads return their first diagnostic and attempt this cleanup too.
The subscription uses the existing native event owner, not a separate teardown
mechanism.

`prepare_html_document` does not mount or register a frame or add a console
provider. Its module's `PrepareHtmlOptions` selects authored-HTML insertion or an
asynchronous JavaScript wrapper. Standalone documents retain supplied provider
helpers and data without parent messaging or navigation interception. Provider
data uses native compact JSON serialization; unsupported values fail rather than
becoming successful null assignments. Generated programs load from separate UTF-8 data resources, preserving literal
values without interpreting program text as HTML. Quoted global keys produce valid programs; native Window indexed
properties still ignore numeric assignments, as they do for any browser script.
External HTML and vector-graphic links resolve through native document addresses
before interception.

Native HTML parsing accepts recovered markup. Explicit validation errors appear
as literal preformatted text, with the selected brand pack's background, text,
error, panel and monospace roles; authored sandbox content keeps its own styling.
Loading is distinct from [hidden JavaScript execution](https://github.com/Orchestration-Maestro/maestro/issues/815).

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
not the provider graph. Window subscriptions share the browser event-registration
owner used by native input controls; extension transport stays with the router.

`browser::sandbox_value::BrowserSandboxValue` owns a native JavaScript handle.
Clones share that value: callbacks do not JSON-round-trip it. Its application
formatter uses native two-space JSON display for objects/arrays/null and native
string conversion otherwise, preserving serializer failures.

## Concrete providers

The browser SDK exports the application-owned
[console, attachment and returned-file providers](../application/browser-tools.md#sandbox-runtime-providers).
Instantiate them with `BrowserSandboxValue` for native handle retention, then
register them in the router's provider collection. Their complete injected
functions supply console forwarding/completion flushing, attachment reads and
online/offline file returns. The host providers install no listeners or timers;
the injected document owns those browser effects. Native collection aliases stay
live until reset replaces the current collection.

## Qualification boundary

Native tests exercise document envelopes and shared routing. The browser-executed
loading witnesses cover native listener removal, current-window admission,
ready/error settlement, preparation/runtime values, literal validation documents
and controlled host failures in Chromium. The tabs capability is controlled in
those tests, not an extension-platform certification. Broader browser/device
qualification remains tracked in [browser qualification](https://github.com/Orchestration-Maestro/maestro/issues/825)
and [surface qualification](https://github.com/Orchestration-Maestro/maestro/issues/1084).
