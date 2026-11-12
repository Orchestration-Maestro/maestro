# Model catalog

`get_providers` and `get_models` enumerate the embedded providers and descriptors
in their recorded insertion order. `get_model` matches provider and model keys
exactly, without trimming or case conversion, and returns `None` for missing
keys. An unknown provider has an empty model list. Catalog access is offline:
it performs no credential lookup, protocol registration or provider call.

Every returned descriptor and list is independently owned. Editing a descriptor,
including its ID, provider, headers or compatibility fields, never changes the
embedded catalog or another caller's values. The original registry keys remain
unchanged. Helpers also accept descriptors that do not belong to the catalog.

`calculate_cost` re-exports the shared request owner's arithmetic. It updates the supplied usage's input, output, cache-read and
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

The table ships as its own crate, `maestro-models-catalog`, written whole in
`crates/maestro-models-catalog/src/`. `maestro-models` keeps the lookups and
reads the crate's `models` function once, on first use.

Each provider gets one file, `provider_<name>.rs`, named from its key: lowercase
letters, digits and hyphens are kept and every other byte becomes `%XX`. The crate
root, `lib.rs`, declares those modules by path and holds the `PROVIDERS` table and
the `models` function that assembles them. Providers are emitted in UTF-16 order
of their keys and each file lists its descriptors in UTF-16 order of their IDs;
filenames never determine enumeration order.

## Provider rules

The generator applies these rules, so the committed catalog carries their result:

- Direct `OpenAI` Responses models that accept reasoning effort `none` map `off` to it. GPT-5.5 Pro
  offers no off, minimal or low level, directly or through a gateway, and direct GPT-5.5 offers
  no minimal level.
- `OpenRouter` `DeepSeek` V4 maps `xhigh` to the gateway's own `xhigh`. `OpenRouter` Kimi K2.6 sends
  system instructions and replays reasoning content.
- All Xiaomi variants route through the `OpenAI`-compatible `/v1` endpoints, replay thinking as
  `reasoning_content`, and the token plans omit `MiMo` V2 Flash.
- Moonshot and the `OpenCode` Kimi K2.6 routes use the thinking-object format, so thinking off sends
  a disabled thinking object, and neither sends `reasoning_effort`. `OpenCode` Go Kimi K2.6 hides
  minimal, low and medium and has no off level. Gateway completion models send explicit limits as
  `max_tokens`.
- Nine extended-context `GitHub` Copilot models have a one-million-token window; other providers
  keep theirs. Direct `MiniMax` models keep their upstream limits.
- Grok 3 and Grok 3 Fast stay when the upstream feed omits them.

## Controlled catalog example

```rust
use maestro_models::{
    ModelThinkingLevel, Usage, calculate_cost, clamp_thinking_level,
    get_model, get_models, get_providers, get_supported_thinking_levels, models_are_equal,
};

let mut model = get_model("google", "gemini-2.5-flash").unwrap();
assert!(get_providers().contains(&model.provider));
assert!(get_models("google").iter().any(|entry| models_are_equal(Some(entry), Some(&model))));
assert!(get_supported_thinking_levels(&model).contains(&ModelThinkingLevel::High));
assert_eq!(clamp_thinking_level(&model, ModelThinkingLevel::Xhigh), ModelThinkingLevel::High);

model.name = "My descriptor".into();
assert_ne!(get_model("google", "gemini-2.5-flash").unwrap().name, model.name);
let mut usage: Usage = serde_json::from_value(serde_json::json!({
    "input": 100.0, "output": 20.0, "cacheRead": 0.0, "cacheWrite": 0.0, "totalTokens": 120.0,
    "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 0.0},
})).unwrap();
let estimate = calculate_cost(&model, &mut usage);
assert!(estimate.total > 0.0);
assert_eq!(usage.total_tokens.present(), Some(&120.0));
```
