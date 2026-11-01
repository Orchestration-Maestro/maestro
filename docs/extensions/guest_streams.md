# Model invocation options

The guest library re-exports `ThinkingBudgets`, `CacheRetention`, `Transport` and
`ProviderResponse` from the shared request owner. `StreamOptions`,
`ProviderStreamOptions` and `SimpleStreamOptions` specialize the same shared
records with guest-local capabilities; they do not copy native option definitions.
`StreamOptionsInput` specializes the shared raw/simple enum with those same
capabilities. Its variants preserve the supplied option form, and `common()`
borrows common controls without converting the raw fields or simple policies.

Common options include temperature, token limits, ordered headers, request metadata,
authentication, session identity, transport, cache policy and retry settings.
Absent options remain absent. Finite numeric values retain signed zero; the guest's
JSON encoder rejects authored Infinity and NaN, including thinking budgets.
Serialization carries only data, never a signal, hook, fetch operation or opaque object.

`OnPayload` receives owned `PayloadHookInput` and a retained model. Its future
returns the selected payload and signal. Returning the input keeps them; returning
null, false or zero selects that value rather than restoring an earlier payload.
`OnResponse` receives copied status and headers with the retained model.
The records alone do not invoke these operations. Model invocation forwards them to
the native provider, which decides when to await each hook.

`ProviderObjects` retains an arbitrary local Rust value without serialization.
Cloning shares its owner. It does not turn that value into a native HTTP client.
The guest fetch and simple tool-choice slots are uninhabited and expose no operation.
Native callers keep their existing fetch, object and tool-choice implementations.

The guest's agent-selection `ThinkingLevel` includes disabled reasoning. Simple
request options instead use `RequestedThinkingLevel`, which excludes that disabled
level. Both reuse the shared request types. `Context` is re-exported for owned model requests.

These records do not provide a stream executor, event buffer or provider registry.
Model dispatch, stream handles and host hook forwarding are separate capabilities.

## Authored event streams

`EventStream<T, R = T>` forwards to the native model library's event buffer. Events
and results can contain local closures or other non-serializable, non-Clone values.
The component retains each value under a typed resource; it does not encode it as
JSON or maintain a guest-side queue.

`EventStream::new(is_complete, extract_result)` creates a component stream. The
classifier and extractor run synchronously before `push` returns, outside callback
storage borrows. Classifier reentry can admit an event before terminal reservation;
extractor reentry cannot admit another event after that reservation.

`next()` readers compete for events in registration order. Ended streams drain
queued events before EOF. The first terminal event is delivered once and settles
`result()` independently of iteration; later events do not run callbacks. Repeated
result observations share the same `Rc<R>`, without requiring `R: Clone`.

`end(None)` stops iteration but does not invent a result. A later `end(Some(value))`
can settle that pending observation, including when `value` is JSON null, false or
zero. Once published, the first result is not replaced.

Dropping a result-observation future does not cancel an independently owned producer
or another observation. A result alias remains usable after the stream is dropped;
the result is released after its last alias. The producer owns cancellation policy:
an authored producer can subscribe to the caller's signal and end its stream explicitly.
Final host-owned value releases wait for Store quiescence. Their destructors run
outside callback-storage borrows and may create another stream.

Admission forwards each accepted event once, without rescanning its history. The
classifier runs once per accepted event and the extractor once for the first terminal
event. The compiled-guest checks cover 64, 128 and 256 events, with a deliberate
history-rescan control to distinguish quadratic work.

Alternate adapters implement `EventStreamPort<T, R>` and use `from_port`. Native
model-library callers retain the default threaded callbacks; its `Local` execution
family accepts local captures through `EventStream::<T, R, Local>::new_local`.

## Assistant event streams

`AssistantMessageEventStream::new()` and `create_assistant_message_event_stream()`
create the assistant specialization on that same native FIFO. Native and guest
streams use the shared event record's `is_terminal()` and `message()` projections;
neither copies a terminal classifier or reconstructs message content from deltas.

`push`, `end`, `next` and `result` retain original shared assistant messages. Partial
events still see edits through their shared message handle after admission. Done
reasons stop, length and tool use select the supplied final message; error and
aborted events select the supplied error message. Repeated `result()` calls return
the same shared message owner without draining queued events. All supplied content,
usage, costs, tool arguments, diagnostics and optional fields remain intact.

## Invoking registered models

`stream(model, context, options)` invokes the registered raw adapter immediately.
`stream_simple` selects its simple adapter instead. Both accept an owned descriptor
without requiring catalog membership. Absent options stay absent; supplied empty
options stay supplied. Raw open fields and simple reasoning/budgets remain distinct,
even when their names resemble common timeout or retry settings.

`complete` and `complete_simple` start invocation when called, before their returned
futures are polled. They observe the native final result without draining events.
Stream `result()` observations share one decoded final-message owner, including
when they overlap, and remain independent of `next()` readers.

Unknown APIs fail with `No API provider registered for api: {api}`. Stream setup
failures return from the call; completion reports them through its returned future.
A provider that has already started retains its own terminal-error event protocol.
Authored nonfinite option fields fail before native dispatch.

Payload and response hooks run through their original component callback identities.
The native producer awaits their results. Payload selection preserves every JSON
kind, including null, false, zero and empty strings, rather than restoring the
previous payload. Returning a signal keeps cancellation; returning a different
signal replaces it, and returning no signal removes it. Opaque author controls
stay retained values, not native HTTP clients.

The streaming chat adapter waits for payload selection before request dispatch.
A hook can edit, replace, add or delete payload fields; the selected JSON reaches
the HTTP peer. Its response hook finishes before the first body poll or start event.
While either hook is pending, native event and result observations remain pending.
Cancellation raised during the payload hook still controls that same invocation.
A rejected hook ends the already-started provider stream with one terminal error
and EOF, preserving the supplied error text. It does not become a setup rejection.
A response-hook failure releases the native body without polling it.

The content-generation adapter consumes the signal selected by the payload hook.
Preserving or replacing it lets only that signal cancel a pending transport;
removing it leaves the transport running even if the original signal is aborted.
The provider still uses the original signal for its final message policy, so a
completed body can produce an aborted result after the original signal was aborted.
Signal relays use the existing synchronous callback continuation, including when
preserving a signal causes an abort listener to invoke another listener in the same guest.

Invoked model events use the existing shared-record JSON codec. A decoded event
is an observation, not a live cross-component object reference; later native
partial-message edits do not modify an earlier decoded event. Authored streams
created inside the component still retain their original local message handles.

## Providers called by native code

Set `ProviderConfig.callbacks.stream_simple` to a local `StreamFunction<StreamOptionsInput>`
and register it through the extension API or a retained model registry. Native raw
and simple calls retain their original option variants. Native hooks stay callable
and awaited in the guest, including their selected payload and supplied failures.
Native opaque controls stay retained; they do not become guest HTTP clients.

A registered guest provider can forward those original native hooks to a bundled
protocol. The selected request reaches the actual HTTP peer, with payload selection
before dispatch and response completion before the first body poll. Hook failures
retain the same terminal protocol in both callback directions.

The SDK agent loop consumes the registered guest stream through its normal stream
function. Start, delta and completion observations keep their original order.
Completed agent messages share the native final-message owner with independent,
repeated result observations.

The native consumer fronts the original authored stream. There is no forwarding
queue or task that drains its events. Repeated final observations share one decoded
native message, also used by the terminal event. Dropping an unassigned pending read
removes only that waiter, as on a native stream; it does not stop the producer.

A Store or codec failure yields one native terminal error with the original error
text, the same final message and EOF. A final message already decoded by the native
consumer wins over a later failure. Unread values in a crashed Store are lost.

Local native hooks require the extension's original pump thread. Calls from another
thread must use the Threaded family; an off-thread Local call returns an affinity
error. These routes use the existing provider registry, not a second registry.

Returned model streams retain their original hooks and arbitrary caller signal
storage after the initiating command and registration are released. Cancellation
subscriptions keep that caller storage alive until they are removed, including
for caller-defined signal ports. Producer completion releases its own captures;
final stream aliases do not cancel the producer. Capture destructors may reenter
stream creation outside callback-storage borrows, and final containing-owner
release still shuts down the component normally.

Overlapping invocations keep separate hook and signal scopes. Dropping every
observer and stream alias leaves the native producer running; finishing one
producer cannot release another invocation's uncalled hooks. Explicit `push`
and `end` on a returned stream use its original native buffer. Ending that
buffer does not cancel its independently owned producer.

Hook phases remain protocol-owned. Public and cloud content generation do not
invoke a response hook. Response-session retries invoke the original response
callback on each native attempt. If that callback rejects, its attempt releases
the body without polling it; a later native retry consumes a new response.

Opaque author values can hold non-Clone closures. Cloned raw options share their
original owner; its destructor runs once when the invocation releases its final
retained value. Ordinary JSON still omits that value.
