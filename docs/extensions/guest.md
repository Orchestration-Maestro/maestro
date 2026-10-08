# Writing an extension as a component

`maestro-extensions-wasm` is the Rust library an extension author links to. It owns the
canonical interface files in `crates/maestro-extensions-wasm/wit/` (package
`maestro:extension`) and a facade over the generated bindings, so an extension is ordinary
async Rust that the host runs as a WebAssembly component. The facade has no internal
dependencies. This page describes what is delivered so far: registration, the input and
before-compact events, command contexts and session continuations.

## Build a component

```sh
mise exec -- just extension-author-component
```

The recipe adds the `wasm32-wasip2` target and builds the example in
`crates/maestro-extensions-wasm/examples/author_component.rs` as a component. An extension
crate is a `cdylib` whose root names its factory:

```rust
maestro_extensions_wasm::export_extension!(factory);

fn factory(api: maestro_extensions_wasm::ExtensionAPI)
    -> maestro_extensions_wasm::ExtensionFuture<'static, ()>
{
    Box::pin(async move { api.on("input", handler) })
}
```

The macro defines the exported entry point; the crate needs no raw bindings. A native build
of the example links an empty library, so only the `wasm32` build produces a component.

## Register

A factory may finish immediately or after awaiting; the host observes the registrations once
the factory's future completes. `on` forwards each handler under its event name, including
names the host does not define, and `register_command` forwards a command. A rejected
registration returns the host's message and retains nothing.

Each closure you register stays alive in the guest until the host releases it, exactly once,
and never while the guest's closure table is borrowed. A destructor may register again while
its own closure is released.

## Events and results

A handler receives `&mut ExtensionEvent` and an `ExtensionContext` and answers with
`Option<ExtensionEventResult>`; an absent result differs from a result whose fields are
`None`. Two events exist so far:

- `ExtensionEvent::Input` carries the text, optional images and source. A handler may answer
  `InputEventResult::Transform`, `Handled` or `Continue`.
- `ExtensionEvent::Session(SessionEvent::BeforeCompact)` carries the compaction preparation,
  custom instructions and an `AbortSignal`. A handler may cancel the compaction or supply a
  `CompactionResult`.

An `AbortSignal` is a capability, not a snapshot: keep it past the callback and read
`aborted()` later.

## Contexts and sessions

Handlers run with an `ExtensionContext`, which reads the working directory. Commands run with
an `ExtensionCommandContext`, which adds `wait_for_idle` and `new_session`; the ordinary
context has neither method, which the rustdoc examples check at compile time.

`new_session` takes a `with_session` continuation. The host runs it against a
`ReplacedSessionContext` bound to the replacement session while the operation is pending, and
the guest releases the continuation when the operation ends. Afterwards the context that
started the operation reports the host's stale-context message from `cwd`, while the
replacement context keeps working. Errors are the plain text the host supplies, so an author
can catch one and continue.

## Example

`crates/maestro-extensions-wasm/src/tests/author.rs` registers an input handler that
upper-cases text, a before-compact handler that cancels an aborted compaction and reports the
state of the signal it kept from the previous one, a command that starts a new session and
waits for idle from its continuation, a handler the host rejects, and a handler whose
destructor registers another. `src/tests.rs` runs it through the built component and through
the controlled adapter and compares the two transcripts.
