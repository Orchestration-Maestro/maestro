use std::{
    borrow::Cow,
    collections::{BTreeSet, VecDeque},
};

use num_traits::ToPrimitive;
use serde_json::Value;

use super::{collections, references, scalars};

#[derive(Default)]
pub(super) struct Evaluation {
    pub errors: Vec<String>,
    pub keys: BTreeSet<String>,
    pub indices: BTreeSet<usize>,
}

impl Evaluation {
    fn merge(&mut self, mut other: Self) {
        self.errors.append(&mut other.errors);
        self.keys.append(&mut other.keys);
        self.indices.append(&mut other.indices);
    }
}

pub(super) enum Instruction<'a> {
    Errors(Vec<String>),
    Children(Batch<'a>),
    Unevaluated(Job<'a>),
}

pub(super) enum Mode<'a> {
    All,
    Any,
    One,
    Members(Vec<Mark>),
    Aggregate {
        marks: Vec<Mark>,
        message: String,
    },
    Count {
        minimum: Option<f64>,
        maximum: Option<f64>,
        message: String,
    },
    Names(Vec<String>),
    Not,
    If {
        then: Job<'a>,
        otherwise: Job<'a>,
    },
    Selected {
        then: bool,
        condition: Evaluation,
    },
}

pub(super) enum Mark {
    Key(String),
    Index(usize),
}

#[derive(Clone, Copy)]
pub(super) enum Instance<'a> {
    Json(&'a Value),
    Name(&'a str),
}

impl<'a> Instance<'a> {
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Json(left), Self::Json(right)) => std::ptr::eq(left, right),
            (Self::Name(left), Self::Name(right)) => std::ptr::eq(left, right),
            _ => false,
        }
    }

    fn value(self) -> Cow<'a, Value> {
        match self {
            Self::Json(value) => Cow::Borrowed(value),
            Self::Name(name) => Cow::Owned(Value::String(name.to_owned())),
        }
    }
}

#[derive(Clone)]
pub(super) struct Job<'a> {
    pub schema: &'a Value,
    pub value: Instance<'a>,
    pub path: String,
}

pub(super) struct Batch<'a> {
    pub mode: Mode<'a>,
    pub jobs: VecDeque<Job<'a>>,
    pub results: Vec<Evaluation>,
}

struct Frame<'a> {
    schema: &'a Value,
    instance: Instance<'a>,
    scope: references::ScopeIdentity<'a>,
    instructions: VecDeque<Instruction<'a>>,
    pending: Option<Batch<'a>>,
    result: Evaluation,
    path: String,
}

impl<'a> Frame<'a> {
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

    fn next_child(&mut self) -> Option<Job<'a>> {
        self.pending.as_mut()?.jobs.pop_front()
    }

    fn finish_batch(&mut self) {
        if let Some(batch) = self.pending.take() {
            self.pending = apply(batch, &self.path, &mut self.result);
        }
    }
}

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
            Some(Instruction::Errors(mut errors)) => frame.result.errors.append(&mut errors),
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
        for (name, mut child) in names.into_iter().zip(batch.results) {
            if child.errors.is_empty() {
                add_mark(&name, result);
            }
            result.errors.append(&mut child.errors);
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
            result.errors.extend(child.errors);
        }
    }
    if !valid && let Some(summary) = summary {
        result.errors.push(scalars::render(path, summary));
    }
    None
}

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
                result.errors.push(scalars::render(path, message));
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
                result.errors.push(scalars::render(path, message));
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
                result.errors.push(scalars::render(
                    path,
                    &format!("property names {} are invalid", invalid.join(", ")),
                ));
            }
        }
        _ => return false,
    }
    true
}

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

fn conditional<'a>(job: &Job<'a>, instructions: &mut Vec<Instruction<'a>>) {
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

fn apply_conditional<'a>(
    batch: Batch<'a>,
    path: &str,
    result: &mut Evaluation,
) -> Option<Batch<'a>> {
    let child = batch.results.into_iter().next()?;
    match batch.mode {
        Mode::Not => {
            if child.errors.is_empty() {
                result
                    .errors
                    .push(scalars::render(path, "must not be valid"));
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
                condition.errors.clear();
                result.merge(condition);
                result.merge(child);
            } else {
                if !then {
                    result.errors.extend(child.errors);
                }
                let keyword = if then { "then" } else { "else" };
                result.errors.push(scalars::render(
                    path,
                    &format!("must match \"{keyword}\" schema"),
                ));
            }
        }
        _ => {}
    }
    None
}

fn reject_cycle(frames: &mut [Frame<'_>], path: &str) {
    if let Some(batch) = frames.last_mut().and_then(|frame| frame.pending.as_mut()) {
        batch.results.push(Evaluation {
            errors: vec![scalars::render(path, "schema is false")],
            ..Evaluation::default()
        });
    }
}

pub(super) fn is_schema(value: &Value) -> bool {
    value.is_object() || value.is_boolean()
}

pub(super) fn schema_array(value: &Value) -> Option<&[Value]> {
    value
        .as_array()
        .filter(|schemas| schemas.iter().all(is_schema))
        .map(Vec::as_slice)
}
