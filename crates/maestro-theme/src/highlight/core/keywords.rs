//! Exact ordered keyword relevance with invocation-local hit limits.
use super::Run;

impl Run<'_> {
    /// Add keyword weights in match order, at most seven times per word.
    pub(super) fn keywords(&mut self, text: &str) {
        let mode = &self.engine.tables.modes[self.stack.current.mode.0];
        let (Some(pattern), Some(table)) = (mode.keyword_pattern, mode.keywords) else {
            return;
        };
        let Some(pattern) = self.engine.tables.patterns[pattern.0]
            .get()
            .and_then(|pattern| pattern.as_ref().ok())
        else {
            return;
        };
        for found in pattern.find_iter(text) {
            let word = &text[found.range];
            let word = if self.language.fold {
                word.to_lowercase()
            } else {
                word.to_owned()
            };
            let Some(weight) = self.engine.tables.keywords[table].get(&word) else {
                continue;
            };
            let count = self.hits.entry(word).or_default();
            *count = count.saturating_add(1);
            if *count <= 7 {
                self.relevance += weight;
            }
        }
    }
}
