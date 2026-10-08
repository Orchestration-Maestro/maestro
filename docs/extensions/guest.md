# Writing an extension as a component

`maestro-extensions-wasm` is the Rust library an extension author links to. It
owns the canonical interface files in `crates/maestro-extensions-wasm/wit/`
(package `maestro:extension`) and a facade over the generated bindings, so an
extension is ordinary async Rust that the host runs as a WebAssembly component.
The facade has no internal dependencies.

## Build a component

```sh
mise exec -- just extension-author-component
```

The recipe adds the `wasm32-wasip2` target and builds the example in
`crates/maestro-extensions-wasm/examples/author_component.rs` as a component.
An extension crate is a `cdylib` whose root names its factory:

```rust
maestro_extensions_wasm::export_extension!(factory);

fn factory(api: maestro_extensions_wasm::ExtensionAPI)
    -> maestro_extensions_wasm::ExtensionFuture<'static, ()>
{
    Box::pin(async move { api.on("input", handler) })
}
```

The macro defines the exported entry point; the crate needs no raw bindings. A
native build of the example links an empty library, so only the `wasm32` build
produces a component.

## Register

A factory may finish immediately or after awaiting. The host observes the
registrations only after the factory's future completes, and a failed factory
stays failed. Registrations are individual and ordered: `on` forwards every
handler under its open event name (names the host does not define are forwarded
unchanged, and two handlers for one name are both kept), and `register_tool`,
`register_command`, `register_shortcut`, `register_flag` and
`register_message_renderer` forward one registration each. A rejected
registration returns the host's message and retains nothing. Flags keep absence,
`false` and the empty string distinct, and the value on the command line wins
over a default.

Each closure you register stays alive in the guest until the host releases it,
exactly once, outside any registry borrow. A destructor may register again while
its own entry is released.

## Events and results

There is one `ExtensionEvent` enum for the whole event set and one
`ExtensionEventResult`. A handler receives `&mut ExtensionEvent`; an edit
(a rewritten tool-call input, a transformed input) is carried back to the host
even when the handler then returns an error. An absent result is distinct from a
present result whose fields are `None`, `false`, zero or empty. Tool-result and
tool-call events are typed for the seven built-in tools and carry custom tools
as JSON. Custom JSON keeps member order and text survives without
normalization.

Each event has its own asynchronous export in the interface; the facade routes
every export to the handlers registered for that name.

## Tools

`ToolDefinition` pairs generated metadata with two closures. `prepare_arguments`
is synchronous and may emit bus data through its `CallbackEmitter`; `execute`
receives the call identifier, the prepared parameters, an optional
`AbortSignal`, an optional progress callback and an `ExtensionContext`.
Progress callbacks send partial results while the call runs. Text, image,
signature, details and `terminate` values are preserved, and an `Err` from either
closure reaches the host unchanged. The `is_*_tool_result` helpers and `is_tool_call_event_type` compare
names only.

## Contexts and signals

Handlers, tools and shortcuts run with an `ExtensionContext`; commands run with
an `ExtensionCommandContext`, and the continuation passed to a session operation
runs with a `ReplacedSessionContext`. Only the command context can wait for
idle, start, fork, navigate, switch or reload a session; the ordinary context
has no such methods, which the rustdoc examples prove at compile time. A getter
reads the host's current state each time. Errors are plain strings the author can
catch; after a session is replaced, the old context reports the stale message
while contexts of other operations keep working.

An `AbortSignal` is a capability, not a snapshot: keep it past the callback and
read `aborted()` later. The reader, catalog, interface and signal resources you
retain are not guarded by the context that produced them.

New-session, fork and switch accept a `setup` writer and a continuation. The
setup runs first, the continuation receives the replacement context of its own
operation even when operations overlap, a cancelled result runs no continuation,
and the continuation is released once the operation ends.

## Session, catalog and the bus

`ReadonlySessionManager` forwards every query and returns the session tree with
its labels and timestamps. `SessionManager`, available only to a setup closure,
adds the append, branch and label operations. `ModelRegistry` offers the four
catalog queries; authentication failures are values, and a failed capability call
is a separate error. `EventBus::on` takes a listener with a synchronous prefix
and an optional asynchronous tail; keep the returned `Subscription` and call
`unsubscribe` to stop it. Dropping it does not unsubscribe.

## Renderers

A message renderer returns `None` for no component, which differs from an empty
one, or a `Component` with `render`, `handle_input`, `wants_key_release` and
`invalidate`. A retained component keeps its captured state after the renderer
returns.

## Errors

Every host call returns `Result<_, String>`. The controlled adapter used in
tests attributes a failed call to the extension and keeps registrations,
subscriptions and unrelated signals usable.

## Example

`crates/maestro-extensions-wasm/src/tests/compaction_example.rs` registers a
before-compact handler that reads the session and the catalog through its context
and returns the user requests (each cut to 100 characters, `[complex]` for
non-text content) as the summary. `tests/author_events.rs` runs it through the
controlled adapter. The example built by the recipe above (`src/tests/author.rs`)
registers an input handler, a tool-call editor, a tool, a command and a
before-compact handler; `src/tests.rs` runs it through the built component and
through the controlled adapter and compares the two transcripts.
