# Model options

`stream` forwards `ProviderStreamOptions` to the registered raw callback; `stream_simple` forwards `SimpleStreamOptions` to its separate simple callback. Absent options remain absent. `StreamOptions` holds the common fields, including api key, signal, temperature, max tokens, transport, cache retention, session ID, headers, callback hooks, retry fields and metadata. Raw options also retain provider-specific extras; simple options retain reasoning and thinking budgets.

Invocation dispatch does not choose defaults, clamp limits, resolve authentication, merge headers or infer provider capabilities. The descriptor is supplied data. Signals are forwarded unchanged; cancelling a signal does not itself close a producer-owned stream. Simple provider adapters call `build_base_options` when they need common simple defaults; the raw path does not call this helper automatically.

`build_base_options` forwards all common fields. An omitted max-token value defaults to the smaller of a positive descriptor limit and 32000; a nonpositive limit leaves it absent. Explicit numeric values are preserved. A nonempty resolved key wins over the supplied option key; keys are not trimmed. `clamp_reasoning` maps Xhigh to High and preserves the other levels or absence.

`adjust_max_tokens_for_thinking` uses minimal 1024, low 2048, medium 8192 and high 16384 unless individually overridden; Xhigh uses High. It caps base plus thinking at the supplied model limit. Only when that total is no greater than thinking does it reduce thinking to the greater of zero and total minus 1024. It does not guarantee 1024 output tokens otherwise, validate limits or round numbers to integers.

`is_context_overflow` checks nonempty error text against exclusions before positive patterns. Successful Stop results overflow when input plus cached input exceeds a supplied truthy context window. Length results with zero output overflow when that sum reaches 99% of the window. Other outcomes remain distinct. `get_overflow_patterns` returns a copied ordered pattern collection. The helpers do not invoke providers, read credentials, mutate messages or print diagnostics.

```rust
use maestro_models::{ThinkingLevel, adjust_max_tokens_for_thinking, clamp_reasoning};

assert_eq!(clamp_reasoning(Some(ThinkingLevel::Xhigh)), Some(ThinkingLevel::High));
let adjusted = adjust_max_tokens_for_thinking(4096.0, 8192.0, ThinkingLevel::High, None);
assert_eq!(adjusted.max_tokens, 8192.0);
assert_eq!(adjusted.thinking_budget, 7168.0);
```
