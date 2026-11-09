# Sandbox execution, loading and runtime messaging

The browser sandbox modules provide `RuntimeMessageRouter`, `MessageConsumer`
and `RuntimeMessageBridge`. The crate root exports `RuntimeMessageBridge` and,
on WebAssembly, the browser-local `RUNTIME_MESSAGE_ROUTER` singleton.

## Quick start

A provider names the helpers its sandbox needs. Maestro's own helpers come from the
runtime bundle, a Rust program built for the browser; your content stays plain
JavaScript that the frame runs as input.

```rust
use maestro_app::{BuiltinRuntime, DiagnosticErrorInfo, Runtime};

// Inside a `SandboxRuntimeProvider` implementation:
fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
    // Installs `listAttachments`, `readTextAttachment` and `readBinaryAttachment`.
    Ok(Runtime::Builtin(BuiltinRuntime::Attachments))
}
```

Load the provider through `SandboxIframe::load_content`, or prepare a document without
mounting it with `prepare_html_document`. Then pin the frame's script policy:

1. Add `'wasm-unsafe-eval'` to the frame's `script-src`. It lets the runtime compile its
   own module. It does not permit `eval` or `Function`.
2. Pin the SHA-256 of `maestro_web::components::sandbox::runtime_bundle::BUNDLE`.
3. Pin each supplied provider and program script as before (see
   [Runtime bundle and script policy](#runtime-bundle-and-script-policy)).

The [browser integration command](../development.md#browser-integration-qualification)
builds and runs the existing execution and loading assertions with supplied
Chromium tools. It documents the selected exports, expected policy error and
cleanup behavior.

## Provider runtime selection

`SandboxRuntimeProvider::get_runtime` returns one `Runtime` entry:

| Entry | Meaning |
| --- | --- |
| `Builtin(BuiltinRuntime::Artifacts)` | Async `listArtifacts`, `getArtifact`, `createOrUpdateArtifact`, `deleteArtifact`. |
| `Builtin(BuiltinRuntime::Attachments)` | Synchronous `listAttachments`, `readTextAttachment`, `readBinaryAttachment`. |
| `Builtin(BuiltinRuntime::Console)` | Console forwarding, error capture, `complete(error, returnValue)`. |
| `Builtin(BuiltinRuntime::FileDownload)` | `returnDownloadableFile(fileName, content, mimeType)`. |
| `Content(String)` | Your function source. It receives the sandbox identifier. |
| `Empty` | Nothing to install. |

Preparation collects every provider's data in entry order, then every runtime entry in
entry order, then serializes the merged globals with native browser operations. Later
values replace earlier keys. In the frame the runtime installs, in order: the sandbox
identifier, the merged globals, the bridge and `onCompleted` (online documents), each
runtime entry, navigation interception (online documents) and last the program. Repeated
entries run repeatedly, and later helpers overwrite earlier ones. Loading and execution
prepend a fresh console provider. Standalone documents omit the bridge and navigation
but keep every selected helper.

## Runtime bundle and script policy

The runtime bundle is one classic script built by `just sandbox-runtime` with
Emscripten 6.0.10 from `crates/maestro-web/runtime`. The generated file is
`crates/maestro-web/assets/maestro-sandbox-runtime.js`; `just sandbox-runtime --check`
rebuilds it and fails when the checked-in bytes differ, when the linker map names a library
outside the audited list, or when the text is not safe inside an inline script. The audited
list is the Rust crate object, `libc.a`, `libdlmalloc.a`, `libclang_rt.builtins*.a` and the
compiler builtins. The browser integration command runs the same check.

A prepared document carries, at the runtime insertion point and in this order:

1. one inert `<template data-maestro-sandbox-provider>` per supplied provider;
2. one inert `<script type="application/json" data-maestro-sandbox>` configuration block
   (`<` is written as `\u003c`);
3. the bundle as a parser-blocking inline classic script.

The bundle instantiates its module synchronously, so its start function runs before the
parser reaches the next element. It reads the configuration, installs everything in the
order above, moves each supplied provider's script into the head so it runs at once, then
removes the configuration block, the provider carriers and its own script element.

Hosts pin these digests over decoded text content, not encoded markup:

| Script | Digest source |
| --- | --- |
| Runtime bundle | `BUNDLE` |
| Each supplied provider | The compiled provider text (`lowering::provider`) |
| REPL program module | The compiled program text (`lowering::program`) |
| HTML artifact completion | `window.__maestroComplete` |

The only addition to the frame's policy is `'wasm-unsafe-eval'`. The change adds no
same-origin permission, `unsafe-eval`, wildcard origin, `blob:` or `data:` source. Hash
policies still refuse unmatched scripts. A provider the policy refuses never runs, so
installation stops there and the runtime reports `TypeError: provider did not complete`.
A frame without `'wasm-unsafe-eval'` installs
nothing: the module fails to compile and the carriers stay inert.

The generator wraps the bundle in a function, so its Emscripten and binding names stay out
of the frame's global scope. Authored scripts may declare `Module`, `abort` or `preRun`
at the top level. A `Module` object defined before the bundle reaches the function as its
program arguments.

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

During runtime execution, `document.currentScript` is the bundle's script element. The
built-in helpers do not read it. If the policy omits a provider's digest, that provider's
inert template stays in the loaded document and its helpers are absent. The completion
signal of an HTML artifact is a script that reads `window.__maestroComplete`; the runtime
defines that property so a read calls `window.complete()` when it exists, after the whole
authored source.

Native HTML parsing accepts recovered markup. Explicit validation errors appear
as literal preformatted text, with the selected brand pack's background, text,
error, panel and monospace roles; authored sandbox content keeps its own styling.
Execution uses the same retained frame owner, but rejects validation errors instead
of painting this loading document.

## Supplied-content compiler qualification

Run `just browser-test` to exercise the Rust compiler and native capture observers
in isolated frames. Production document preparation uses the same compiler for every
supplied provider and program.

The compiler is available at
`maestro_web::components::sandboxed_iframe::lowering`:

```rust
use maestro_web::components::sandboxed_iframe::lowering;

let compiled = lowering::program("await null; return 42;")?;
let module_source = compiled.code();
let capture_property = compiled.capture();
# let _ = (module_source, capture_property);
# Ok::<(), lowering::Error>(())
```

### Reference

`program` accepts an asynchronous body. It parses an asynchronous arrow solely for
grammar admission, then lowers the body into a native inline module. Returns become
breaks to a private label. Falling through clears the saved result inside that
label, so `finally` can cancel a pending return with a break or continue. Each
finalizer saves the pending value and restores it on normal completion. A local
break can discard an inner return without replacing an outer pending return.
Native loops retain iterator-close error precedence. Function declarations retain their
function-scoped bindings; nested function and method returns stay untouched.
Generated names exclude decoded identifiers, labels and literal names in the input.

Before module execution, the fixture installs a temporary native getter at
`Compiled::capture()`. Reading it restores the original global descriptor, removes
the carrier and returns a private outcome observer. Assignments to that observer
carry native success values or errors to Rust. Returned thenables settle before
callbacks and completion. A parser SyntaxError before module entry reaches the
same observer; the browser remains the final gate for semantic early errors.

`provider` accepts one expression and emits a capture assignment, without an
invocation. The assignment carries the expression's completion: `[true, value]` when it
yields a value, `[false, error]` when it throws. A native setter restores the original
global descriptor before Rust calls the captured function with the sandbox identifier.
The function still runs synchronously while its classic script is current. Anonymous
functions keep their empty name, and provider changes to the restored property remain
visible. The runtime never touches `window.onerror`, so a provider that sets it, by
assignment or with `Object.defineProperty`, keeps its handler. Native callability checks
ignore a function's own `call` property. A provider that throws, an expression that throws
before it yields a function and a non-callable expression each stop later installation
with their native error, reported once. A carrier the browser refuses to run, such as one
with a semantic early error, never reports a completion; installation stops with
`TypeError: provider did not complete` after the browser's own report.
The capture property must be configurable when already present; failed installation
is reported rather than overwriting a protected property.

HTML carrier encoding and exact decoded-source hashing remain separate operations.
The compiler adds no eval permission, Function constructor, interpreter, same-origin
access or dynamic script-text sink. A program the parser refuses is reported through the
configuration: the runtime settles it as a `SyntaxError` outcome, as the browser would.

### Examples covered by qualification

- `try { return 1 } finally { return 2 }` returns 2.
- `for (let i = 0; i < 1; i++) { try { return 1 } finally { continue } }`
  falls through with undefined.
- Returned thenables, rejected promises, native DOM/fetch/timers and relative
  imports retain their browser behavior.
- Native object identity, signed zero, undefined and null survive outcome capture.
  A document with every built-in helper leaves the reserved capture namespace unused.
- Real tool inputs keep closing-script text, carriage returns and Unicode through
  the existing carrier. Invalid labels and duplicate lexical declarations still
  fail as SyntaxErrors.

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

Online documents install the iframe bridge from their configuration. For a world without
a prepared document, generate the runtime script with the module's typed options. The
following example is also checked in the generator's public rustdoc:

```rust
use maestro_web::RuntimeMessageBridge;
use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
use maestro_web::components::sandbox::runtime_message_bridge::{
    RuntimeMessageBridgeOptions, RuntimeMessageContext,
};

let source = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
    context: RuntimeMessageContext::SandboxIframe,
    sandbox_id: "execution",
})?;
assert!(source.starts_with("var Module={arguments:[\"frame-bridge\",\"execution\"]};"));
assert!(source.ends_with(BUNDLE));
# Ok::<(), maestro_app::DiagnosticErrorInfo>(())
```

The frame script is the runtime bundle preceded by the channel and identifier as program
arguments. The extension user-script script is the one it has always been, kept until that
world is qualified for the bundle and its `'wasm-unsafe-eval'` policy; the browser
integration command does not exercise it. Running either installs `sendRuntimeMessage` and
ordered `onCompleted` callbacks. `__completionCallbacks` is reset to a new public array,
and `onCompleted` appends to its current value, so replacing the array stays visible.
Iframe requests correlate replies by message id, listen before posting and reject after
30,000 ms; user scripts await the extension's native reply with the identifier last.
Identifiers are quoted without trimming or extra HTML escaping.

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
register them in the router's provider collection. Each returns a built-in selection;
the runtime bundle supplies console forwarding and completion flushing, attachment reads
and online/offline file returns. The host providers install no listeners or timers; the
injected document owns those browser effects. Native collection aliases stay
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
