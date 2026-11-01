//! Model list assembly, ordered-character search and capability filters.
use maestro_app::{Model, ModelInput, get_models, get_providers, models_are_equal};
use num_traits::ToPrimitive as _;
use std::{cmp::Ordering, collections::HashSet};

/// One listed model with the provider name it is shown under.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModelEntry {
    /// Provider name shown on the row and matched by search.
    pub(crate) provider: String,
    /// Model identifier shown on the row.
    pub(crate) id: String,
    /// The descriptor selected by this row.
    pub(crate) model: Model,
}
/// Search text and filters applied to the list.
pub(crate) struct ModelFilter<'a> {
    /// Search text as typed.
    pub(crate) search_query: &'a str,
    /// Keep only reasoning models.
    pub(crate) filter_thinking: bool,
    /// Keep only models accepting images.
    pub(crate) filter_vision: bool,
    /// Model shown first while no search text is typed.
    pub(crate) current_model: Option<&'a Model>,
    /// Providers whose models may be listed; absence admits every provider.
    pub(crate) allowed_providers: Option<&'a HashSet<String>>,
}
/// How tightly `query`'s characters match `text` in order, in `(0, 1]`, or zero when they do not.
///
/// An exact substring scores one; each skipped character between matches lowers the score.
pub(crate) fn subsequence_score(query: &str, text: &str) -> f64 {
    let query: Vec<char> = query.chars().collect();
    let mut matched = 0;
    let mut gaps = 0_usize;
    let mut last = None;
    for (index, character) in text.chars().enumerate() {
        if matched == query.len() {
            break;
        }
        if query[matched] == character {
            if let Some(last) = last {
                gaps += index - last - 1;
            }
            last = Some(index);
            matched += 1;
        }
    }
    if matched < query.len() {
        return 0.0;
    }
    let length = query.len().to_f64().unwrap_or(f64::MAX);
    length / (length + gaps.to_f64().unwrap_or(f64::MAX))
}
/// Every catalog model in provider order, then `custom` models under their own provider names.
pub(crate) fn model_entries(custom: &[Model]) -> Vec<ModelEntry> {
    let known = get_providers().into_iter().flat_map(|provider| {
        get_models(&provider)
            .into_iter()
            .map(move |model| ModelEntry {
                provider: provider.clone(),
                id: model.id.clone(),
                model,
            })
    });
    let custom = custom.iter().map(|model| ModelEntry {
        provider: model.provider.clone(),
        id: model.id.clone(),
        model: model.clone(),
    });
    known.chain(custom).collect()
}
/// Apply the provider restriction, search ranking and capability filters to `entries`.
///
/// Typed search text is lowercased with its whitespace removed and ranks matches by score,
/// keeping list order between equal scores. Without search text the current model comes first
/// and the rest are ordered by provider with `compare`.
pub(crate) fn filter_models(
    entries: Vec<ModelEntry>,
    filter: &ModelFilter<'_>,
    compare: &dyn Fn(&str, &str) -> Ordering,
) -> Vec<ModelEntry> {
    let mut entries: Vec<ModelEntry> = match filter.allowed_providers {
        Some(allowed) => entries
            .into_iter()
            .filter(|entry| allowed.contains(&entry.provider))
            .collect(),
        None => entries,
    };
    let query: String = filter
        .search_query
        .to_lowercase()
        .chars()
        .filter(|character| !maestro_app::is_whitespace_scalar(*character))
        .collect();
    if !query.is_empty() {
        let mut scored: Vec<(ModelEntry, f64)> = entries
            .into_iter()
            .filter_map(|entry| {
                let text =
                    format!("{} {} {}", entry.provider, entry.id, entry.model.name).to_lowercase();
                let score = subsequence_score(&query, &text);
                (score > 0.0).then_some((entry, score))
            })
            .collect();
        scored.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));
        entries = scored.into_iter().map(|(entry, _)| entry).collect();
    }
    entries.retain(|entry| {
        (!filter.filter_thinking || entry.model.reasoning)
            && (!filter.filter_vision || entry.model.input.contains(&ModelInput::Image))
    });
    if filter.search_query.is_empty() {
        let current =
            |entry: &ModelEntry| models_are_equal(filter.current_model, Some(&entry.model));
        entries.sort_by(|a, b| match (current(a), current(b)) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => compare(&a.provider, &b.provider),
        });
    }
    entries
}
/// A token count in millions above a million, thousands above a thousand, otherwise whole.
pub(crate) fn format_tokens(tokens: f64) -> String {
    let mut buffer = ryu_js::Buffer::new();
    if tokens >= 1_000_000.0 {
        format!("{}M", buffer.format_to_fixed(tokens / 1_000_000.0, 0))
    } else if tokens >= 1000.0 {
        format!("{}K", buffer.format_to_fixed(tokens / 1000.0, 0))
    } else {
        buffer.format(tokens).to_owned()
    }
}
