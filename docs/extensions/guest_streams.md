# Model invocation options

The guest library re-exports `ThinkingBudgets`, `CacheRetention`, `Transport` and
`ProviderResponse` from the shared request owner. `StreamOptions`,
`ProviderStreamOptions` and `SimpleStreamOptions` specialize the same shared
records with guest-local capabilities; they do not copy native option definitions.

Common options include temperature, token limits, ordered headers, request metadata,
authentication, session identity, transport, cache policy and retry settings.
Absent options remain absent. Finite numeric values retain signed zero; the guest's
JSON encoder rejects authored Infinity and NaN, including thinking budgets.
Serialization carries only data, never a signal, hook, fetch operation or opaque object.

`OnPayload` receives owned `PayloadHookInput` and a retained model. Its future
returns the selected payload and signal. Returning the input keeps them; returning
null, false or zero selects that value rather than restoring an earlier payload.
`OnResponse` receives copied status and headers with the retained model.
These callback types describe owned operations; the records alone do not invoke them.

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

Alternate adapters implement `EventStreamPort<T, R>` and use `from_port`. Native
model-library callers retain the default threaded callbacks; its `Local` execution
family accepts local captures through `EventStream::<T, R, Local>::new_local`.
These operations do not yet provide model invocation, assistant-stream forwarding
or awaited provider hooks.
