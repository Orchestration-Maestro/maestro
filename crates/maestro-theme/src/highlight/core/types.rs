//! Immutable grammar records and invocation-local parse state.
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};

/// Index of a shared immutable mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub(in crate::highlight) struct ModeId(pub usize);

/// Index of a compiled expression.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(transparent)]
pub(super) struct PatternId(pub usize);

/// An admitted language label and its recognition grammar.
pub(in crate::highlight) struct Language {
    /// Canonical label passed to the painter.
    pub name: &'static str,
    /// Recognition root; absent for explicitly admitted non-detecting labels.
    pub root: Option<ModeId>,
    /// Alternative explicit labels.
    pub aliases: &'static [&'static str],
    /// Base language preferred on an equal score.
    pub superset: Option<&'static str>,
    /// Whether keywords use lowercase lookup.
    pub fold: bool,
}

/// Action taken by one ordered matching rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RuleKind {
    /// Enter the identified mode.
    Begin(ModeId),
    /// Attempt to end the current mode or its parent.
    End,
    /// Reject a candidate unless embedded parsing ignores illegals.
    Illegal,
}

/// A compiled rule's immutable expression identity and action.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rule {
    /// Expression executed by the matcher.
    pub pattern: PatternId,
    /// Mode transition at the match.
    pub action: RuleKind,
    /// Remaining alternatives when this begin can be ignored without advancing.
    #[serde(skip)]
    pub resumption: OnceLock<Result<super::matcher::MultiRegex, super::SharedFailure>>,
}

/// Ownership of a boundary's text.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Boundary {
    /// Include in the entered or ending mode.
    Include,
    /// Leave in the parent buffer.
    Exclude,
    /// Reprocess in the newly active mode.
    Return,
}

/// Whether an entered mode owns its scoring buffer.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Processing {
    /// Flush and score normally.
    Normal,
    /// Continue buffering in the parent context.
    Skip,
}

/// Parent termination relationships.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ParentPolicy {
    /// End only this mode using its own expression.
    Local,
    /// Also terminate the enclosing mode.
    EndsParent,
    /// Accept the enclosing mode's terminator.
    WithParent,
    /// Both parent relationships apply.
    Both,
}

/// Admission or delimiter operation associated with a boundary.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Callback {
    /// No additional admission rule.
    None,
    /// Reject a begin immediately following a dot.
    NotDot,
    /// Admit only at the beginning of the original input.
    Shebang,
    /// Retain the first capture on this entered frame.
    Save,
    /// Retain the first nonempty of the first two captures.
    SaveEither,
    /// Require the capture retained by this frame.
    Same,
    /// Distinguish tags from generic and comparison expressions.
    Jsx,
}

/// Score-producing part of a boundary scope.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Scope {
    /// Score the delimiter normally.
    Plain,
    /// A classified delimiter contributes no keyword score.
    Wrapped,
    /// Score only unclassified capture indexes.
    Captures(Vec<usize>),
}

/// Embedded-language selection attached to one mode.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SubLanguage {
    /// Score host keywords.
    None,
    /// Resume the named embedded grammar across buffers.
    Named(String),
    /// Detect among the admitted subset, or the whole profile when empty.
    Automatic(Vec<String>),
}

/// Immutable score-producing grammar data.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Mode {
    /// Awarded when this mode ends normally.
    pub relevance: f64,
    /// Pre-admission rule.
    pub before: Callback,
    /// Begin-boundary callback.
    pub on_begin: Callback,
    /// End-boundary callback.
    pub on_end: Callback,
    /// Ordered begin, end and illegal expressions.
    pub rules: Vec<Rule>,
    /// Combined terminators prepared before publishing this mode.
    #[serde(skip)]
    pub matcher: OnceLock<Result<super::matcher::MultiRegex, super::SharedFailure>>,
    /// This mode's own end expression.
    pub end: Option<PatternId>,
    /// Shared keyword weight table.
    pub keywords: Option<usize>,
    /// Expression identifying words in a scoring buffer.
    pub keyword_pattern: Option<PatternId>,
    /// Scoring policy of a beginning delimiter.
    pub begin_scope: Scope,
    /// Scoring policy of an ending delimiter.
    pub end_scope: Scope,
    /// Mode entered after ending this mode.
    pub starts: Option<ModeId>,
    /// Number of begin rules preceding end and illegal rules.
    pub begin_rule_count: usize,
    /// Beginning delimiter ownership.
    pub begin: Boundary,
    /// Ending delimiter ownership.
    pub end_boundary: Boundary,
    /// Buffer processing policy.
    pub processing: Processing,
    /// Relationship to enclosing mode endings.
    pub parent: ParentPolicy,
    /// Embedded-language policy.
    pub sub: SubLanguage,
}

/// One entered mode, with callback state private to this entry.
#[derive(Debug)]
pub(in crate::highlight) struct Frame {
    /// Immutable mode identity.
    pub mode: ModeId,
    /// Delimiter captured on entry, independent of other entries.
    pub delimiter: Option<String>,
}

/// Current frame and its enclosing frames, never shared between queries.
#[derive(Debug)]
pub(in crate::highlight) struct Stack {
    /// Always-present active mode.
    pub current: Frame,
    /// Enclosing modes in entry order.
    pub parents: Vec<Frame>,
}

impl Stack {
    /// Start at one language's root.
    pub fn new(root: ModeId) -> Self {
        Self {
            current: Frame {
                mode: root,
                delimiter: None,
            },
            parents: Vec::new(),
        }
    }

    /// Enter a child, preserving its enclosing frame's callback state.
    pub fn enter(&mut self, frame: Frame) {
        self.parents
            .push(std::mem::replace(&mut self.current, frame));
    }
}

/// Observable candidate result, without token emission or retained mutable state.
#[derive(Debug)]
pub(in crate::highlight) struct Score {
    /// Canonical registration name; absent for the plain candidate.
    pub language: Option<&'static str>,
    /// Accumulated relevance or an illegal candidate rejection.
    pub outcome: Relevance,
}

impl Score {
    /// Numerical ranking weight; rejected candidates score zero.
    pub fn relevance(&self) -> f64 {
        match self.outcome {
            Relevance::Legal(score) => score,
            Relevance::Illegal => 0.,
        }
    }

    /// Always-first, zero-relevance plain candidate.
    pub fn plain() -> Self {
        Self {
            language: None,
            outcome: Relevance::Legal(0.),
        }
    }
}

/// A candidate's score plus its invocation-owned continuation.
pub(in crate::highlight) struct Evaluation {
    /// Observable candidate fields.
    pub score: Score,
    /// Frames retained only for a named embedded continuation.
    pub stack: Option<Stack>,
}

/// Per-word hit counters for one candidate evaluation.
pub(super) type KeywordHits = HashMap<String, u8>;

/// A candidate either accumulates relevance or rejects illegal syntax.
#[derive(Debug)]
pub(in crate::highlight) enum Relevance {
    /// Successfully parsed keyword and ended-mode relevance.
    Legal(f64),
    /// Rejected candidate, ranked at zero relevance.
    Illegal,
}
