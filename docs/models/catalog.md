# Offline model catalog

`get_providers` and `get_models` enumerate the embedded providers and descriptors
in their recorded insertion order. `get_model` matches provider and model keys
exactly, without trimming or case conversion, and returns `None` for missing
keys. An unknown provider has an empty model list. Catalog access is offline:
it performs no credential lookup, protocol registration or provider call.

Every returned descriptor and list is independently owned. Editing a descriptor,
including its ID, provider, headers or compatibility fields, never changes the
embedded catalog or another caller's values. The original registry keys remain
unchanged. Helpers also accept descriptors that do not belong to the catalog.

`calculate_cost` updates the supplied usage's input, output, cache-read and
cache-write costs: each category uses its own USD-per-million rate divided by
1,000,000, multiplied by its reported count. Total cost is their left-to-right
sum. It returns a mutable borrow of that cost record without changing any token
count, rounding, clamping or rejecting negative/nonfinite arithmetic.

`get_supported_thinking_levels` returns off for nonreasoning models. Reasoning
models use off/minimal/low/medium/high/xhigh order: explicit null disables a
level, missing ordinary mappings retain it, and xhigh needs an explicit text
mapping (empty text counts). `clamp_thinking_level` keeps a supported request;
otherwise it prefers the next higher available level before the nearest lower
one, falling back to off when none is available.

`models_are_equal` compares only provider and ID. Missing operands are unequal;
all unrelated metadata is ignored. These infallible helpers require neither
catalog membership nor a registered protocol adapter.

## Generated data layout

Generated descriptors use `<provider>/<family>.rs`. The family is the vendor
segment for qualified IDs or the leading model-family segment (before a hyphen,
dot or digit) for unqualified IDs. Account/model/router, `workers-ai/@cf` and
regional inference-profile prefixes are stripped first; unqualified `labs-` and
`open-` prefixes are also stripped. Names use lowercase Rust snake casing.
Each family retains recorded order and is greedily partitioned only when its
formatted file would exceed 500 lines, using `_1`, `_2`, … suffixes for all of its
partitions. Provider assembly preserves original registry order, including
interleaved families; filenames never determine enumeration order.

## Controlled catalog example

```rust
use maestro_models::{
    ModelThinkingLevel, Usage, UsageCost, calculate_cost, clamp_thinking_level,
    get_model, get_models, get_providers, get_supported_thinking_levels, models_are_equal,
};

let mut model = get_model("google", "gemini-2.5-flash").unwrap();
assert!(get_providers().contains(&model.provider));
assert!(get_models("google").iter().any(|entry| models_are_equal(Some(entry), Some(&model))));
assert!(get_supported_thinking_levels(&model).contains(&ModelThinkingLevel::High));
assert_eq!(clamp_thinking_level(&model, ModelThinkingLevel::Xhigh), ModelThinkingLevel::High);

model.name = "My descriptor".into();
assert_ne!(get_model("google", "gemini-2.5-flash").unwrap().name, model.name);
let mut usage = Usage {
    input: 100.0, output: 20.0, cache_read: 0.0, cache_write: 0.0,
    total_tokens: 120.0,
    cost: UsageCost { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0, total: 0.0 },
};
let estimate = calculate_cost(&model, &mut usage);
assert!(estimate.total > 0.0);
assert_eq!(usage.total_tokens, 120.0);
```
