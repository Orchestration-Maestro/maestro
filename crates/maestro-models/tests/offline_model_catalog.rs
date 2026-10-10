use maestro_models::{get_model, get_models, get_providers};
use serde_json::Value;
use std::collections::BTreeSet;

fn assert_descriptor(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, value) in b {
                assert!(a.contains_key(key), "missing descriptor field {key}");
                assert_descriptor(&a[key], value);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_descriptor(a, b);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

fn assert_exact_misses() {
    for missing in ["", "unknown", "GOOGLE"] {
        assert!(get_models(missing).is_empty());
        assert!(get_model(missing, "gemini-2.5-flash").is_none());
    }
    for missing in ["", "unknown", "GEMINI-2.5-FLASH"] {
        assert!(get_model("google", missing).is_none());
    }
    for boundary in [" ", "\u{feff}", "\u{85}", "\u{2003}"] {
        for provider in [format!("{boundary}google"), format!("google{boundary}")] {
            assert!(get_models(&provider).is_empty());
            assert!(get_model(&provider, "gemini-2.5-flash").is_none());
        }
        for id in [
            format!("{boundary}gemini-2.5-flash"),
            format!("gemini-2.5-flash{boundary}"),
        ] {
            assert!(get_model("google", &id).is_none());
        }
    }
}

#[test]
fn maestro_catalog_retains_every_descriptor() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/offline_model_catalog.json")).unwrap();
    let providers = fixture.as_object().unwrap();
    assert_eq!(
        get_providers(),
        providers.keys().cloned().collect::<Vec<_>>()
    );
    assert_eq!(providers.len(), 31);
    let mut protocols = BTreeSet::new();
    let mut counts = [0; 4];
    for (provider, records) in providers {
        let records = records.as_object().unwrap();
        let models = get_models(provider);
        assert_eq!(models.len(), records.len());
        for ((key, expected), model) in records.iter().zip(models) {
            assert_eq!(model.id, *key);
            assert_descriptor(&serde_json::to_value(&model).unwrap(), expected);
            assert_eq!(get_model(provider, key).unwrap(), model);
            protocols.insert(model.api);
            counts[0] += 1;
            counts[1] += usize::from(model.thinking_level_map.is_some());
            counts[2] += usize::from(model.compat.is_some());
            counts[3] += usize::from(model.headers.is_some());
        }
    }
    assert_eq!(counts, [970, 157, 71, 28]);
    assert_eq!(
        protocols,
        [
            "anthropic-messages",
            "azure-openai-responses",
            "bedrock-converse-stream",
            "google-generative-ai",
            "google-vertex",
            "mistral-conversations",
            "openai-codex-responses",
            "openai-completions",
            "openai-responses"
        ]
        .map(str::to_owned)
        .into()
    );
    assert_exact_misses();
}

#[test]
fn maestro_catalog_returns_independent_descriptors() {
    use maestro_models::ModelThinkingLevel;
    let original = get_model("github-copilot", "gpt-4.1").unwrap();
    let mut edited = get_model("github-copilot", "gpt-4.1").unwrap();
    edited.id = "edited-id".into();
    edited.provider = "edited-provider".into();
    edited.name = "Edited".into();
    edited.cost.input = -10.0;
    edited.headers.as_mut().unwrap().clear();
    edited.thinking_level_map = Some([(ModelThinkingLevel::Xhigh, None)].into());
    let Some(compat) = &mut edited.compat else {
        panic!("expected completion compatibility");
    };
    compat
        .0
        .insert("sendSessionAffinityHeaders".into(), false.into());
    assert_ne!(edited, original);
    assert_eq!(get_model("github-copilot", "gpt-4.1").unwrap(), original);
    assert!(get_model("edited-provider", "edited-id").is_none());
    assert!(get_model("github-copilot", "edited-id").is_none());
    assert!(get_models("github-copilot").contains(&original));
    let baseline = get_models("github-copilot");
    let mut list = get_models("github-copilot");
    list[0].name = "Changed list entry".into();
    list.remove(1);
    list.reverse();
    assert_eq!(get_models("github-copilot"), baseline);
    let mut providers = get_providers();
    providers.clear();
    assert_eq!(get_providers().len(), 31);
    let original = get_model("deepseek", "deepseek-v4-flash").unwrap();
    let mut edited = get_model("deepseek", "deepseek-v4-flash").unwrap();
    edited.thinking_level_map.as_mut().unwrap().clear();
    assert_ne!(edited.thinking_level_map, original.thinking_level_map);
    assert_eq!(
        get_model("deepseek", "deepseek-v4-flash").unwrap(),
        original
    );
}

const COST_CASES: [([f64; 4], [f64; 4], [f64; 4]); 7] = [
    (
        [2.0, 3.0, 5.0, 7.0],
        [11.0, 13.0, 17.0, 19.0],
        [0.000_022, 0.000_039, 0.000_085, 0.000_133],
    ),
    (
        [1e308, 0.0, 0.0, 0.0],
        [1e6, 0.0, 0.0, 0.0],
        [1e308, 0.0, 0.0, 0.0],
    ),
    ([0.0; 4], [1.0; 4], [0.0; 4]),
    ([1.0; 4], [0.0; 4], [0.0; 4]),
    (
        [-2.5, 4.0, -8.0, 16.0],
        [0.5, -0.25, -0.5, 0.125],
        [-0.000_001_25, -0.000_001, 0.000_004, 0.000_002],
    ),
    (
        [f64::INFINITY, 1.0, 1.0, 1.0],
        [1.0; 4],
        [f64::INFINITY, 0.000_001, 0.000_001, 0.000_001],
    ),
    (
        [f64::INFINITY, f64::NAN, 1.0, 1.0],
        [0.0, 1.0, 1.0, 1.0],
        [f64::NAN, f64::NAN, 0.000_001, 0.000_001],
    ),
];

#[test]
fn maestro_costs_use_each_reported_category_once() {
    let mut model = get_model("google", "gemini-2.5-flash").unwrap();

    for (rates, counts, expected) in COST_CASES {
        assert_cost_case(&mut model, rates, counts, expected);
    }
    assert_cost_sum_order(&mut model);
}

fn assert_cost_case(
    model: &mut maestro_models::Model,
    rates: [f64; 4],
    counts: [f64; 4],
    expected: [f64; 4],
) {
    use maestro_models::{ModelCost, Usage, UsageCost, calculate_cost};
    model.cost = ModelCost {
        input: rates[0],
        output: rates[1],
        cache_read: rates[2],
        cache_write: rates[3],
    };
    let mut usage = Usage {
        input: counts[0],
        output: counts[1],
        cache_read: counts[2],
        cache_write: counts[3],
        total_tokens: -123.5,
        cost: UsageCost {
            input: 99.0,
            output: 99.0,
            cache_read: 99.0,
            cache_write: 99.0,
            total: 99.0,
        },
    };
    let result = calculate_cost(model, &mut usage);
    for (actual, expected) in [
        result.input,
        result.output,
        result.cache_read,
        result.cache_write,
    ]
    .into_iter()
    .zip(expected)
    {
        assert_float(actual, expected);
    }
    assert_float(
        result.total,
        ((expected[0] + expected[1]) + expected[2]) + expected[3],
    );
    result.input = 42.0;
    assert_float(usage.cost.input, 42.0);
    assert_eq!(
        [
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write
        ]
        .map(f64::to_bits),
        counts.map(f64::to_bits)
    );
    assert_float(usage.total_tokens, -123.5);
}

fn assert_cost_sum_order(model: &mut maestro_models::Model) {
    assert_cost_case(
        model,
        [1_000_000.0; 4],
        [1e16, -1e16, 1.0, 2.0],
        [1e16, -1e16, 1.0, 2.0],
    );
}

fn assert_float(actual: f64, expected: f64) {
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

#[test]
fn maestro_model_equality_uses_provider_and_id() {
    use maestro_models::models_are_equal;
    let original = get_model("google", "gemini-2.5-flash").unwrap();
    assert!(!models_are_equal(None, None));
    assert!(!models_are_equal(Some(&original), None));
    assert!(!models_are_equal(None, Some(&original)));
    assert!(models_are_equal(Some(&original), Some(&original.clone())));
    let mut custom = get_model("github-copilot", "gpt-4.1").unwrap();
    custom.thinking_level_map = Some([(maestro_models::ModelThinkingLevel::Xhigh, None)].into());
    custom.id.clone_from(&original.id);
    custom.provider.clone_from(&original.provider);
    assert!(models_are_equal(Some(&original), Some(&custom)));
    custom.provider = "different".into();
    assert!(!models_are_equal(Some(&original), Some(&custom)));
    custom.provider.clone_from(&original.provider);
    custom.id = "different".into();
    assert!(!models_are_equal(Some(&original), Some(&custom)));
}
