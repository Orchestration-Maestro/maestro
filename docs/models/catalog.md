# Offline model catalog

Maestro includes 970 model descriptors from 31 providers. The catalog is checked-in data; lookup and enumeration do not access the network or refresh it.

get_providers and get_models preserve catalog order. get_model returns a shared descriptor or None; an unknown provider has no models. Returned descriptor handles share changes, while returned lists are independent. Lookup keys remain the original provider and model identifiers.

calculate_cost updates usage.cost in place using USD-per-million input, output, cache-read and cache-write rates. It preserves supplied numeric values, including negative rates and nonfinite arithmetic, and leaves token counts unchanged. models_are_equal compares only provider and id and returns false when either operand is absent.

Nonreasoning models support only off. Reasoning models start with off, minimal, low, medium and high; an explicit null mapping removes a level, and xhigh requires a present non-null mapping. clamp_thinking_level accepts those six exact spellings, searches upward before downward when a requested level is unavailable, and uses the first available level for an unknown spelling. Empty availability falls back to off. Inputs are not trimmed or case-folded.

These operations return values without console output or a lookup error. They use the existing Model, Usage and ModelThinkingLevel records. Runtime refresh, provider invocation and local catalog overrides are separate concerns.

```rust
use maestro_models::{get_model, get_models, get_providers};

assert_eq!(get_providers().len(), 31);
assert!(get_models("unknown-provider").is_empty());
let model = get_model("openai", "gpt-4o-mini").expect("catalog model");
let descriptor = model.read().unwrap_or_else(|poisoned| poisoned.into_inner());
assert_eq!(descriptor.id, "gpt-4o-mini");
```
