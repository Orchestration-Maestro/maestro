//! The replaceable syntax-highlighting interface and its native adapter.
use super::ThemeColor;
use std::collections::HashMap;
use std::error::Error;
use std::ops::Range;
use syntect::easy::ScopeRangeIterator;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// A failure of a highlighting engine, with its native cause.
type EngineError = Box<dyn Error + Send + Sync>;

/// One classified run of the highlighted code.
#[derive(Debug, Clone)]
pub struct SyntaxSpan {
    /// Byte range of the run in the supplied code.
    pub range: Range<usize>,
    /// One of the nine syntax colors, or `None` for text without a class.
    pub color: Option<ThemeColor>,
}

/// Classifies code in an explicitly named language.
pub trait SyntaxHighlighter {
    /// Whether the label names a language this highlighter can classify.
    fn supports_language(&self, language: &str) -> bool;

    /// Classify the complete `code` of a supported language.
    ///
    /// The returned spans are ordered, partition `code` on character boundaries and are
    /// not validated by the callers.
    ///
    /// # Errors
    /// Returns the engine's failure.
    fn highlight(&self, code: &str, language: &str) -> Result<Vec<SyntaxSpan>, EngineError>;
}

/// Syntect with the bundled extra syntaxes, selected by language label.
pub struct SyntectHighlighter {
    /// Parsed grammars.
    syntaxes: SyntaxSet,
    /// Lowercase label to a grammar name, or `None` for a label with no grammar here.
    aliases: HashMap<String, Option<String>>,
}

impl SyntectHighlighter {
    /// Load the grammars and the label overrides.
    ///
    /// # Errors
    /// Returns the failure to parse the bundled override table.
    pub fn new() -> Result<Self, EngineError> {
        Ok(Self {
            syntaxes: two_face::syntax::extra_newlines(),
            aliases: serde_json::from_str(include_str!("../../assets/syntax-aliases.json"))?,
        })
    }

    /// The grammar named by a label, which is matched without case.
    fn find(&self, language: &str) -> Option<&SyntaxReference> {
        match self.aliases.get(&language.to_ascii_lowercase()) {
            Some(name) => name
                .as_deref()
                .and_then(|name| self.syntaxes.find_syntax_by_name(name)),
            None => self.syntaxes.find_syntax_by_token(language),
        }
    }
}

impl SyntaxHighlighter for SyntectHighlighter {
    fn supports_language(&self, language: &str) -> bool {
        self.find(language).is_some()
    }

    /// Parses with one state for the whole code; an unsupported label is plain text.
    fn highlight(&self, code: &str, language: &str) -> Result<Vec<SyntaxSpan>, EngineError> {
        let syntax = self
            .find(language)
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());
        let (mut state, mut stack) = (ParseState::new(syntax), ScopeStack::new());
        let (mut spans, mut start) = (Vec::<SyntaxSpan>::new(), 0);
        for line in LinesWithEndings::from(code) {
            let ops = state.parse_line(line, &self.syntaxes)?;
            for (range, op) in ScopeRangeIterator::new(&ops, line) {
                stack.apply(op)?;
                let color = scope_color(stack.as_slice());
                push_run(&mut spans, start + range.start..start + range.end, color);
            }
            start += line.len();
        }
        Ok(spans)
    }
}

/// Append a nonempty run, joining it to the previous run of the same color.
fn push_run(spans: &mut Vec<SyntaxSpan>, range: Range<usize>, color: Option<ThemeColor>) {
    match spans.last_mut() {
        _ if range.is_empty() => {}
        Some(last) if last.color == color => last.range.end = range.end,
        _ => spans.push(SyntaxSpan { range, color }),
    }
}

/// Scope prefixes with their color; the longest matching prefix of a scope wins.
const RULES: [(&str, ThemeColor); 15] = [
    ("comment", ThemeColor::SyntaxComment),
    ("keyword", ThemeColor::SyntaxKeyword),
    ("keyword.operator", ThemeColor::SyntaxOperator),
    ("entity.name.function", ThemeColor::SyntaxFunction),
    ("support.function", ThemeColor::SyntaxFunction),
    ("variable", ThemeColor::SyntaxVariable),
    ("entity.other.attribute-name", ThemeColor::SyntaxVariable),
    ("string", ThemeColor::SyntaxString),
    ("constant.numeric", ThemeColor::SyntaxNumber),
    ("constant.language", ThemeColor::SyntaxNumber),
    ("storage.type", ThemeColor::SyntaxType),
    ("entity.name.type", ThemeColor::SyntaxType),
    ("entity.name.class", ThemeColor::SyntaxType),
    ("support.class", ThemeColor::SyntaxType),
    ("punctuation", ThemeColor::SyntaxPunctuation),
];

/// The color of the innermost scope that has one.
fn scope_color(stack: &[Scope]) -> Option<ThemeColor> {
    stack
        .iter()
        .rev()
        .find_map(|scope| rule_color(&scope.build_string()))
}

/// The color of the longest rule prefix that contains whole dotted segments of `scope`.
fn rule_color(scope: &str) -> Option<ThemeColor> {
    RULES
        .iter()
        .filter(|(prefix, _)| {
            scope
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        })
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, color)| color.clone())
}

#[cfg(test)]
mod tests;
