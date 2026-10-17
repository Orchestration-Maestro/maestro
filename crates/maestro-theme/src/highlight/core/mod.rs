//! One interpreter over immutable native grammars.
mod buffer;
mod callbacks;
mod keywords;
mod matcher;
mod selection;
mod transitions;
pub(super) mod types;
use super::{HighlightProfile, languages};
use regress::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, LazyLock};
use types::{Evaluation, KeywordHits, Language, Mode, Stack};

/// Owned preparation failures, retaining their native cause.
pub(in crate::highlight) type Failure = Box<dyn Error + Send + Sync>;

/// Immutable shared patterns, keyword tables and modes.
pub(in crate::highlight) struct Tables {
    /// Native expressions compiled once.
    patterns: Vec<Regex>,
    /// Interned keyword weights.
    keywords: Vec<HashMap<String, f64>>,
    /// Immutable mode graph.
    modes: Vec<Mode>,
}

/// An expression and its native matching flags.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pattern {
    /// Source expression.
    pattern: String,
    /// Native expression flags.
    flags: String,
}

impl Tables {
    /// Decode trusted built-in tables and compile their shared expressions.
    pub(in crate::highlight) fn load(
        patterns: &str,
        keywords: &str,
        modes: &str,
    ) -> Result<Self, Failure> {
        let patterns: Vec<Pattern> = serde_json::from_str(patterns)?;
        let patterns = patterns
            .iter()
            .map(|p| Regex::with_flags(&p.pattern, p.flags.as_str()))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            patterns,
            keywords: serde_json::from_str(keywords)?,
            modes: serde_json::from_str(modes)?,
        })
    }
}

/// One immutable publication, including an original preparation failure.
static TABLES: LazyLock<Result<Tables, Arc<dyn Error + Send + Sync>>> = LazyLock::new(|| {
    Tables::load(
        include_str!("../../../assets/highlighting/patterns.json"),
        include_str!("../../../assets/highlighting/keywords.json"),
        include_str!("../../../assets/highlighting/modes.json"),
    )
    .map_err(Arc::from)
});

/// The shared fallible grammar publication.
pub(super) fn tables() -> Result<&'static Tables, Failure> {
    TABLES
        .as_ref()
        .map_err(|error| Box::new(Arc::clone(error)) as Failure)
}

/// A surface's ordered registry over shared grammar tables.
pub(in crate::highlight) struct Engine<'a> {
    /// Immutable expression and mode owner.
    pub tables: &'a Tables,
    /// Surface-specific registration order and membership.
    pub languages: &'a [Language],
    /// Illegal-empty embedded-buffer policy for this grammar version.
    pub profile: HighlightProfile,
}

impl Engine<'_> {
    /// Find an admitted canonical name or alias.
    fn language(&self, name: &str) -> Option<&Language> {
        languages::lookup(self.languages, name)
    }

    /// Evaluate one grammar, optionally resuming invocation-owned embedded frames.
    pub(in crate::highlight) fn evaluate(
        &self,
        language: &Language,
        text: &str,
        continuation: Option<Stack>,
        illegals: IllegalPolicy,
    ) -> Evaluation {
        let Some(root) = language.root else {
            return Evaluation {
                score: types::Score {
                    language: Some(language.name),
                    outcome: types::Relevance::Legal(0.),
                },
                stack: None,
            };
        };
        let mut run = Run {
            engine: self,
            language,
            stack: continuation.unwrap_or_else(|| Stack::new(root)),
            buffer: String::new(),
            relevance: 0.,
            hits: HashMap::new(),
            continuations: HashMap::new(),
        };
        let illegal = run.scan(text, illegals);
        Evaluation {
            score: types::Score {
                language: Some(language.name),
                outcome: if illegal {
                    types::Relevance::Illegal
                } else {
                    types::Relevance::Legal(run.relevance)
                },
            },
            stack: Some(run.stack),
        }
    }
}

/// Whether illegal matches reject or remain part of an embedded parse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::highlight) enum IllegalPolicy {
    /// Reject an automatically tested candidate.
    Reject,
    /// Preserve the named embedded parse.
    Ignore,
}

/// Invocation-local state for one grammar.
struct Run<'a> {
    /// Shared immutable interpreter input.
    engine: &'a Engine<'a>,
    /// Current candidate's case and root policy.
    language: &'a Language,
    /// Entered frames, each owning its delimiter.
    stack: Stack,
    /// Pending unclassified text.
    buffer: String,
    /// Relevance accumulated in evaluation order.
    relevance: f64,
    /// Hit limits private to this invocation.
    hits: KeywordHits,
    /// Distinct named embedded frame stacks.
    continuations: HashMap<String, Stack>,
}

impl Run<'_> {
    /// Currently active immutable mode.
    fn mode(&self) -> &Mode {
        &self.engine.tables.modes[self.stack.current.mode.0]
    }
}

/// Detect using the exact surface registry, returning stable ranked candidates.
pub(super) fn detect(profile: HighlightProfile, code: &str) -> Result<Vec<types::Score>, Failure> {
    Ok(Engine {
        tables: tables()?,
        languages: languages::registered(profile),
        profile,
    }
    .automatic(code, &[]))
}
