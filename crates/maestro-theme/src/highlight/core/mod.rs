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
use std::sync::{Arc, LazyLock, OnceLock};
use types::{Evaluation, KeywordHits, Language, Mode, ModeId, Stack};

/// Owned preparation failures, retaining their native cause.
pub(in crate::highlight) type Failure = Box<dyn Error + Send + Sync>;

/// Cloneable preparation failures retained by once-only publications.
type SharedFailure = Arc<dyn Error + Send + Sync>;

/// Borrow one publication while preserving its original failure.
fn published<T>(result: &Result<T, SharedFailure>) -> Result<&T, Failure> {
    result
        .as_ref()
        .map_err(|error| Box::new(Arc::clone(error)) as Failure)
}

/// Immutable shared patterns, keyword tables and modes.
pub(in crate::highlight) struct Tables {
    /// Native expressions compiled once.
    patterns: Vec<OnceLock<Result<Regex, SharedFailure>>>,
    /// Original expressions and capture counts shared by both profiles.
    sources: Vec<Expression>,
    /// Tokenizer shared by capture counting and backreference relocation.
    rewriter: Regex,
    /// Each surface prepares only its own candidate graph at first detection.
    profiles: [OnceLock<Result<(), SharedFailure>>; 2],
    /// Interned keyword weights.
    keywords: Vec<HashMap<String, f64>>,
    /// Immutable mode graph.
    modes: Vec<Mode>,
}

/// An expression and its native matching flags.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expression {
    /// Source expression.
    pattern: String,
    /// Native expression flags.
    flags: String,
    /// Native capture count before adding an alternative wrapper.
    #[serde(skip)]
    captures: usize,
}

impl Tables {
    /// Decode trusted built-in tables and compile their shared expressions.
    pub(in crate::highlight) fn load(
        patterns: &str,
        keywords: &str,
        modes: &str,
    ) -> Result<Self, Failure> {
        let mut sources: Vec<Expression> = serde_json::from_str(patterns)?;
        let rewriter = Regex::new(r"\[(?:[^\\\]]|\\.)*\]|\(\??|\\([1-9][0-9]*)|\\.")?;
        for source in &mut sources {
            source.captures = rewriter
                .find_iter(&source.pattern)
                .filter(|token| token.as_str(&source.pattern) == "(")
                .count();
        }
        Ok(Self {
            patterns: (0..sources.len()).map(|_| OnceLock::new()).collect(),
            sources,
            rewriter,
            profiles: std::array::from_fn(|_| OnceLock::new()),
            keywords: serde_json::from_str(keywords)?,
            modes: serde_json::from_str(modes)?,
        })
    }

    /// Prepare one profile once without duplicating shared modes or patterns.
    fn prepare(&self, profile: HighlightProfile) -> Result<(), Failure> {
        let index = match profile {
            HighlightProfile::ExportViewer => 0,
            HighlightProfile::CodeBlock => 1,
        };
        published(self.profiles[index].get_or_init(|| {
            self.prepare_roots(languages::registered(profile).iter().filter_map(|l| l.root))
                .map_err(Arc::from)
        }))
        .copied()
    }

    /// Compile only modes reachable from these candidate roots.
    pub(in crate::highlight) fn prepare_roots(
        &self,
        roots: impl IntoIterator<Item = ModeId>,
    ) -> Result<(), Failure> {
        matcher::prepare(self, roots)
    }

    /// Compile a standalone expression only for a keyword or end consumer.
    fn prepare_pattern(&self, index: usize) -> Result<(), Failure> {
        published(self.patterns[index].get_or_init(|| {
            let source = &self.sources[index];
            Regex::with_flags(&source.pattern, source.flags.as_str())
                .map_err(|error| Arc::from(Box::new(error) as Failure))
        }))
        .map(|_| ())
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
pub(super) fn tables(profile: HighlightProfile) -> Result<&'static Tables, Failure> {
    let tables = published(&TABLES)?;
    tables.prepare(profile)?;
    Ok(tables)
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
        tables: tables(profile)?,
        languages: languages::registered(profile),
        profile,
    }
    .automatic(code, &[]))
}
