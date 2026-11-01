//! One continuation driver shared by read-only and editable checking.

use std::collections::VecDeque;

use crate::DiagnosticErrorInfo;
use serde_json::{Map, Value};

use super::super::{
    collections,
    prepare::{self, Patterns},
    references::{Context, Location, Root},
    scalars,
};
use super::{
    Batch, Code, Evaluation, Failure, Instruction, Job, Mode, Outcome, apply,
    operand::{Instance, Operand},
    program::{instructions, phase_instructions},
};

/// Source evaluation policy shared by the continuation driver.
#[derive(Clone, Copy)]
enum Pass {
    /// Accelerated checking may short-circuit compositions without location tracking.
    Compiled {
        /// Whether any schema requires evaluated-location tracking.
        unevaluated: bool,
    },
    /// Interpreted checking reduces every composition branch.
    Check,
    /// Diagnostics visit every ordinary assertion.
    Errors,
}

impl Pass {
    /// Whether ordinary assertion failures allow subsequent assertions.
    fn exhaustive(self) -> bool {
        matches!(self, Self::Errors)
    }
}

/// Explicit evaluator continuation with one live operand.
struct Frame<'s, 'v> {
    /// Evaluation and child-admission policy.
    pass: Pass,
    /// Schema location with live reference bindings.
    context: Context<'s>,
    /// Borrowed or exclusively owned current value.
    instance: Operand<'v>,
    /// Operand identity retained across same-instance applications.
    identity: usize,
    /// Selection to restore into the suspended parent.
    selected: Instance,
    /// Keywords not yet reached.
    instructions: VecDeque<Instruction<'s>>,
    /// Child applications awaiting settlement.
    pending: Option<Batch<'s>>,
    /// Ordered failures and evaluated locations.
    result: Evaluation,
    /// Diagnostic location within the argument.
    path: String,
}

impl<'s, 'v> Frame<'s, 'v> {
    /// Start one application without observing its value-dependent rules early.
    fn new(job: Job<'s>, instance: Operand<'v>, identity: usize, pass: Pass) -> Self {
        Self {
            pass,
            instructions: instructions(&job).into(),
            context: job.context,
            selected: job.value,
            instance,
            identity,
            pending: None,
            result: Evaluation::default(),
            path: job.path,
        }
    }

    /// Admit the next child according to the reached collection or composition policy.
    fn next_child(&mut self) -> Option<Job<'s>> {
        let batch = self.pending.as_mut()?;
        let failed = matches!(batch.results.last(), Some(Outcome::Invalid(_)));
        let succeeded = matches!(batch.results.last(), Some(Outcome::Valid(_)));
        let stop = match (&batch.mode, self.pass) {
            (Mode::Some, _) | (Mode::Any, Pass::Compiled { unevaluated: false }) => succeeded,
            (Mode::All, Pass::Compiled { unevaluated: false }) => failed,
            (Mode::Members(_) | Mode::Aggregate { .. } | Mode::Names(_), pass) => {
                !pass.exhaustive() && failed
            }
            _ => false,
        };
        if stop {
            batch.jobs.clear();
        }
        batch.jobs.pop_front()
    }

    /// Select the interpreted checker for diagnostic containment and negation.
    fn child_pass(&self) -> Pass {
        match (self.pass, self.pending.as_ref().map(|batch| &batch.mode)) {
            (Pass::Errors, Some(Mode::Some | Mode::Count { .. } | Mode::Not)) => Pass::Check,
            _ => self.pass,
        }
    }

    /// Merge completed children or schedule a selected conditional branch.
    fn finish_batch(&mut self) {
        if let Some(batch) = self.pending.take() {
            self.pending = apply(batch, &self.path, &mut self.result);
        }
    }

    /// Restore the exclusive operand before publishing the child's outcome.
    fn restore(&mut self, completed: Self) {
        self.instance
            .restore(&completed.selected, completed.instance);
        if let Some(batch) = &mut self.pending {
            batch.results.push(completed.result.finish());
        }
    }

    /// Resolve the next keyword only after earlier child applications have settled.
    fn advance(
        &mut self,
        root: &Root<'_, 's>,
        patterns: &Patterns<'_>,
        code: &Code<'_>,
    ) -> Result<bool, DiagnosticErrorInfo> {
        if !self.pass.exhaustive() && !self.result.errors.is_empty() {
            self.instructions.clear();
        }
        let Some(instruction) = self.instructions.pop_front() else {
            return Ok(false);
        };
        let mut work = Vec::new();
        match instruction {
            Instruction::Guard => super::code::guard(
                code.node(self.context.location.schema),
                &mut self.instance,
                &self.path,
                self.pass.exhaustive(),
                &mut self.result,
            )?,
            Instruction::Phase(phase) => {
                let job = Job {
                    context: self.context.clone(),
                    value: Instance::Same,
                    path: self.path.clone(),
                };
                if matches!(
                    (phase, self.pass),
                    (super::Phase::Properties, Pass::Compiled { .. })
                ) && let Some(Value::Object(value)) = self.instance.value()
                {
                    collections::properties(&job, value, true, &mut work);
                } else {
                    work = phase_instructions(phase, &job, self.instance.value(), root, patterns);
                }
            }
            Instruction::Dependency(job, keyword, key) => {
                if let Some(value) = self.instance.value() {
                    collections::dependency(&job, value, keyword, &key, &mut work);
                }
            }
            Instruction::Errors(errors) => self.result.add_errors(errors),
            Instruction::Children(batch) => self.pending = Some(batch),
            Instruction::Unevaluated(job) => {
                if let Some(value) = self.instance.value() {
                    work = collections::unevaluated(&job, &self.result.marks, value);
                }
            }
        }
        for instruction in work.into_iter().rev() {
            self.instructions.push_front(instruction);
        }
        Ok(true)
    }
}

/// Check a borrowed operand without copying its tree or changing its contents.
pub(in super::super) fn check<'s>(
    schema: &'s Value,
    definitions: &[&'s Map<String, Value>],
    value: &Value,
) -> Result<Vec<String>, DiagnosticErrorInfo> {
    evaluate(schema, definitions, Operand::Read(value), &Code::default()).map(|(_, errors)| errors)
}

/// Run only candidate acceptance, without requesting diagnostic factories.
pub(in super::super) fn candidate(
    schema: &Value,
    value: &mut Value,
    code: &Code<'_>,
) -> Result<bool, DiagnosticErrorInfo> {
    acceptance(
        schema,
        &[],
        value,
        code,
        Pass::Compiled {
            unevaluated: uses_unevaluated(schema),
        },
    )
}

/// Check an authored conversion candidate with its currently visible definitions.
pub(in super::super) fn authored_candidate<'s>(
    schema: &'s Value,
    definitions: &[&'s Map<String, Value>],
    value: &mut Value,
    code: &Code<'_>,
) -> Result<bool, DiagnosticErrorInfo> {
    acceptance(schema, definitions, value, code, Pass::Check)
}

/// Run acceptance without diagnostic factories under the caller's source policy.
fn acceptance<'s>(
    schema: &'s Value,
    definitions: &[&'s Map<String, Value>],
    value: &mut Value,
    code: &Code<'_>,
    pass: Pass,
) -> Result<bool, DiagnosticErrorInfo> {
    let root = Root {
        location: Location::root(schema),
        definitions,
    };
    let Ok(patterns) = prepare::prepare(&root) else {
        return Ok(false);
    };
    let (result, errors) = run(
        &root,
        &patterns,
        Operand::Write(std::mem::take(value)),
        code,
        pass,
    )?;
    if let Operand::Write(result) = result {
        *value = result;
    }
    Ok(errors.is_empty())
}

/// Check an exclusive argument value through the same continuation as borrowed checking.
pub(in super::super) fn check_owned(
    schema: &Value,
    value: Value,
    code: &Code<'_>,
) -> Result<(Value, Vec<String>), DiagnosticErrorInfo> {
    let (value, errors) = evaluate(schema, &[], Operand::Write(value), code)?;
    Ok((
        value
            .into_owned()
            .ok_or_else(|| super::super::diagnostic("root operand is absent".into()))?,
        errors,
    ))
}

/// Check once, then collect corrective diagnostics only when checking rejected the value.
pub(super) fn evaluate<'s, 'v>(
    schema: &'s Value,
    definitions: &[&'s Map<String, Value>],
    value: Operand<'v>,
    code: &Code<'_>,
) -> Result<(Operand<'v>, Vec<String>), DiagnosticErrorInfo> {
    let root = Root {
        location: Location::root(schema),
        definitions,
    };
    let patterns =
        prepare::prepare(&root).map_err(|error| super::super::diagnostic(error.to_string()))?;
    let pass = Pass::Compiled {
        unevaluated: uses_unevaluated(schema),
    };
    let (value, check_errors) = run(&root, &patterns, value, code, pass)?;
    if check_errors.is_empty() {
        return Ok((value, check_errors));
    }
    let (value, errors) = run(&root, &patterns, value, code, Pass::Errors)?;
    Ok((
        value,
        if errors.is_empty() {
            check_errors
        } else {
            errors
        },
    ))
}

/// Resolve the schema-wide composition policy without retaining a cache.
fn uses_unevaluated(schema: &Value) -> bool {
    let mut pending = vec![schema];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                if object.contains_key("unevaluatedProperties")
                    || object.contains_key("unevaluatedItems")
                {
                    return true;
                }
                pending.extend(object.values());
            }
            Value::Array(values) => pending.extend(values),
            _ => {}
        }
    }
    false
}

/// Execute one requested mode, keeping the current frame separate from suspended parents.
fn run<'v>(
    root: &Root<'_, '_>,
    patterns: &Patterns<'_>,
    value: Operand<'v>,
    code: &Code<'_>,
    pass: Pass,
) -> Result<(Operand<'v>, Vec<String>), DiagnosticErrorInfo> {
    let job = Job {
        context: Context::root(root.location.clone()),
        value: Instance::Same,
        path: String::new(),
    };
    let mut active = Frame::new(job, value, 0, pass);
    let mut parents = Vec::new();
    let mut next_identity = 1;
    loop {
        if let Some(job) = active.next_child() {
            if let Some(child) = admit(&mut active, job, &parents, &mut next_identity) {
                parents.push(active);
                active = child;
            }
            continue;
        }
        active.finish_batch();
        if active.pending.is_some() || active.advance(root, patterns, code)? {
            continue;
        }
        if active.result.errors.is_empty() {
            super::code::refine(
                code.node(active.context.location.schema),
                &mut active.instance,
                &active.path,
                active.pass.exhaustive(),
                &mut active.result,
            )?;
        }
        let Some(mut parent) = parents.pop() else {
            return Ok((
                active.instance,
                active
                    .result
                    .errors
                    .into_iter()
                    .map(Failure::into_message)
                    .collect(),
            ));
        };
        parent.restore(active);
        active = parent;
    }
}

/// Select a live child and reject only non-progressing reference cycles.
fn admit<'s, 'v>(
    active: &mut Frame<'s, 'v>,
    job: Job<'s>,
    parents: &[Frame<'s, 'v>],
    next_identity: &mut usize,
) -> Option<Frame<'s, 'v>> {
    let identity = if matches!(job.value, Instance::Same) {
        active.identity
    } else {
        let identity = *next_identity;
        *next_identity += 1;
        identity
    };
    let value = active.instance.select(&job.value)?;
    if parents
        .iter()
        .chain([&*active])
        .any(|frame| frame.identity == identity && frame.context.same(&job.context))
    {
        active.instance.restore(&job.value, value);
        if let Some(batch) = &mut active.pending {
            batch.results.push(Outcome::Invalid(vec![
                scalars::render(&job.path, "schema is false").into(),
            ]));
        }
        return None;
    }
    Some(Frame::new(job, value, identity, active.child_pass()))
}
