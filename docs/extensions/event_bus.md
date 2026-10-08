# Shared event bus

Shared event bus for communication between extensions:

```rust
let subscription = events.on("saved", listener);
events.emit("saved", serde_json::json!({"path": "notes.md"})).await;
```

`maestro_extensions::create_event_bus(spawn, report)` creates an empty controller.
`for_extension()` supplies an independently identified handle on the same bus;
cloning a handle retains its identity. Channels are literal strings, including
empty, Unicode and reserved-looking names. JSON payloads retain their values and
existing object-key order; listeners share an immutable payload.

Ordinary `emit` runs listener prefixes in registration order and returns a ready
future without awaiting their optional asynchronous tails. Each emission keeps
its own listener snapshot: subscription edits affect subsequent emissions, not
listeners already captured for the current one. Nested emissions take fresh
snapshots. There is no listener limit or threshold warning.

A listener receives a borrowed `CallbackEmitter` for its synchronous prefix.
`invoke_callback` supplies the same capability to other synchronous callbacks:
same-extension listeners run immediately; foreign listeners run after that
individual callback returns, before its caller continues. Nested callbacks have
independent queues. A returned error value does not discard queued deliveries.
The borrowed emitter cannot escape into a tail; `events()` supplies an owned
ordinary emitter retaining the callback's extension identity instead.

The caller supplies a serial cooperative executor through `TailSpawner`.
Enqueue tails on the same execution context as the emitting code. Do not poll
inline or concurrently with its current synchronous segment. Genuine suspension
must release queued tails, including those that resume the awaiting emitter;
waiting for the whole root future to finish can deadlock. A Tokio current-thread
runtime can satisfy this contract; raw multithreaded spawning alone cannot.
The bus creates no runtime and does not enforce this executor obligation.

`subscription.unsubscribe()` removes only its registration, idempotently.
Dropping a subscription handle does not unsubscribe. Controller `clear()` removes
all channels and leaves the bus reusable. Neither operation cancels a started
tail: its captured data, origin and cancellation signals live until it settles.

Listener prefix and tail failures go to the supplied reporter as one complete
line: `Event handler error (<channel>): <cause>\n`. Native error `Display` supplies
the cause, including any embedded newlines. Healthy listeners continue. The bus
neither prints directly nor catches panics. An instance adapter owns trap state:
it must return the attributed failure on later calls without re-entering the
failed instance. The bus does not detach that instance's registrations or UI.

The crate's runnable rustdoc example demonstrates construction, subscription,
emission and explicit unsubscribe. Installed UI resources and component execution
are not provided by this event-channel library.
