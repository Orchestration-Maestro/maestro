use super::search::{
    ModelEntry, ModelFilter, filter_models, format_tokens, model_entries, subsequence_score,
};
use maestro_app::Model;
use std::collections::HashSet;

/// A descriptor with the given identity and capabilities: `r` for reasoning, `i` for images.
fn model(provider: &str, id: &str, name: &str, capabilities: &str) -> Model {
    let reasoning = capabilities.contains('r');
    let input = if capabilities.contains('i') {
        r#"["text","image"]"#
    } else {
        r#"["text"]"#
    };
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","name":"{name}","api":"openai-completions","provider":"{provider}",
        "baseUrl":"","reasoning":{reasoning},"input":{input},
        "cost":{{"input":0,"output":0,"cacheRead":0,"cacheWrite":0}},
        "contextWindow":1000,"maxTokens":100}}"#
    ))
    .unwrap()
}
/// The row for `model` under its own provider.
fn entry(model: Model) -> ModelEntry {
    ModelEntry {
        provider: model.provider.clone(),
        id: model.id.clone(),
        model,
    }
}
/// No search, filters or restriction.
fn unfiltered() -> ModelFilter<'static> {
    ModelFilter {
        search_query: "",
        filter_thinking: false,
        filter_vision: false,
        current_model: None,
        allowed_providers: None,
    }
}
/// Row identifiers in order.
fn ids(entries: &[ModelEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.id.as_str()).collect()
}

#[test]
fn subsequence_score_ranks_tighter_ordered_matches() {
    assert!((subsequence_score("gpt", "openai gpt-4o") - 1.0).abs() < f64::EPSILON);
    assert!((subsequence_score("g4", "gpt-4") - 2.0 / 5.0).abs() < f64::EPSILON);
    assert!(subsequence_score("g4", "g4") > subsequence_score("g4", "gx4"));
    assert!(subsequence_score("4g", "gpt-4").abs() < f64::EPSILON);
    assert!(subsequence_score("gptx", "gpt").abs() < f64::EPSILON);
    assert!((subsequence_score("雪", "x 雪") - 1.0).abs() < f64::EPSILON);
}

#[test]
fn model_search_keeps_score_ties_in_list_order_and_ignores_case_and_whitespace() {
    let entries = vec![
        entry(model("b", "alpha-two", "Second", "")),
        entry(model("a", "alpha-one", "First", "")),
        entry(model("c", "a-l-p-h-a", "Spread", "")),
        entry(model("d", "omega", "Other", "")),
    ];
    let filter = ModelFilter {
        search_query: " AL Pha ",
        ..unfiltered()
    };
    let compare = |_: &str, _: &str| panic!("search keeps score order");
    let found = filter_models(entries, &filter, &compare);
    assert_eq!(ids(&found), ["alpha-two", "alpha-one", "a-l-p-h-a"]);
}

#[test]
fn model_list_puts_current_first_only_without_search_text() {
    let current = model("z", "current", "Current", "");
    let entries = || {
        vec![
            entry(model("b", "b-model", "B", "")),
            entry(current.clone()),
            entry(model("a", "a-model", "A", "")),
        ]
    };
    let filter = ModelFilter {
        current_model: Some(&current),
        ..unfiltered()
    };
    let sorted = filter_models(entries(), &filter, &str::cmp);
    assert_eq!(ids(&sorted), ["current", "a-model", "b-model"]);
    let blank = ModelFilter {
        search_query: "  ",
        current_model: Some(&current),
        ..unfiltered()
    };
    let compare = |_: &str, _: &str| panic!("typed text keeps list order");
    assert_eq!(
        ids(&filter_models(entries(), &blank, &compare)),
        ["b-model", "current", "a-model"]
    );
    let searching = ModelFilter {
        search_query: "model",
        current_model: Some(&current),
        ..unfiltered()
    };
    assert_eq!(
        ids(&filter_models(entries(), &searching, &compare)),
        ["b-model", "a-model"]
    );
}

#[test]
fn model_filters_restrict_providers_and_intersect_capabilities() {
    let entries = || {
        vec![
            entry(model("a", "plain", "Plain", "")),
            entry(model("a", "thinks", "Thinks", "r")),
            entry(model("b", "sees", "Sees", "i")),
            entry(model("b", "both", "Both", "ri")),
        ]
    };
    let thinking = ModelFilter {
        filter_thinking: true,
        ..unfiltered()
    };
    assert_eq!(
        ids(&filter_models(entries(), &thinking, &str::cmp)),
        ["thinks", "both"]
    );
    let vision = ModelFilter {
        filter_vision: true,
        ..unfiltered()
    };
    assert_eq!(
        ids(&filter_models(entries(), &vision, &str::cmp)),
        ["sees", "both"]
    );
    let both = ModelFilter {
        filter_thinking: true,
        filter_vision: true,
        ..unfiltered()
    };
    assert_eq!(ids(&filter_models(entries(), &both, &str::cmp)), ["both"]);
    let allowed: HashSet<String> = ["b".to_owned()].into();
    let restricted = ModelFilter {
        allowed_providers: Some(&allowed),
        ..unfiltered()
    };
    assert_eq!(
        ids(&filter_models(entries(), &restricted, &str::cmp)),
        ["sees", "both"]
    );
    let none: HashSet<String> = HashSet::new();
    let empty = ModelFilter {
        allowed_providers: Some(&none),
        ..unfiltered()
    };
    assert!(filter_models(entries(), &empty, &str::cmp).is_empty());
}

#[test]
fn model_entries_append_custom_models_under_their_own_provider() {
    let custom = model("My Server", "local-1", "Local", "");
    let entries = model_entries(std::slice::from_ref(&custom));
    let catalog = maestro_app::get_providers()
        .iter()
        .map(|provider| maestro_app::get_models(provider).len())
        .sum::<usize>();
    assert_eq!(entries.len(), catalog + 1);
    let last = entries.last().unwrap();
    assert_eq!(
        (last.provider.as_str(), last.id.as_str()),
        ("My Server", "local-1")
    );
    assert_eq!(last.model, custom);
    let first = &entries[0];
    assert_eq!(first.provider, maestro_app::get_providers()[0]);
    assert_eq!(first.model.provider, first.provider);
}

#[test]
fn format_tokens_uses_millions_thousands_and_whole_counts() {
    assert_eq!(format_tokens(2_000_000.0), "2M");
    assert_eq!(format_tokens(1_000_000.0), "1M");
    assert_eq!(format_tokens(200_000.0), "200K");
    assert_eq!(format_tokens(128_000.0), "128K");
    assert_eq!(format_tokens(1000.0), "1K");
    assert_eq!(format_tokens(999.0), "999");
    assert_eq!(format_tokens(0.0), "0");
}
