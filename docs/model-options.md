# Supplied model options

`stream` forwards `ProviderStreamOptions` to the registered raw callback;
`stream_simple` forwards `SimpleStreamOptions` to its separate simple callback.
Absent options remain absent. `StreamOptions` holds the common fields, including
api key, signal, temperature, max tokens, transport, cache retention, session ID,
headers, callback hooks, retry fields and metadata. Raw options also retain
provider-specific extras; simple options retain reasoning and thinking budgets.

This layer does not choose defaults, clamp limits, resolve authentication, merge
headers or infer provider capabilities. The descriptor's reasoning, thinking map,
limits and costs are supplied data. Provider adapters own their interpretation.
Signals are forwarded unchanged; cancelling a signal does not itself close a
producer-owned stream. See [supplied model invocation](records.md).
