//! Keyword continuations resolved only when their phase is reached.

use super::super::{
    collections,
    prepare::Patterns,
    references::{self, Root},
    scalars,
};
use super::{Batch, Instruction, Job, Mode, conditional, is_schema, schema_array};
use serde_json::Value;

/// Source-ordered keyword continuations, with no value-dependent result stored early.
#[derive(Clone, Copy)]
pub(in super::super) enum Phase {
    /// Resolve the `Type` assertion at its ordered position.
    Type,
    /// Resolve the `Required` assertion at its ordered position.
    Required,
    /// Resolve the `AdditionalProperties` assertion at its ordered position.
    AdditionalProperties,
    /// Resolve the `Dependencies` assertion at its ordered position.
    Dependencies,
    /// Resolve the `DependentRequired` assertion at its ordered position.
    DependentRequired,
    /// Resolve the `DependentSchemas` assertion at its ordered position.
    DependentSchemas,
    /// Resolve the `PatternProperties` assertion at its ordered position.
    PatternProperties,
    /// Resolve the `Properties` assertion at its ordered position.
    Properties,
    /// Resolve the `PropertyNames` assertion at its ordered position.
    PropertyNames,
    /// Resolve the `PropertyBounds` assertion at its ordered position.
    PropertyBounds,
    /// Resolve the `AdditionalItems` assertion at its ordered position.
    AdditionalItems,
    /// Resolve the `Contains` assertion at its ordered position.
    Contains,
    /// Resolve the `Items` assertion at its ordered position.
    Items,
    /// Resolve the `MaxContains` assertion at its ordered position.
    MaxContains,
    /// Resolve the `MaxItems` assertion at its ordered position.
    MaxItems,
    /// Resolve the `MinContains` assertion at its ordered position.
    MinContains,
    /// Resolve the `MinItems` assertion at its ordered position.
    MinItems,
    /// Resolve the `PrefixItems` assertion at its ordered position.
    PrefixItems,
    /// Resolve the `UniqueItems` assertion at its ordered position.
    UniqueItems,
    /// Resolve the `String` assertion at its ordered position.
    String,
    /// Resolve the `Number` assertion at its ordered position.
    Number,
    /// Resolve the `References` assertion at its ordered position.
    References,
    /// Resolve the `Literal` assertion at its ordered position.
    Literal,
    /// Resolve the `Conditional` assertion at its ordered position.
    Conditional,
    /// Resolve the `Not` assertion at its ordered position.
    Not,
    /// Resolve the `AllOf` assertion at its ordered position.
    AllOf,
    /// Resolve the `AnyOf` assertion at its ordered position.
    AnyOf,
    /// Resolve the `OneOf` assertion at its ordered position.
    OneOf,
}

/// Schedule schema work, never observations of the current value.
pub(super) fn instructions<'a>(job: &Job<'a>) -> Vec<Instruction<'a>> {
    let phases = [
        Phase::Type,
        Phase::Required,
        Phase::AdditionalProperties,
        Phase::Dependencies,
        Phase::DependentRequired,
        Phase::DependentSchemas,
        Phase::PatternProperties,
        Phase::Properties,
        Phase::PropertyNames,
        Phase::PropertyBounds,
        Phase::AdditionalItems,
        Phase::Contains,
        Phase::Items,
        Phase::MaxContains,
        Phase::MaxItems,
        Phase::MinContains,
        Phase::MinItems,
        Phase::PrefixItems,
        Phase::UniqueItems,
        Phase::String,
        Phase::Number,
        Phase::References,
        Phase::Literal,
        Phase::Conditional,
        Phase::Not,
        Phase::AllOf,
        Phase::AnyOf,
        Phase::OneOf,
    ];
    let mut work = Vec::new();
    for phase in phases {
        if matches!(phase, Phase::Literal) {
            work.push(Instruction::Guard);
        }
        work.push(Instruction::Phase(phase));
    }
    work.push(Instruction::Unevaluated(job.clone()));
    work
}
/// Resolve one reached keyword against the one live operand.
pub(super) fn phase_instructions<'a>(
    phase: Phase,
    job: &Job<'a>,
    value: Option<&Value>,
    root: &Root<'_, 'a>,
    patterns: &Patterns<'_>,
) -> Vec<Instruction<'a>> {
    let mut work = Vec::new();
    match phase {
        Phase::Type => work.push(Instruction::Errors(scalars::type_errors(
            job.schema(),
            value,
            &job.path,
        ))),
        Phase::String | Phase::Number => {
            if let Some(value) = value {
                let errors = if matches!(phase, Phase::String) {
                    scalars::string_errors(job.schema(), value, &job.path, patterns)
                } else {
                    scalars::number_errors(job.schema(), value, &job.path)
                };
                work.push(Instruction::Errors(errors));
            }
        }
        Phase::References => work.extend(references::instructions(root, job)),
        Phase::Literal => work.push(Instruction::Errors(scalars::literal_errors(
            job.schema(),
            value,
            &job.path,
        ))),
        Phase::Conditional => conditional(job, &mut work),
        Phase::Not => {
            if let Some(schema) = job.schema().get("not").filter(|schema| is_schema(schema)) {
                work.push(Instruction::Children(Batch::new(
                    Mode::Not,
                    [job.same_instance(schema)],
                )));
            }
        }
        Phase::AllOf | Phase::AnyOf | Phase::OneOf => {
            let (keyword, mode) = match phase {
                Phase::AllOf => ("allOf", Mode::All),
                Phase::AnyOf => ("anyOf", Mode::Any),
                _ => ("oneOf", Mode::One),
            };
            if let Some(schemas) = job.schema().get(keyword).and_then(schema_array) {
                work.push(Instruction::Children(Batch::new(
                    mode,
                    schemas.iter().map(|schema| job.same_instance(schema)),
                )));
            }
        }
        _ => {
            if let Some(value) = value {
                collections::phase(phase, job, value, patterns, &mut work);
            }
        }
    }
    work
}
