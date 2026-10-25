//! One continuation driver shared by read-only and editable checking.

use std::collections::VecDeque;

use serde_json::{Map, Value};

use super::super::{
    collections,
    prepare::{self, Patterns},
    references::{Context, Location, Root},
    scalars,
};
use super::{
    Batch, Evaluation, Instruction, Job, Mode, Outcome, apply,
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
    fn advance(&mut self, root: &Root<'_, 's>, patterns: &Patterns<'_>) -> bool {
        if !self.pass.exhaustive() && !self.result.errors.is_empty() {
            self.instructions.clear();
        }
        let Some(instruction) = self.instructions.pop_front() else {
            return false;
        };
        let mut work = Vec::new();
        match instruction {
            Instruction::Phase(phase) => {
                let job = Job {
                    context: self.context.clone(),
                    value: Instance::Same,
                    path: self.path.clone(),
                };
                work = phase_instructions(phase, &job, self.instance.value(), root, patterns);
            }
            Instruction::Dependency(job, keyword, key) => {
                collections::dependency(&job, self.instance.value(), keyword, &key, &mut work);
            }
            Instruction::Errors(errors) => self.result.add_errors(errors),
            Instruction::Children(batch) => self.pending = Some(batch),
            Instruction::Unevaluated(job) => {
                work = collections::unevaluated(&job, &self.result.marks, self.instance.value());
            }
        }
        for instruction in work.into_iter().rev() {
            self.instructions.push_front(instruction);
        }
        true
    }

    /// Apply the supplied refinement only after structural success.
    fn refine(&mut self, effect: &mut Effect<'s>) {
        if self.result.errors.is_empty()
            && let Operand::Write(value) = &mut self.instance
            && let Err(error) = effect(self.context.location.schema, value)
        {
            self.result
                .add_errors([scalars::render(&self.path, &error)]);
        }
    }
}

/// Mutable callback boundary used by the same continuation as borrowed checking.
type Effect<'s> = dyn FnMut(&'s Value, &mut Value) -> Result<(), String> + 's;

/// Check a borrowed operand without copying its tree or changing its contents.
pub(in super::super) fn check<'s>(
    schema: &'s Value,
    definitions: &[&'s Map<String, Value>],
    value: &Value,
) -> Result<Vec<String>, regress::Error> {
    evaluate(
        schema,
        definitions,
        Operand::Read(value),
        &mut |_, _| Ok(()),
    )
    .map(|(_, errors)| errors)
}

/// Check once, then collect corrective diagnostics only when checking rejected the value.
pub(super) fn evaluate<'s, 'v>(
    schema: &'s Value,
    definitions: &[&'s Map<String, Value>],
    value: Operand<'v>,
    effect: &mut Effect<'s>,
) -> Result<(Operand<'v>, Vec<String>), regress::Error> {
    let root = Root {
        location: Location::root(schema),
        definitions,
    };
    let patterns = prepare::prepare(&root)?;
    let pass = Pass::Compiled {
        unevaluated: uses_unevaluated(schema),
    };
    let (value, check_errors) = run(&root, &patterns, value, effect, pass);
    if check_errors.is_empty() {
        return Ok((value, check_errors));
    }
    let (value, errors) = run(&root, &patterns, value, effect, Pass::Errors);
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
fn run<'s, 'v>(
    root: &Root<'_, 's>,
    patterns: &Patterns<'_>,
    value: Operand<'v>,
    effect: &mut Effect<'s>,
    pass: Pass,
) -> (Operand<'v>, Vec<String>) {
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
        if active.pending.is_some() || active.advance(root, patterns) {
            continue;
        }
        active.refine(effect);
        let Some(mut parent) = parents.pop() else {
            return (active.instance, active.result.errors);
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
            batch.results.push(Outcome::Invalid(vec![scalars::render(
                &job.path,
                "schema is false",
            )]));
        }
        return None;
    }
    Some(Frame::new(job, value, identity, active.child_pass()))
}
