//! Ordered schema evaluation with isolated branch results and location marks.

use std::{
    borrow::Cow,
    collections::{BTreeSet, VecDeque},
};

use num_traits::ToPrimitive;
use serde_json::Value;

use super::{collections, references, scalars};

/// Maximum number of distinct corrective messages retained per evaluation.
const ERROR_LIMIT: usize = 8;

#[derive(Default)]
/// Failures and evaluated locations for one instance and schema branch.
pub(super) struct Evaluation {
    /// Ordered corrective messages accumulated by this evaluation.
    pub errors: Vec<String>,
    /// Successfully evaluated properties at this instance only.
    pub keys: BTreeSet<String>,
    /// Successfully evaluated array positions at this instance only.
    pub indices: BTreeSet<usize>,
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

    /// Append child errors and union its evaluated locations.
    fn merge(&mut self, mut other: Self) {
        self.add_errors(other.errors);
        self.keys.append(&mut other.keys);
        self.indices.append(&mut other.indices);
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
    /// Merge the selected conditional branch with eligible condition marks.
    Selected {
        /// Whether the selected branch came from a successful condition.
        then: bool,
        /// Condition outcome retained until the selected branch finishes.
        condition: Evaluation,
    },
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
/// One schema application retaining its instance and diagnostic path.
pub(super) struct Job<'a> {
    /// Original, unmodified schema location to apply.
    pub schema: &'a Value,
    /// Instance location whose validity is being determined.
    pub value: Instance<'a>,
    /// Slash-separated instance path used for corrective diagnostics.
    pub path: String,
}

/// Child applications and their outcomes awaiting policy-specific aggregation.
pub(super) struct Batch<'a> {
    /// Policy controlling validity, messages and exported locations.
    pub mode: Mode<'a>,
    /// Pending child applications in evaluation order.
    pub jobs: VecDeque<Job<'a>>,
    /// Completed child outcomes in the same order as their applications.
    pub results: Vec<Evaluation>,
}

/// Explicit evaluator continuation for one active schema and instance.
struct Frame<'a> {
    /// Active original schema location used for cycle detection.
    schema: &'a Value,
    /// Active instance location used for cycle detection.
    instance: Instance<'a>,
    /// Resource and anchor bindings that distinguish active evaluations.
    scope: references::ScopeIdentity<'a>,
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
    /// Prepare an evaluation frame using the current resource scopes.
    fn new(job: Job<'a>, root: &'a Value, scopes: &[&'a Value]) -> Self {
        Self {
            schema: job.schema,
            instance: job.value,
            scope: references::scope_identity(root, scopes),
            instructions: instructions(&job, root, scopes).into(),
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
    let mut frames = vec![Frame::new(
        Job {
            schema,
            value: Instance::Json(value),
            path: String::new(),
        },
        schema,
        &[schema],
    )];
    while let Some(frame) = frames.last_mut() {
        if let Some(job) = frame.next_child() {
            let scopes: Vec<_> = frames
                .iter()
                .map(|frame| frame.schema)
                .chain(std::iter::once(job.schema))
                .collect();
            let scope = references::scope_identity(schema, &scopes);
            if frames.iter().any(|frame| {
                std::ptr::eq(frame.schema, job.schema)
                    && frame.instance.same(job.value)
                    && frame.scope == scope
            }) {
                reject_cycle(&mut frames, &job.path);
                continue;
            }
            frames.push(Frame::new(job, schema, &scopes));
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
                .extend(collections::unevaluated(&job, &frame.result)),
            None => {
                let Some(completed) = frames.pop() else { break };
                let Some(parent) = frames.last_mut() else {
                    return completed.result.errors;
                };
                if let Some(batch) = &mut parent.pending {
                    batch.results.push(completed.result);
                }
            }
        }
    }
    Vec::new()
}

/// Schedule keyword assertions in their diagnostic order.
fn instructions<'a>(job: &Job<'a>, root: &'a Value, scopes: &[&'a Value]) -> Vec<Instruction<'a>> {
    let value = job.value.value();
    let mut instructions = vec![Instruction::Errors(scalars::type_errors(
        job.schema, &value, &job.path,
    ))];
    collections::object(job, &mut instructions);
    collections::array(job, &mut instructions);
    instructions.push(Instruction::Errors(scalars::string_errors(
        job.schema, &value, &job.path,
    )));
    instructions.push(Instruction::Errors(scalars::number_errors(
        job.schema, &value, &job.path,
    )));
    instructions.extend(references::instructions(root, job, scopes));
    instructions.push(Instruction::Errors(scalars::literal_errors(
        job.schema, &value, &job.path,
    )));
    conditional(job, &mut instructions);
    if let Some(schema) = job.schema.get("not").filter(|schema| is_schema(schema)) {
        instructions.push(Instruction::Children(Batch {
            mode: Mode::Not,
            jobs: [Job {
                schema,
                ..job.clone()
            }]
            .into(),
            results: Vec::new(),
        }));
    }
    for (keyword, mode) in [
        ("allOf", Mode::All),
        ("anyOf", Mode::Any),
        ("oneOf", Mode::One),
    ] {
        if let Some(schemas) = job.schema.get(keyword).and_then(schema_array) {
            let jobs = schemas
                .iter()
                .map(|schema| Job {
                    schema,
                    value: job.value,
                    path: job.path.clone(),
                })
                .collect();
            instructions.push(Instruction::Children(Batch {
                mode,
                jobs,
                results: Vec::new(),
            }));
        }
    }
    instructions.push(Instruction::Unevaluated(job.clone()));
    instructions
}

/// Merge branch outcomes according to their composition or membership policy.
fn apply<'a>(batch: Batch<'a>, path: &str, result: &mut Evaluation) -> Option<Batch<'a>> {
    if matches!(
        batch.mode,
        Mode::If { .. } | Mode::Selected { .. } | Mode::Not
    ) {
        return apply_conditional(batch, path, result);
    }
    if apply_collection(&batch, path, result) {
        return None;
    }
    let passing = batch
        .results
        .iter()
        .filter(|result| result.errors.is_empty())
        .count();
    let valid = match &batch.mode {
        Mode::All | Mode::Members(_) => passing == batch.results.len(),
        Mode::Any => passing > 0,
        Mode::One => passing == 1,
        _ => false,
    };
    if let Mode::Members(names) = batch.mode {
        for (name, child) in names.into_iter().zip(batch.results) {
            if child.errors.is_empty() {
                add_mark(&name, result);
            }
            result.add_errors(child.errors);
        }
        return None;
    }
    let summary = match batch.mode {
        Mode::Any => Some("must match a schema in anyOf"),
        Mode::One => Some("must match exactly one schema in oneOf"),
        _ => None,
    };
    for child in batch.results {
        if valid && child.errors.is_empty() {
            result.merge(child);
        } else if !valid && (passing == 0 || matches!(batch.mode, Mode::All)) {
            result.add_errors(child.errors);
        }
    }
    if !valid && let Some(summary) = summary {
        result.add_errors([scalars::render(path, summary)]);
    }
    None
}

/// Aggregate member, count or property-name failures at the containing instance.
fn apply_collection(batch: &Batch<'_>, path: &str, result: &mut Evaluation) -> bool {
    let passing = batch
        .results
        .iter()
        .filter(|child| child.errors.is_empty())
        .count();
    match &batch.mode {
        Mode::Aggregate { marks, message } => {
            for (mark, child) in marks.iter().zip(&batch.results) {
                if child.errors.is_empty() {
                    add_mark(mark, result);
                }
            }
            if passing != batch.results.len() {
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
                result.add_errors([scalars::render(path, message)]);
            }
        }
        Mode::Names(names) => {
            let invalid: Vec<_> = names
                .iter()
                .zip(&batch.results)
                .filter(|(_, child)| !child.errors.is_empty())
                .map(|(name, _)| name.as_str())
                .collect();
            if !invalid.is_empty() {
                result.add_errors([scalars::render(
                    path,
                    &format!("property names {} are invalid", invalid.join(", ")),
                )]);
            }
        }
        _ => return false,
    }
    true
}

/// Record a successfully evaluated property or array index.
fn add_mark(mark: &Mark, result: &mut Evaluation) {
    match mark {
        Mark::Key(key) => {
            result.keys.insert(key.clone());
        }
        Mark::Index(index) => {
            result.indices.insert(*index);
        }
    }
}

/// Schedule the condition with permissive defaults for absent branches.
fn conditional<'a>(job: &Job<'a>, instructions: &mut Vec<Instruction<'a>>) {
    /// Permissive branch schema used when then or else is absent.
    const TRUE: Value = Value::Bool(true);
    if let Some(schema) = job.schema.get("if").filter(|schema| is_schema(schema)) {
        let then = Job {
            schema: job
                .schema
                .get("then")
                .filter(|schema| is_schema(schema))
                .unwrap_or(&TRUE),
            ..job.clone()
        };
        let otherwise = Job {
            schema: job
                .schema
                .get("else")
                .filter(|schema| is_schema(schema))
                .unwrap_or(&TRUE),
            ..job.clone()
        };
        let jobs = [Job {
            schema,
            ..job.clone()
        }]
        .into();
        instructions.push(Instruction::Children(Batch {
            mode: Mode::If { then, otherwise },
            jobs,
            results: Vec::new(),
        }));
    }
}

/// Select a conditional branch or invert validity without leaking failed marks.
fn apply_conditional<'a>(
    batch: Batch<'a>,
    path: &str,
    result: &mut Evaluation,
) -> Option<Batch<'a>> {
    let child = batch.results.into_iter().next()?;
    match batch.mode {
        Mode::Not => {
            if child.errors.is_empty() {
                result.add_errors([scalars::render(path, "must not be valid")]);
            }
        }
        Mode::If { then, otherwise } => {
            let selected_then = child.errors.is_empty();
            let jobs = [if selected_then { then } else { otherwise }].into();
            return Some(Batch {
                mode: Mode::Selected {
                    then: selected_then,
                    condition: child,
                },
                jobs,
                results: Vec::new(),
            });
        }
        Mode::Selected {
            then,
            mut condition,
        } => {
            if child.errors.is_empty() {
                if then {
                    condition.errors.clear();
                    result.merge(condition);
                }
                result.merge(child);
            } else {
                if !then {
                    result.add_errors(child.errors);
                }
                let keyword = if then { "then" } else { "else" };
                result.add_errors([scalars::render(
                    path,
                    &format!("must match \"{keyword}\" schema"),
                )]);
            }
        }
        _ => {}
    }
    None
}

/// Record a false-schema failure for a non-progressing child.
fn reject_cycle(frames: &mut [Frame<'_>], path: &str) {
    if let Some(batch) = frames.last_mut().and_then(|frame| frame.pending.as_mut()) {
        batch.results.push(Evaluation {
            errors: vec![scalars::render(path, "schema is false")],
            ..Evaluation::default()
        });
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
