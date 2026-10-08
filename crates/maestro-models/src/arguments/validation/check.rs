//! Ordered schema evaluation with isolated branch results and location marks.

use std::{
    borrow::Cow,
    collections::{BTreeSet, VecDeque},
};

use num_traits::ToPrimitive;
use serde_json::Value;

use super::{
    collections,
    references::{self, Context, Location},
    scalars,
};

/// Maximum number of distinct corrective messages retained per evaluation.
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

/// Completed child application: marks exist only for a valid branch.
pub(super) enum Outcome {
    /// Every assertion held; the locations the branch evaluated.
    Valid(Marks),
    /// At least one assertion failed; its ordered corrective messages.
    Invalid(Vec<String>),
}

#[derive(Default)]
/// Failures and evaluated locations accumulated by one schema application.
pub(super) struct Evaluation {
    /// Ordered corrective messages accumulated by this evaluation.
    pub errors: Vec<String>,
    /// Locations evaluated so far, visible to later unevaluated checks.
    pub marks: Marks,
}

impl Evaluation {
    /// Retain ordered distinct failures within the diagnostic limit.
    fn add_errors(&mut self, errors: impl IntoIterator<Item = String>) {
        for error in errors {
            if self.errors.len() == ERROR_LIMIT {
                break;
            }
            if !self.errors.contains(&error) {
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
        message: String,
    },
    /// Compare the number of successful children with contains bounds.
    Count {
        /// Active lower bound on the number of matching items.
        minimum: Option<f64>,
        /// Active upper bound on the number of matching items.
        maximum: Option<f64>,
        /// Keyword failure text rendered at the containing instance.
        message: String,
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

#[derive(Clone, Copy)]
/// Borrowed JSON location or property name being checked as a string.
pub(super) enum Instance<'a> {
    /// Existing argument value whose location can be compared by identity.
    Json(&'a Value),
    /// Object key exposed as a string for propertyNames assertions.
    Name(&'a str),
}

impl<'a> Instance<'a> {
    /// Compare instance locations for non-progress cycle detection.
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Json(left), Self::Json(right)) => std::ptr::eq(left, right),
            (Self::Name(left), Self::Name(right)) => std::ptr::eq(left, right),
            _ => false,
        }
    }

    /// Expose a JSON instance, materializing property names as strings.
    fn value(self) -> Cow<'a, Value> {
        match self {
            Self::Json(value) => Cow::Borrowed(value),
            Self::Name(name) => Cow::Owned(Value::String(name.to_owned())),
        }
    }
}

#[derive(Clone)]
/// One schema application retaining its context, instance and diagnostic path.
pub(super) struct Job<'a> {
    /// Schema location and live anchor bindings to apply.
    pub context: Context<'a>,
    /// Instance location whose validity is being determined.
    pub value: Instance<'a>,
    /// Slash-separated instance path used for corrective diagnostics.
    pub path: String,
}

impl<'a> Job<'a> {
    /// Original, unmodified schema location to apply.
    pub fn schema(&self) -> &'a Value {
        self.context.location.schema
    }

    /// Apply a lexically nested schema to a member instance.
    pub fn child(&self, schema: &'a Value, value: Instance<'a>, path: String) -> Self {
        Self {
            context: self.context.child(schema),
            value,
            path,
        }
    }

    /// Apply a lexically nested schema to the same instance.
    pub fn same_instance(&self, schema: &'a Value) -> Self {
        self.child(schema, self.value, self.path.clone())
    }

    /// Apply a resolved reference target to the same instance.
    pub fn resolved(&self, target: Location<'a>) -> Self {
        Self {
            context: self.context.follow(target),
            value: self.value,
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

/// Explicit evaluator continuation for one active schema and instance.
struct Frame<'a> {
    /// Active schema location and bindings used for cycle detection.
    context: Context<'a>,
    /// Active instance location used for cycle detection.
    instance: Instance<'a>,
    /// Remaining keyword work in diagnostic order.
    instructions: VecDeque<Instruction<'a>>,
    /// Child applications currently being evaluated or aggregated.
    pending: Option<Batch<'a>>,
    /// Accumulated failures and successful location marks for this frame.
    result: Evaluation,
    /// Diagnostic instance path retained across child evaluations.
    path: String,
}

impl<'a> Frame<'a> {
    /// Prepare an evaluation frame for one schema application.
    fn new(job: Job<'a>, root: &Location<'a>) -> Self {
        Self {
            instructions: instructions(&job, root).into(),
            context: job.context,
            instance: job.value,
            pending: None,
            result: Evaluation::default(),
            path: job.path,
        }
    }

    /// Take the next pending child without retaining its result yet.
    fn next_child(&mut self) -> Option<Job<'a>> {
        self.pending.as_mut()?.jobs.pop_front()
    }

    /// Merge completed children or schedule the selected conditional branch.
    fn finish_batch(&mut self) {
        if let Some(batch) = self.pending.take() {
            self.pending = apply(batch, &self.path, &mut self.result);
        }
    }
}

/// Return ordered failures while evaluating schemas without recursive frames.
pub(super) fn check(schema: &Value, value: &Value) -> Vec<String> {
    let root = Location::root(schema);
    let job = Job {
        context: Context::root(root.clone()),
        value: Instance::Json(value),
        path: String::new(),
    };
    let mut frames = vec![Frame::new(job, &root)];
    while let Some(frame) = frames.last_mut() {
        if let Some(job) = frame.next_child() {
            if frames
                .iter()
                .any(|frame| frame.instance.same(job.value) && frame.context.same(&job.context))
            {
                reject_cycle(&mut frames, &job.path);
            } else {
                frames.push(Frame::new(job, &root));
            }
            continue;
        }
        frame.finish_batch();
        if frame.pending.is_some() {
            continue;
        }
        match frame.instructions.pop_front() {
            Some(Instruction::Errors(errors)) => frame.result.add_errors(errors),
            Some(Instruction::Children(batch)) => frame.pending = Some(batch),
            Some(Instruction::Unevaluated(job)) => frame
                .instructions
                .extend(collections::unevaluated(&job, &frame.result.marks)),
            None => {
                let Some(completed) = frames.pop() else { break };
                let Some(parent) = frames.last_mut() else {
                    return completed.result.errors;
                };
                if let Some(batch) = &mut parent.pending {
                    batch.results.push(completed.result.finish());
                }
            }
        }
    }
    Vec::new()
}

/// Schedule keyword assertions in their diagnostic order.
fn instructions<'a>(job: &Job<'a>, root: &Location<'a>) -> Vec<Instruction<'a>> {
    let schema = job.schema();
    let value = job.value.value();
    let mut instructions = vec![Instruction::Errors(scalars::type_errors(
        schema, &value, &job.path,
    ))];
    collections::object(job, &mut instructions);
    collections::array(job, &mut instructions);
    instructions.push(Instruction::Errors(scalars::string_errors(
        schema, &value, &job.path,
    )));
    instructions.push(Instruction::Errors(scalars::number_errors(
        schema, &value, &job.path,
    )));
    instructions.extend(references::instructions(root, job));
    instructions.push(Instruction::Errors(scalars::literal_errors(
        schema, &value, &job.path,
    )));
    conditional(job, &mut instructions);
    if let Some(schema) = schema.get("not").filter(|schema| is_schema(schema)) {
        instructions.push(Instruction::Children(Batch::new(
            Mode::Not,
            [job.same_instance(schema)],
        )));
    }
    for (keyword, mode) in [
        ("allOf", Mode::All),
        ("anyOf", Mode::Any),
        ("oneOf", Mode::One),
    ] {
        if let Some(schemas) = schema.get(keyword).and_then(schema_array) {
            let jobs = schemas.iter().map(|schema| job.same_instance(schema));
            instructions.push(Instruction::Children(Batch::new(mode, jobs)));
        }
    }
    instructions.push(Instruction::Unevaluated(job.clone()));
    instructions
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
                result.add_errors([scalars::render(path, &message)]);
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

/// Record a false-schema failure for a non-progressing child.
fn reject_cycle(frames: &mut [Frame<'_>], path: &str) {
    if let Some(batch) = frames.last_mut().and_then(|frame| frame.pending.as_mut()) {
        batch.results.push(Outcome::Invalid(vec![scalars::render(
            path,
            "schema is false",
        )]));
    }
}

/// Recognize the admitted object and boolean schema forms.
pub(super) fn is_schema(value: &Value) -> bool {
    value.is_object() || value.is_boolean()
}

/// Borrow a list only when every member is an admitted schema.
pub(super) fn schema_array(value: &Value) -> Option<&[Value]> {
    value
        .as_array()
        .filter(|schemas| schemas.iter().all(is_schema))
        .map(Vec::as_slice)
}
