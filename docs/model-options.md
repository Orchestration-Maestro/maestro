# Model option helpers

`maestro-models` exports pure helpers without catalog lookup, environment
credential resolution, provider invocation or hook execution.

- `build_base_options` copies the 14 common invocation fields, retaining callback
  and cancellation handles. A missing token limit uses the positive model limit;
  nonpositive model limits leave it absent. Every limit, supplied or defaulted, is
  then fitted to the room the model's context window leaves: the window less the
  estimated tokens of the system prompt, tools and messages and 4,096 safety tokens,
  never below one token. A limit already inside that room, such as an explicit 2,000,
  is kept; a model with no window keeps its limit, raised to one token when it is
  lower. A separately supplied nonempty key overrides the option key; empty or absent
  keys fall back without trimming. Reasoning settings and thinking budgets are not common output fields.
  `build_base_options_for` performs the same preparation for an explicit execution
  family, sharing its original payload and response hooks without invoking them.
  `build_base_options` keeps the default thread-safe SDK signature.
- `reasoning_from_thinking_level` borrows an already-selected model thinking level:
  Off becomes absent; minimal through extra-high retain their level. Model support
  clamping remains with the caller.
- `clamp_reasoning` maps extra-high to high, retaining every other level and absence.
- `adjust_max_tokens_for_thinking` selects budgets of 1,024 (minimal), 2,048 (low),
  8,192 (medium) or 16,384 (high and extra-high). Custom slots override individually,
  including zero. `AdjustedMaxTokens` carries the combined `max_tokens` and
  `thinking_budget`: add the budget to the base and cap at the model limit; without
  a base the combined limit is the model limit, and the budget fits inside it. Only
  when that limit is no larger than the budget, reduce the budget to leave 1,024
  output tokens, never below zero. There is no unconditional output reserve,
  rounding or input validation. A NaN sum or model limit produces a NaN combined limit.
- `is_context_overflow` classifies a borrowed assistant response as described below.
- `utf16_units`, `tokens_for_chars` and `ESTIMATED_IMAGE_CHARS` are the parts of the
  token estimate: four UTF-16 units per token, rounded up, and 4,800 units per image.
  The output limit above and session compaction both count with them.

## Cache retention

Provider request builders share the options owner's retention resolution: an explicit
`None`, `Short` or `Long` wins; otherwise only the exact environment value
`MAESTRO_CACHE_RETENTION=long` selects long retention. An absent, empty, differently
cased or whitespace-padded value selects short. Each provider keeps its own markers,
affinity and compatibility policy.

## Request timeouts

A session request without its own `timeout_ms` takes the provider retry setting, then
the `httpIdleTimeoutMs` setting, so every provider that reads the timeout honours it;
see the [SDK guide](application/sdk.md). The Mistral transport applies the same timeout
to the wait for each stream chunk, and rejects a negative or fractional value as the
other providers do; see [reasoning conversations](models/reasoning-conversations.md).

## Three context-overflow signals

1. An error stop with a recognized nonempty diagnostic, such as
   `prompt is too long`, `request_too_large`, `exceeds the context window`,
   `maximum prompt length is 131072`, `context_length_exceeded`,
   `prompt too long; exceeded max context length` or `400 (no body)`.
   Anchored throttling and service-unavailable prefixes, rate limits and
   too-many-requests diagnostics take precedence over token phrases.
2. A successful stop with input plus cached input strictly exceeding a supplied
   nonzero context window.
3. A length stop with zero output and input plus cached input filling at least
   99% of that window. This detects servers that truncate oversized input to fill
   the context and leave no output space.

Cache writes, total tokens and cost do not count toward context input. No window
or a zero window disables the two usage-based signals, not diagnostic detection.
Text matching is ASCII-case-insensitive; numeric positions accept ASCII digits.

## Detection limits

Many providers reliably return recognizable diagnostics. Others sometimes accept
oversized input silently, report a rate limit instead, or truncate without a
measurable overflow signal. Successful excess usage and filled zero-output length
stops are detectable only when a window is supplied. Silent truncation without
these signals cannot be inferred because the expected input token count is unknown.

Custom providers may use unfamiliar diagnostics. Inspect a controlled oversized
request's error message, then either check it separately or contribute a matching
rule to the classifier. These helpers never send such a request themselves.

## Controlled example

```rust
use maestro_models::{ThinkingLevel, adjust_max_tokens_for_thinking, clamp_reasoning};

assert_eq!(clamp_reasoning(Some(ThinkingLevel::Xhigh)), Some(ThinkingLevel::High));
let adjusted = adjust_max_tokens_for_thinking(Some(16.0), 10_000.0, ThinkingLevel::Low, None);
assert_eq!(adjusted.max_tokens, 2_064.0);
assert_eq!(adjusted.thinking_budget, 2_048.0);
```
