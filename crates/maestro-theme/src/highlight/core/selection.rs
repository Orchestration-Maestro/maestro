//! Stable candidate ranking with plain-first and base-language ties.
use super::{Engine, IllegalPolicy, types::Score};
use std::cmp::Ordering;

impl Engine<'_> {
    /// Evaluate eligible candidates in the supplied subset or registration order.
    pub(in crate::highlight) fn automatic(&self, text: &str, subset: &[String]) -> Vec<Score> {
        let mut scores = vec![Score::plain()];
        let languages: Vec<_> = if subset.is_empty() {
            self.languages.iter().collect()
        } else {
            subset
                .iter()
                .filter_map(|name| self.language(name))
                .collect()
        };
        scores.extend(
            languages
                .into_iter()
                .filter(|l| l.root.is_some())
                .map(|l| self.evaluate(l, text, None, IllegalPolicy::Reject).score),
        );
        scores.sort_by(|a, b| {
            b.relevance()
                .total_cmp(&a.relevance())
                .then_with(|| self.base_order(a, b))
        });
        scores
    }

    /// A base grammar precedes its explicit superset at equal relevance.
    fn base_order(&self, a: &Score, b: &Score) -> Ordering {
        let a_base = a
            .language
            .and_then(|name| self.language(name))
            .and_then(|l| l.superset);
        let b_base = b
            .language
            .and_then(|name| self.language(name))
            .and_then(|l| l.superset);
        if a_base.is_some() && a_base == b.language {
            Ordering::Greater
        } else if b_base.is_some() && b_base == a.language {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}
