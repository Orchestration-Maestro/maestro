use maestro_models::{Model, get_model, get_models};
use std::sync::{Arc, Mutex, RwLock};
static CATALOG: Mutex<()> = Mutex::new(());
struct Restore(Arc<RwLock<Model>>, Option<Model>);
impl Drop for Restore {
    fn drop(&mut self) {
        let old = std::mem::replace(&mut *self.0.write().unwrap(), self.1.take().unwrap());
        drop(old);
    }
}
#[test]
fn catalog_lookup_keeps_live_descriptor_identity() {
    let _serial = CATALOG.lock().unwrap();
    let handle = get_model("openai", "gpt-4o-mini").unwrap();
    let original = handle.read().unwrap().clone();
    let _restore = Restore(handle.clone(), Some(original));
    assert!(Arc::ptr_eq(
        &handle,
        &get_model("openai", "gpt-4o-mini").unwrap()
    ));
    assert!(get_models("openai").iter().any(|m| Arc::ptr_eq(m, &handle)));
    {
        let mut model = handle.write().unwrap();
        model.name = "changed".into();
        model.cost.input = -99.0;
        model.cost.output = -98.0;
        model.cost.cache_read = -97.0;
        model.cost.cache_write = -96.0;
        model.provider = "changed-provider".into();
        model.id = "changed-id".into();
    }
    let again = get_model("openai", "gpt-4o-mini").unwrap();
    assert!(Arc::ptr_eq(&handle, &again));
    let observed = again.read().unwrap().clone();
    assert_eq!(observed.name, "changed");
    assert_eq!(
        [
            observed.cost.input,
            observed.cost.output,
            observed.cost.cache_read,
            observed.cost.cache_write
        ],
        [-99.0, -98.0, -97.0, -96.0]
    );
    assert_eq!(observed.provider, "changed-provider");
    assert_eq!(observed.id, "changed-id");
    assert!(get_model("changed-provider", "changed-id").is_none());
    for (provider, id) in [
        ("unknown", "gpt-4o-mini"),
        ("openai", "unknown"),
        ("", ""),
        ("", "gpt-4o-mini"),
        ("openai", ""),
        ("Openai", "gpt-4o-mini"),
        ("openai", "GPT-4o-mini"),
    ] {
        assert!(get_model(provider, id).is_none());
    }
    for prefix in [" ", "\u{feff}", "\u{85}"] {
        assert!(get_model(&format!("{prefix}openai"), "gpt-4o-mini").is_none());
        assert!(get_model("openai", &format!("{prefix}gpt-4o-mini")).is_none());
    }
}
#[test]
fn catalog_lists_preserve_generated_positions() {
    let _serial = CATALOG.lock().unwrap();
    let expected = [
        ("amazon-bedrock", 93),
        ("anthropic", 23),
        ("azure-openai-responses", 42),
        ("cerebras", 4),
        ("cloudflare-ai-gateway", 35),
        ("cloudflare-workers-ai", 8),
        ("deepseek", 2),
        ("fireworks", 19),
        ("github-copilot", 26),
        ("google", 27),
        ("google-vertex", 13),
        ("groq", 18),
        ("huggingface", 22),
        ("kimi-coding", 2),
        ("minimax", 2),
        ("minimax-cn", 2),
        ("mistral", 28),
        ("moonshotai", 7),
        ("moonshotai-cn", 7),
        ("openai", 42),
        ("openai-codex", 10),
        ("opencode", 38),
        ("opencode-go", 14),
        ("openrouter", 274),
        ("vercel-ai-gateway", 162),
        ("xai", 25),
        ("xiaomi", 5),
        ("xiaomi-token-plan-ams", 5),
        ("xiaomi-token-plan-cn", 5),
        ("xiaomi-token-plan-sgp", 5),
        ("zai", 5),
    ];
    let providers = maestro_models::get_providers();
    assert_eq!(
        providers,
        expected
            .iter()
            .map(|(p, _)| p.to_string())
            .collect::<Vec<_>>()
    );
    for (provider, count) in expected {
        let mut models = get_models(provider);
        assert_eq!(models.len(), count);
        let first = models[0].clone();
        models.reverse();
        models.pop();
        models.clear();
        assert!(Arc::ptr_eq(&first, &get_models(provider)[0]));
    }
    let mut edited = providers;
    edited.reverse();
    edited.pop();
    edited.clear();
    assert_eq!(maestro_models::get_providers().len(), 31);
    assert_eq!(
        maestro_models::get_providers(),
        expected
            .iter()
            .map(|(p, _)| p.to_string())
            .collect::<Vec<_>>()
    );
    assert!(get_models("unknown").is_empty());
}
#[path = "support/catalog_snapshot.rs"]
mod snapshot;
#[test]
fn catalog_snapshot_covers_every_descriptor() {
    let _serial = CATALOG.lock().unwrap();
    let data = snapshot::snapshot();
    let providers = data.as_array().unwrap();
    assert_eq!(providers.len(), 31);
    let mut count = 0;
    let mut optional = [0; 3];
    let mut apis = std::collections::BTreeSet::new();
    for provider in providers {
        for entry in provider[1].as_array().unwrap() {
            let model = &entry[1];
            count += 1;
            apis.insert(model["api"].as_str().unwrap());
            for (i, key) in ["thinkingLevelMap", "compat", "headers"].iter().enumerate() {
                optional[i] += usize::from(model.get(key).is_some());
            }
            for key in [
                "id",
                "name",
                "api",
                "provider",
                "baseUrl",
                "reasoning",
                "input",
                "cost",
                "contextWindow",
                "maxTokens",
            ] {
                assert!(model.get(key).is_some());
            }
        }
    }
    assert_eq!(count, 970);
    assert_eq!(optional, [157, 71, 28]);
    assert_eq!(
        apis,
        [
            "bedrock-converse-stream",
            "anthropic-messages",
            "azure-openai-responses",
            "openai-completions",
            "openai-responses",
            "google-generative-ai",
            "google-vertex",
            "mistral-conversations",
            "openai-codex-responses"
        ]
        .into_iter()
        .collect()
    );
    let text = snapshot::compact(&data);
    assert_eq!(text.len(), 338697);
    assert_eq!(
        maestro_models::short_hash(&text.encode_utf16().collect::<Vec<_>>()),
        "wdiks4lx4n7a"
    );
    let mut changed = data.clone();
    changed[0][1][0][1]["name"] = serde_json::json!("changed");
    assert_ne!(
        maestro_models::short_hash(
            &snapshot::compact(&changed)
                .encode_utf16()
                .collect::<Vec<_>>()
        ),
        "wdiks4lx4n7a"
    );
    let mut reordered = data.clone();
    reordered[0][1].as_array_mut().unwrap().swap(0, 1);
    assert_ne!(
        maestro_models::short_hash(
            &snapshot::compact(&reordered)
                .encode_utf16()
                .collect::<Vec<_>>()
        ),
        "wdiks4lx4n7a"
    );
    let sentinel = get_model("openrouter", "openrouter/auto").unwrap();
    let model = sentinel.read().unwrap();
    assert_eq!(
        [
            model.cost.input,
            model.cost.output,
            model.cost.cache_read,
            model.cost.cache_write
        ],
        [-1000000.0, -1000000.0, 0.0, 0.0]
    );
}
fn supplied() -> Model {
    serde_json::from_value(serde_json::json!({"id":"supplied","name":"Caller","api":"custom","provider":"custom","baseUrl":"local","reasoning":true,"input":["text"],"cost":{"input":2,"output":8,"cacheRead":0.5,"cacheWrite":3},"contextWindow":100,"maxTokens":50})).unwrap()
}
fn usage(tokens: [f64; 4]) -> maestro_models::Usage {
    maestro_models::Usage {
        input: tokens[0],
        output: tokens[1],
        cache_read: tokens[2],
        cache_write: tokens[3],
        total_tokens: 123.0,
        cost: maestro_models::UsageCost {
            input: 9.0,
            output: 9.0,
            cache_read: 9.0,
            cache_write: 9.0,
            total: 9.0,
        },
    }
}
#[test]
fn cost_updates_the_existing_usage_cost() {
    let model = supplied();
    let mut usage = usage([250000.0, 125000.0, 1000000.0, 2000000.0]);
    let address = std::ptr::addr_of!(usage.cost);
    let cost = maestro_models::calculate_cost(&model, &mut usage);
    assert_eq!(address, cost as *const _);
    assert_eq!(
        [
            cost.input,
            cost.output,
            cost.cache_read,
            cost.cache_write,
            cost.total
        ],
        [0.5, 1.0, 0.5, 6.0, 8.0]
    );
    cost.total = 42.0;
    assert_eq!(usage.cost.total, 42.0);
    assert_eq!(
        [
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write,
            usage.total_tokens
        ],
        [250000.0, 125000.0, 1000000.0, 2000000.0, 123.0]
    );
}
#[test]
fn cost_preserves_binary64_edge_values() {
    for (rate, tokens, expected) in [
        (-2.0, 3.0, -0.000006),
        (2.0, -3.0, -0.000006),
        (0.0, -3.0, -0.0),
        (-0.0, 3.0, -0.0),
        (2.0, 0.25, 5e-7),
        (f64::NAN, 2.0, f64::NAN),
        (f64::INFINITY, 2.0, f64::INFINITY),
        (f64::NEG_INFINITY, 2.0, f64::NEG_INFINITY),
        (1e308, 1e308, f64::INFINITY),
        (1e308, 1e-308, 0.000001),
        (1e-320, 1e308, 0.0),
    ] {
        let mut model = supplied();
        model.cost = maestro_models::TokenRates {
            input: rate,
            output: rate,
            cache_read: rate,
            cache_write: rate,
        };
        let mut usage = usage([tokens; 4]);
        let cost = maestro_models::calculate_cost(&model, &mut usage);
        for actual in [cost.input, cost.output, cost.cache_read, cost.cache_write] {
            if expected.is_nan() {
                assert!(actual.is_nan());
            } else {
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
        }
        let total = ((expected + expected) + expected) + expected;
        if total.is_nan() {
            assert!(cost.total.is_nan());
        } else {
            assert_eq!(cost.total.to_bits(), total.to_bits());
        }
    }
    let mut model = supplied();
    model.cost = maestro_models::TokenRates {
        input: 1e22,
        output: -1e22,
        cache_read: 1e6,
        cache_write: 1e6,
    };
    assert_eq!(
        maestro_models::calculate_cost(&model, &mut usage([1.0; 4])).total,
        2.0
    );
}
#[test]
fn model_equality_uses_only_provider_and_id() {
    let a = supplied();
    let mut b = a.clone();
    b.name = "different".into();
    b.api = "different".into();
    b.base_url = "different".into();
    b.reasoning = false;
    b.thinking_level_map = Some(serde_json::Map::new());
    b.input = vec![];
    b.cost.input = 99.0;
    b.cost.output = 99.0;
    b.cost.cache_read = 99.0;
    b.cost.cache_write = 99.0;
    b.context_window = 1.0;
    b.max_tokens = 1.0;
    b.headers = Some(serde_json::Map::new());
    b.compat = Some(serde_json::json!({}));
    assert!(maestro_models::models_are_equal(Some(&a), Some(&b)));
    for change in ["different", "Supplied", " supplied"] {
        let mut c = b.clone();
        c.id = change.into();
        assert!(!maestro_models::models_are_equal(Some(&a), Some(&c)));
    }
    for change in ["different", "Custom", " custom"] {
        let mut c = b.clone();
        c.provider = change.into();
        assert!(!maestro_models::models_are_equal(Some(&a), Some(&c)));
    }
    for (left, right) in [(None, None), (Some(&a), None), (None, Some(&a))] {
        assert!(!maestro_models::models_are_equal(left, right));
    }
}
use maestro_models::ModelThinkingLevel::{self, High, Low, Medium, Minimal, Off, Xhigh};
fn levels(model: &Model) -> Vec<ModelThinkingLevel> {
    maestro_models::get_supported_thinking_levels(model)
}
#[test]
fn thinking_levels_distinguish_missing_null_and_present() {
    let mut model = supplied();
    let base = vec![Off, Minimal, Low, Medium, High];
    assert_eq!(levels(&model), base);
    model.thinking_level_map = Some(serde_json::Map::new());
    assert_eq!(levels(&model), base);
    for value in [
        serde_json::json!(""),
        serde_json::json!(0),
        serde_json::json!(false),
        serde_json::json!({}),
        serde_json::json!([]),
        serde_json::json!("normal"),
    ] {
        model.thinking_level_map = Some(
            serde_json::from_value(serde_json::json!({"xhigh":value,"unknown":null})).unwrap(),
        );
        assert_eq!(levels(&model), vec![Off, Minimal, Low, Medium, High, Xhigh]);
    }
    for (spelling, level) in [
        ("off", Off),
        ("minimal", Minimal),
        ("low", Low),
        ("medium", Medium),
        ("high", High),
        ("xhigh", Xhigh),
    ] {
        model.thinking_level_map =
            Some(serde_json::from_value(serde_json::json!({"xhigh":"supported"})).unwrap());
        model
            .thinking_level_map
            .as_mut()
            .unwrap()
            .insert(spelling.into(), serde_json::Value::Null);
        let result = levels(&model);
        assert!(!result.contains(&level));
        assert_eq!(result.len(), 5);
    }
    model.thinking_level_map=Some(serde_json::from_value(serde_json::json!({"off":null,"minimal":null,"low":null,"medium":null,"high":null,"xhigh":null})).unwrap());
    assert!(levels(&model).is_empty());
    model.reasoning = false;
    assert_eq!(levels(&model), vec![Off]);
    let mut fresh = levels(&model);
    fresh.clear();
    assert_eq!(levels(&model), vec![Off]);
}
#[test]
fn thinking_clamp_prefers_upward_before_downward() {
    let mut model = supplied();
    model.thinking_level_map =
        Some(serde_json::from_value(serde_json::json!({"xhigh":"supported"})).unwrap());
    for (name, level) in [
        ("off", Off),
        ("minimal", Minimal),
        ("low", Low),
        ("medium", Medium),
        ("high", High),
        ("xhigh", Xhigh),
    ] {
        assert_eq!(maestro_models::clamp_thinking_level(&model, name), level);
    }
    model.thinking_level_map=Some(serde_json::from_value(serde_json::json!({"off":null,"minimal":null,"medium":null,"high":null,"xhigh":"supported"})).unwrap());
    for (name, level) in [
        ("off", Low),
        ("minimal", Low),
        ("low", Low),
        ("medium", Xhigh),
        ("high", Xhigh),
        ("xhigh", Xhigh),
    ] {
        assert_eq!(maestro_models::clamp_thinking_level(&model, name), level);
    }
    model
        .thinking_level_map
        .as_mut()
        .unwrap()
        .insert("xhigh".into(), serde_json::Value::Null);
    assert_eq!(maestro_models::clamp_thinking_level(&model, "high"), Low);
    model.thinking_level_map = None;
    assert_eq!(maestro_models::clamp_thinking_level(&model, "xhigh"), High);
    model.reasoning = false;
    assert_eq!(maestro_models::clamp_thinking_level(&model, "xhigh"), Off);
    model.reasoning = true;
    model.thinking_level_map=Some(serde_json::from_value(serde_json::json!({"off":null,"minimal":null,"low":null,"medium":null,"high":null,"xhigh":null})).unwrap());
    assert_eq!(maestro_models::clamp_thinking_level(&model, "high"), Off);
    assert!(levels(&model).is_empty());
}
#[test]
fn thinking_clamp_unknown_spelling_uses_first_available() {
    let mut model = supplied();
    for request in [
        "",
        "HIGH",
        "max",
        " high ",
        "\u{feff}high",
        "\u{85}high",
        "思考",
    ] {
        assert_eq!(maestro_models::clamp_thinking_level(&model, request), Off);
        model.thinking_level_map = Some(
            serde_json::from_value(
                serde_json::json!({"off":null,"minimal":null,"low":null,"medium":null}),
            )
            .unwrap(),
        );
        assert_eq!(maestro_models::clamp_thinking_level(&model, request), High);
        model
            .thinking_level_map
            .as_mut()
            .unwrap()
            .insert("high".into(), serde_json::Value::Null);
        assert_eq!(maestro_models::clamp_thinking_level(&model, request), Off);
        model.thinking_level_map = None;
    }
}
#[test]
fn catalog_snapshot_number_spelling_is_stable() {
    for (value, expected) in [
        (-0.0, "0"),
        (2.0, "2"),
        (1e-7, "1e-7"),
        (1e-6, "0.000001"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (0.33, "0.33"),
    ] {
        assert_eq!(snapshot::compact(&serde_json::json!(value)), expected);
    }
    let value: serde_json::Value =
        serde_json::from_str(r#"{"z":[null,"quote\"\n\u0001","思考"],"a":{"second":2,"first":1}}"#)
            .unwrap();
    assert_eq!(
        snapshot::compact(&value),
        "{\"z\":[null,\"quote\\\"\\n\\u0001\",\"思考\"],\"a\":{\"second\":2,\"first\":1}}"
    );
}
#[test]
fn catalog_operations_have_no_console_output() {
    let _serial = CATALOG.lock().unwrap();
    if std::env::var_os("MAESTRO_CATALOG_SILENCE_CHILD").is_some() {
        let handle = get_model("openai", "gpt-4o-mini").unwrap();
        assert!(get_model("missing", "missing").is_none());
        assert_eq!(maestro_models::get_providers().len(), 31);
        assert!(!get_models("openai").is_empty());
        let model = handle.read().unwrap().clone();
        let mut usage = usage([1.0; 4]);
        maestro_models::calculate_cost(&model, &mut usage);
        assert!(!levels(&model).is_empty());
        maestro_models::clamp_thinking_level(&model, "invalid");
        assert!(maestro_models::models_are_equal(Some(&model), Some(&model)));
        std::process::exit(0);
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "catalog_operations_have_no_console_output",
            "--nocapture",
        ])
        .env("MAESTRO_CATALOG_SILENCE_CHILD", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert_eq!(output.stdout, b"\nrunning 1 test\n");
}

#[test]
fn catalog_guide_keeps_all_documented_paragraphs() {
    assert_eq!(
        include_str!("../../../docs/models/catalog.md"),
        r##"# Offline model catalog

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
"##
    );
}
