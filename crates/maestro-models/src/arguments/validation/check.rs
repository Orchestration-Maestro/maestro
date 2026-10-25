//! Ordered schema evaluation with isolated branch results and location marks.

mod code;
mod frame;
mod operand;
mod program;

pub(super) use code::Code;
pub(super) use frame::{authored_candidate, candidate, check, check_owned};
pub(super) use operand::Instance;
pub(super) use program::Phase;

use std::{
    borrow::Cow,
    collections::{BTreeSet, VecDeque},
};

use num_traits::ToPrimitive;
use serde_json::{Map, Value};

use super::{
    references::{Context, Location},
    scalars,
};

/// Maximum number of corrective messages retained per evaluation.
const ERROR_LIMIT: usize = 8;

#[derive(Default)]
/// Locations a schema branch evaluated successfully at one instance.
pub(super) struct Marks {
    /// Successfully evaluated properties at this instance only.
    pub keys: BTreeSet<String>,
    /// Successfully evaluated array positions at this instance only.
    pub indices: BTreeSet<usize>,
}

impl Marks {
    /// Union the locations another valid branch evaluated.
    fn merge(&mut self, mut other: Self) {
        self.keys.append(&mut other.keys);
        self.indices.append(&mut other.indices);
    }

    /// Record a successfully evaluated property or array index.
    fn add(&mut self, mark: Mark) {
        match mark {
            Mark::Key(key) => self.keys.insert(key),
            Mark::Index(index) => self.indices.insert(index),
        };
    }
}

/// A corrective line retaining whether its refinement index makes it distinct.
#[derive(PartialEq)]
pub(super) enum Failure {
    /// Ordinary duplicate corrective lines are suppressed.
    Ordinary(String),
    /// Separate refinement checks remain separate failures, even with equal text.
    Refinement(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Ordinary(message)
    }
}

impl Failure {
    /// Render a retained failure at the external diagnostic boundary.
    fn into_message(self) -> String {
        match self {
            Self::Ordinary(message) | Self::Refinement(message) => message,
        }
    }
}

/// Completed child application: marks exist only for a valid branch.
pub(super) enum Outcome {
    /// Every assertion held; the locations the branch evaluated.
    Valid(Marks),
    /// At least one assertion failed; its ordered corrective messages.
    Invalid(Vec<Failure>),
}

#[derive(Default)]
/// Failures and evaluated locations accumulated by one schema application.
pub(super) struct Evaluation {
    /// Ordered corrective messages accumulated by this evaluation.
    pub errors: Vec<Failure>,
    /// Locations evaluated so far, visible to later unevaluated checks.
    pub marks: Marks,
}

impl Evaluation {
    /// Retain ordered ordinary failures once, without merging indexed refinements.
    fn add_errors(&mut self, errors: impl IntoIterator<Item = impl Into<Failure>>) {
        for error in errors {
            let error = error.into();
            if self.errors.len() == ERROR_LIMIT {
                break;
            }
            if matches!(error, Failure::Refinement(_)) || !self.errors.contains(&error) {
                self.errors.push(error);
            }
        }
    }

    /// Export marks only when no assertion failed.
    fn finish(self) -> Outcome {
        if self.errors.is_empty() {
            Outcome::Valid(self.marks)
        } else {
            Outcome::Invalid(self.errors)
        }
    }
}

/// Deferred work preserving keyword order within an evaluation frame.
pub(super) enum Instruction<'a> {
    /// Apply an author guard before literals and compositions.
    Guard,
    /// Resolve a keyword against the live operand when its source phase is reached.
    Phase(Phase),
    /// Admit a dependency only after the preceding dependency has completed.
    Dependency(Job<'a>, &'static str, String),
    /// Append already computed scalar or cardinality failures.
    Errors(Vec<String>),
    /// Evaluate child schemas before merging their outcomes.
    Children(Batch<'a>),
    /// Defer remaining-member checks until earlier marks are available.
    Unevaluated(Job<'a>),
}

/// Policy for combining child validity, diagnostics and evaluated locations.
pub(super) enum Mode<'a> {
    /// Require every child to succeed before exporting branch marks.
    All,
    /// Accept at least one child and union all successful branch marks.
    Any,
    /// Accept exactly one child and export only its marks.
    One,
    /// Associate each successful child with its containing member location.
    Members(Vec<Mark>),
    /// Report failed member checks with one containing-instance message.
    Aggregate {
        /// Containing locations corresponding to the child evaluations.
        marks: Vec<Mark>,
        /// Keyword failure text rendered at the containing instance.
        message: &'static str,
    },
    /// Stop as soon as one member satisfies the existence rule.
    Some,
    /// Compare the number of successful children with contains bounds.
    Count {
        /// Active lower bound on the number of matching items.
        minimum: Option<f64>,
        /// Active upper bound on the number of matching items.
        maximum: Option<f64>,
        /// Keyword failure text rendered at the containing instance.
        message: Cow<'static, str>,
    },
    /// Aggregate invalid property-name evaluations by their original names.
    Names(Vec<String>),
    /// Invert child validity without exporting its errors or marks.
    Not,
    /// Evaluate a condition before choosing one of two branch jobs.
    If {
        /// Branch job selected when the condition succeeds.
        then: Job<'a>,
        /// Branch job selected when the condition fails.
        otherwise: Job<'a>,
    },
    /// Merge the selected branch with the marks of a successful condition.
    Selected(Option<Marks>),
}

/// Containing member location that a successful child can evaluate.
pub(super) enum Mark {
    /// Property name at the containing object location.
    Key(String),
    /// Position at the containing array location.
    Index(usize),
}

#[derive(Clone)]
/// One schema application retaining its context, instance and diagnostic path.
pub(super) struct Job<'a> {
    /// Schema location and live anchor bindings to apply.
    pub context: Context<'a>,
    /// Instance location whose validity is being determined.
    pub value: Instance,
    /// Slash-separated instance path used for corrective diagnostics.
    pub path: String,
}

impl<'a> Job<'a> {
    /// Original, unmodified schema location to apply.
    pub fn schema(&self) -> &'a Value {
        self.context.location.schema
    }

    /// Apply a lexically nested schema to a member instance.
    pub fn child(&self, schema: &'a Value, value: Instance, path: String) -> Self {
        Self {
            context: self.context.child(schema),
            value,
            path,
        }
    }

    /// Apply a lexically nested schema to the same instance.
    pub fn same_instance(&self, schema: &'a Value) -> Self {
        self.child(schema, Instance::Same, self.path.clone())
    }

    /// Apply a resolved reference target to the same instance.
    pub fn resolved(&self, target: Location<'a>) -> Self {
        Self {
            context: self.context.follow(target),
            value: Instance::Same,
            path: self.path.clone(),
        }
    }
}

/// Child applications and their outcomes awaiting policy-specific aggregation.
pub(super) struct Batch<'a> {
    /// Policy controlling validity, messages and exported locations.
    pub mode: Mode<'a>,
    /// Pending child applications in evaluation order.
    pub jobs: VecDeque<Job<'a>>,
    /// Completed child outcomes in the same order as their applications.
    pub results: Vec<Outcome>,
}

impl<'a> Batch<'a> {
    /// Queue child applications to be combined under a policy.
    pub fn new(mode: Mode<'a>, jobs: impl IntoIterator<Item = Job<'a>>) -> Self {
        Self {
            mode,
            jobs: jobs.into_iter().collect(),
            results: Vec::new(),
        }
    }
}

/// Merge branch outcomes according to their composition or membership policy.
fn apply<'a>(batch: Batch<'a>, path: &str, result: &mut Evaluation) -> Option<Batch<'a>> {
    let Batch { mode, results, .. } = batch;
    match mode {
        Mode::If { .. } | Mode::Selected(_) | Mode::Not => {
            return apply_conditional(mode, results, path, result);
        }
        Mode::All | Mode::Any | Mode::One => combine(&mode, results, path, result),
        Mode::Members(marks) => {
            for (mark, outcome) in marks.into_iter().zip(results) {
                match outcome {
                    Outcome::Valid(_) => result.marks.add(mark),
                    Outcome::Invalid(errors) => result.add_errors(errors),
                }
            }
        }
        Mode::Some => {
            if !results
                .iter()
                .any(|outcome| matches!(outcome, Outcome::Valid(_)))
            {
                result.add_errors([scalars::render(path, "must contain at least 1 valid item")]);
            }
        }
        Mode::Aggregate { .. } | Mode::Count { .. } | Mode::Names(_) => {
            apply_collection(mode, &results, path, result);
        }
    }
    None
}

/// Combine sibling branches; only a valid composition exports passing branch marks.
fn combine(mode: &Mode<'_>, results: Vec<Outcome>, path: &str, result: &mut Evaluation) {
    let passing = results
        .iter()
        .filter(|outcome| matches!(outcome, Outcome::Valid(_)))
        .count();
    let (valid, summary) = match mode {
        Mode::All => (passing == results.len(), None),
        Mode::Any => (passing > 0, Some("must match a schema in anyOf")),
        Mode::One => (passing == 1, Some("must match exactly one schema in oneOf")),
        _ => return,
    };
    let report = !valid && (passing == 0 || matches!(mode, Mode::All));
    for outcome in results {
        match outcome {
            Outcome::Valid(marks) if valid => result.marks.merge(marks),
            Outcome::Invalid(errors) if report => result.add_errors(errors),
            _ => {}
        }
    }
    if let Some(summary) = summary.filter(|_| !valid) {
        result.add_errors([scalars::render(path, summary)]);
    }
}

/// Aggregate member, count or property-name failures at the containing instance.
fn apply_collection(mode: Mode<'_>, results: &[Outcome], path: &str, result: &mut Evaluation) {
    let valid = |outcome: &Outcome| matches!(outcome, Outcome::Valid(_));
    let passing = results.iter().filter(|outcome| valid(outcome)).count();
    match mode {
        Mode::Aggregate { marks, message } => {
            let members = marks.into_iter().zip(results);
            for (mark, _) in members.filter(|(_, outcome)| valid(outcome)) {
                result.marks.add(mark);
            }
            if passing != results.len() {
                result.add_errors([scalars::render(path, message)]);
            }
        }
        Mode::Count {
            minimum,
            maximum,
            message,
        } => {
            if passing.to_f64().is_some_and(|passing| {
                minimum.is_some_and(|limit| passing < limit)
                    || maximum.is_some_and(|limit| passing > limit)
            }) {
                result.add_errors([scalars::render(path, &message)]);
            }
        }
        Mode::Names(names) => {
            let invalid: Vec<_> = names
                .iter()
                .zip(results)
                .filter(|(_, outcome)| !valid(outcome))
                .map(|(name, _)| name.as_str())
                .collect();
            if !invalid.is_empty() {
                result.add_errors([scalars::render(
                    path,
                    &format!("property names {} are invalid", invalid.join(", ")),
                )]);
            }
        }
        _ => {}
    }
}

/// Schedule the condition with permissive defaults for absent branches.
fn conditional<'a>(job: &Job<'a>, instructions: &mut Vec<Instruction<'a>>) {
    /// Permissive branch schema used when then or else is absent.
    const TRUE: Value = Value::Bool(true);
    let schema = job.schema();
    if let Some(condition) = schema.get("if").filter(|schema| is_schema(schema)) {
        let branch = |keyword: &str| {
            let schema = schema.get(keyword).filter(|schema| is_schema(schema));
            job.same_instance(schema.unwrap_or(&TRUE))
        };
        let mode = Mode::If {
            then: branch("then"),
            otherwise: branch("else"),
        };
        instructions.push(Instruction::Children(Batch::new(
            mode,
            [job.same_instance(condition)],
        )));
    }
}

/// Select a conditional branch or invert validity without leaking failed marks.
fn apply_conditional<'a>(
    mode: Mode<'a>,
    results: Vec<Outcome>,
    path: &str,
    result: &mut Evaluation,
) -> Option<Batch<'a>> {
    let outcome = results.into_iter().next()?;
    match (mode, outcome) {
        (Mode::Not, Outcome::Valid(_)) => {
            result.add_errors([scalars::render(path, "must not be valid")]);
        }
        (Mode::If { then, otherwise }, outcome) => {
            let (branch, condition) = match outcome {
                Outcome::Valid(marks) => (then, Some(marks)),
                Outcome::Invalid(_) => (otherwise, None),
            };
            return Some(Batch::new(Mode::Selected(condition), [branch]));
        }
        (Mode::Selected(condition), Outcome::Valid(marks)) => {
            if let Some(condition) = condition {
                result.marks.merge(condition);
            }
            result.marks.merge(marks);
        }
        (Mode::Selected(condition), Outcome::Invalid(errors)) => {
            let then = condition.is_some();
            if !then {
                result.add_errors(errors);
            }
            let keyword = if then { "then" } else { "else" };
            result.add_errors([scalars::render(
                path,
                &format!("must match \"{keyword}\" schema"),
            )]);
        }
        _ => {}
    }
    None
}

/// Recognize the admitted object and boolean schema forms.
pub(super) fn is_schema(value: &Value) -> bool {
    value.is_object() || value.is_boolean()
}

/// Borrow an object only when every member is an admitted schema.
pub(super) fn schema_map(value: Option<&Value>) -> Option<&Map<String, Value>> {
    value
        .and_then(Value::as_object)
        .filter(|schemas| schemas.values().all(is_schema))
}

/// Borrow a list only when every member is an admitted schema.
pub(super) fn schema_array(value: &Value) -> Option<&[Value]> {
    value
        .as_array()
        .filter(|schemas| schemas.iter().all(is_schema))
        .map(Vec::as_slice)
}

#[cfg(test)]
mod tests;
