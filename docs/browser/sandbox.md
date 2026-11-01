# Sandbox execution, loading and runtime messaging

The browser sandbox modules provide `RuntimeMessageRouter`, `MessageConsumer`
and `RuntimeMessageBridge`. The crate root exports `RuntimeMessageBridge` and,
on WebAssembly, the browser-local `RUNTIME_MESSAGE_ROUTER` singleton.

The [browser integration command](../development.md#browser-integration-qualification)
builds and runs the existing execution and loading assertions with supplied
Chromium tools. It documents the selected exports, expected policy error and
cleanup behavior.

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

The [HTML artifact](artifacts.md#native-html-artifact) loads through this adapter
under the sandbox name `artifact-<filename>`, with the completion signal appended
after the whole authored source. Its downloads use `prepare_html_document` with
both the authored-HTML and standalone options.

`prepare_html_document` does not mount or register a frame or add a console
provider. Its module's `PrepareHtmlOptions` selects authored-HTML insertion or an
asynchronous JavaScript wrapper. Standalone documents retain supplied provider
helpers and data without parent messaging or navigation interception. Provider
data uses native compact JSON serialization; unsupported values fail rather than
becoming successful null assignments. Programs run as inline parser-inserted
scripts with their exact text: CR is kept, while U+0000 becomes U+FFFD on the HTML
parser route. Quoted global keys produce valid programs; native Window indexed
properties still ignore numeric assignments, as they do for any browser script.
HTML and vector links use native hyperlink state and effective resolved HTTP(S)
targets, including namespaced and animated vector addresses.

When the script policy admits the head boot script, the runtime's inert template
moves its parser-created script into the head before authored head content. When it
admits the body remover, the REPL module's non-rendering vector scaffold is removed
before execution. With both admitted, no carrier template or vector element remains
after loading; a policy omitting a boot script's digest leaves its inert template or
vector scaffold in the loaded document.
During runtime execution, `document.currentScript` is an `SVGScriptElement`; the built-in
providers do not read it. Hosts pin four digests over decoded text content, not encoded
markup: the runtime, module and these two constant boot scripts (SHA-256):

- Head boot: `7lLUb+wLFQezr+10zcikIzRbnDaVQGyuZnkcXksV7J4=`
- Body remover: `8q+8kOKrdFrQmwFljSnTFbN67/+QuOXbkIu3Fp6to98=`

Hash policies still refuse unmatched scripts. The carrier adds no script-string
sink, dynamic evaluation, program URL resource or policy relaxation.

Native HTML parsing accepts recovered markup. Explicit validation errors appear
as literal preformatted text, with the selected brand pack's background, text,
error, panel and monospace roles; authored sandbox content keeps its own styling.
Execution uses the same retained frame owner, but rejects validation errors instead
of painting this loading document.

## Hidden JavaScript execution

On WebAssembly, `BrowserSandboxHost` creates a hidden `sandbox-iframe` parent.
Its application trait adapter forwards to `SandboxIframe::execute` with no
additional consumers and JavaScript-wrapper mode. The browser-local
`JAVASCRIPT_REPL_TOOL` is a thread-local lazy application registration: access it
with `.with(...)` to read or replace its existing provider-factory and URL slots.
Initialization mounts nothing; each tool execution obtains its own adapter.
[Application output and download formatting](../application/browser-tools.md#javascript-execution)
stay with the application factory, also re-exported at the browser crate root.

Direct SDK execution takes an owned `SandboxExecution<BrowserSandboxValue>`,
additional message consumers and an independent `is_html_artifact` flag.
Providers run before additional consumers; internal result collection comes last.
A fresh console provider precedes supplied providers. Start/end notifications
preserve provider entry order, including repeated allocations. The live URL slot
is shared: use `sandbox.sandbox_url_provider.replace(Some(callback))`; it is read
after start callbacks. Loading, execution and removal accept shared receivers.

The direct result is `SandboxResult<BrowserSandboxValue, BrowserSandboxValue>`.
File names, MIME types and content handles are captured when each message arrives;
in-place edits to a captured buffer remain visible after settlement. Native return
handles preserve absence versus null, signed zero and native object kinds.
Terminal return/error fields are read after end hooks, including property
replacement, addition and removal. End hooks run before the application trait
converts file content to the default owned byte/text records. Actual byte arrays
become bytes; strings stay text; other content uses native string conversion.
`SandboxResult<V>` retains that default payload type. The adapter does not
serialize returns or encode downloads.

Cancellation before terminal admission rejects with `Execution aborted`;
cancellation after admission leaves the settled result unchanged. The cooperative
120,000 ms deadline returns an unsuccessful result with
`Execution timeout (120s)`, an empty stack and earlier console/files. It is
separate from the bridge request's 30,000 ms deadline. Neither deadline can
interrupt JavaScript blocking the page's event loop; no worker or hard
interruption is provided.

The first terminal outcome owns the ordered end-hook pass and independent
resource-release attempts. Setup failures, containing-adapter removal and dropped
pending futures use this same owner. A failed end hook cannot skip later hooks;
an earlier operation failure wins over cleanup failures. Explicit fallible
`remove` retains failed native detach registrations for retry. The host's hidden
parent is removed on settlement or removal; caller-supplied loading parents stay
caller-owned. Keep the adapter alive while awaiting its execution future.

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

The [browser test command](../development.md#browser-integration-qualification)
fetches the checkout's locked dependencies before offline scratch preparation.
An initially empty Cargo home is supported when the configured dependency sources
are reachable. The command installs no browser tools.

Native tests exercise document envelopes, ordered result collection and shared routing. The browser-executed
loading witnesses cover native listener removal, current-window admission,
ready/error settlement, preparation/runtime values, literal validation documents
and controlled host failures in Chromium. Execution witnesses additionally cover
real hidden frames, raw return/file identity, tool formatting, callback flushing,
cooperative cancellation, captured production-deadline policy and cleanup retries.
The deadline witness invokes the captured production callback; it does not claim
elapsed-wall-clock qualification. The tabs capability is controlled in
those tests, not an extension-platform certification. Broader browser/device
qualification remains tracked in [browser qualification](https://github.com/Orchestration-Maestro/maestro/issues/825)
and [surface qualification](https://github.com/Orchestration-Maestro/maestro/issues/1084).
