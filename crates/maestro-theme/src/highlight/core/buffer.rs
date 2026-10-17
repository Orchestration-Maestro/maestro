//! Scoring-buffer ownership and named embedded continuations.
use super::{
    IllegalPolicy, Run,
    matcher::Hit,
    types::{Scope, Score, SubLanguage},
};

impl Run<'_> {
    /// Score pending host text or parse it using the admitted embedded policy.
    pub(super) fn flush(&mut self) {
        let text = std::mem::take(&mut self.buffer);
        if text.is_empty() {
            return;
        }
        let mode = &self.engine.tables.modes[self.stack.current.mode.0];
        let score = match &mode.sub {
            SubLanguage::Named(name) => self.named(name, &text),
            SubLanguage::Automatic(subset) => self
                .engine
                .automatic(&text, subset)
                .first()
                .map_or(0., Score::relevance),
            SubLanguage::None => {
                self.keywords(&text);
                return;
            }
        };
        if mode.relevance > 0. {
            self.relevance += score;
        }
    }

    /// Resume a named embedded language, retaining its frames within this query.
    fn named(&mut self, name: &str, text: &str) -> f64 {
        let Some(language) = self.engine.language(name) else {
            return 0.;
        };
        let result = self.engine.evaluate(
            language,
            text,
            self.continuations.remove(name),
            IllegalPolicy::Ignore,
        );
        if let Some(stack) = result.stack {
            self.continuations.insert(name.to_owned(), stack);
        }
        result.score.relevance()
    }

    /// Score only the unclassified captures left by a scoped delimiter.
    pub(super) fn captures(&mut self, scope: &Scope, hit: &Hit<'_>) {
        let Scope::Captures(indices) = scope else {
            return;
        };
        for text in indices
            .iter()
            .filter_map(|index| hit.captures.get(*index).copied().flatten())
        {
            self.keywords(text);
        }
    }
}
