# Model option helpers

`maestro-models` exports four pure helpers without catalog lookup, environment
credential resolution, provider invocation or hook execution.

- `build_base_options` copies the 14 common invocation fields, retaining callback
  and cancellation handles. A missing token limit uses the positive model limit
  capped at 32,000; nonpositive model limits leave it absent. Explicit limits,
  including zero or negative values, remain unchanged. A separately supplied
  nonempty key overrides the option key; empty or absent keys fall back without
  trimming. Reasoning settings and thinking budgets are not common output fields.
- `clamp_reasoning` maps extra-high to high, retaining every other level and absence.
- `adjust_max_tokens_for_thinking` selects budgets of 1,024 (minimal), 2,048 (low),
  8,192 (medium) or 16,384 (high and extra-high). Custom slots override individually,
  including zero. `AdjustedMaxTokens` carries the combined `max_tokens` and
  `thinking_budget`: add the budget to the base and cap at the model limit. Only
  when that limit is no larger than the budget, reduce the budget to leave 1,024
  output tokens, never below zero. There is no unconditional output reserve,
  rounding or input validation.
- `is_context_overflow` classifies a borrowed assistant response as described below.

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
let adjusted = adjust_max_tokens_for_thinking(16.0, 10_000.0, ThinkingLevel::Low, None);
assert_eq!(adjusted.max_tokens, 2_064.0);
assert_eq!(adjusted.thinking_budget, 2_048.0);
```
