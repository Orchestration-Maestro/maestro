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
