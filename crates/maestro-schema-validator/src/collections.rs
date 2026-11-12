//! Object and array assertions with per-location evaluation evidence.

use std::{borrow::Cow, collections::HashSet};

use num_traits::ToPrimitive;
use serde_json::{Map, Value};

use super::{
    check::{Batch, Instance, Instruction, Job, Mark, Marks, Mode, Phase},
    check::{is_schema, schema_array, schema_map},
    diagnostics,
    prepare::Patterns,
    scalars,
};

/// Prepared pattern expressions paired with their member schemas.
type Matchers<'p, 'a> = Vec<(&'p regress::Regex, &'a Value)>;

/// Resolve collection assertions only at their source-ordered phase.
pub(super) fn phase<'a>(
    phase: Phase,
    job: &Job<'a>,
    value: &Value,
    prepared: &Patterns<'_>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    if let Value::Object(value) = value {
        match phase {
            Phase::Required => instructions.push(Instruction::Errors(scalars::required_errors(
                job.schema(),
                value,
                &job.path,
            ))),
            Phase::AdditionalProperties | Phase::PatternProperties => {
                let patterns: Matchers<'_, 'a> = schema_map(job.schema().get("patternProperties"))
                    .into_iter()
                    .flat_map(diagnostics::entries)
                    .filter_map(|(source, schema)| Some((prepared.get(source)?, schema)))
                    .collect();
                if matches!(phase, Phase::AdditionalProperties) {
                    additional_properties(job, value, &patterns, instructions);
                } else {
                    pattern_properties(job, value, &patterns, instructions);
                }
            }
            Phase::Dependencies => dependencies(job, "dependencies", instructions),
            Phase::DependentRequired => dependencies(job, "dependentRequired", instructions),
            Phase::DependentSchemas => dependencies(job, "dependentSchemas", instructions),
            Phase::Properties => properties(job, value, false, instructions),
            Phase::PropertyNames => property_names(job, value, instructions),
            Phase::PropertyBounds => {
                instructions.push(bounds(job, value.len(), "Properties", "properties"));
            }
            _ => {}
        }
    }
    if let Value::Array(values) = value {
        match phase {
            Phase::AdditionalItems => additional_items(job, values, instructions),
            Phase::Contains => contains(job, values, "contains", instructions),
            Phase::Items => items(job, values, instructions),
            Phase::MaxContains => contains(job, values, "maxContains", instructions),
            Phase::MaxItems => {
                instructions.push(single_bound(job, values.len(), "maxItems", "more"));
            }
            Phase::MinContains => contains(job, values, "minContains", instructions),
            Phase::MinItems => {
                instructions.push(single_bound(job, values.len(), "minItems", "fewer"));
            }
            Phase::PrefixItems => prefix_items(job, values, instructions),
            Phase::UniqueItems => unique_items(job, values, instructions),
            _ => {}
        }
    }
}

/// Check undeclared, unmatched members with an aggregate diagnostic.
fn additional_properties<'a>(
    job: &Job<'a>,
    value: &Map<String, Value>,
    patterns: &Matchers<'_, 'a>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(schema) = job
        .schema()
        .get("additionalProperties")
        .filter(|schema| schema.is_boolean() || schema.is_object())
    else {
        return;
    };
    let declared = schema_map(job.schema().get("properties"));
    let members = diagnostics::entries(value)
        .into_iter()
        .filter(|(key, _)| {
            !declared.is_some_and(|declared| declared.contains_key(*key))
                && !patterns.iter().any(|(regex, _)| regex.find(key).is_some())
        })
        .map(|(key, _)| {
            (
                Mark::Key(key.clone()),
                schema,
                format!("{}/{key}", job.path),
            )
        });
    instructions.push(aggregate(
        job,
        members,
        "must not have additional properties",
    ));
}

/// Apply every matching pattern schema to each object member.
fn pattern_properties<'a>(
    job: &Job<'a>,
    value: &Map<String, Value>,
    patterns: &Matchers<'_, 'a>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    for (regex, schema) in patterns {
        let members = diagnostics::entries(value)
            .into_iter()
            .filter(|(key, _)| regex.find(key).is_some())
            .map(|(key, _)| {
                (
                    Mark::Key(key.clone()),
                    *schema,
                    format!("{}/{key}", job.path),
                )
            });
        instructions.push(Instruction::Children(members_batch(job, members)));
    }
}

/// Schedule declared members, retaining deleted required members in compiled checking.
pub(super) fn properties<'a>(
    job: &Job<'a>,
    value: &Map<String, Value>,
    compiled: bool,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(properties) = schema_map(job.schema().get("properties")) else {
        return;
    };
    let required: HashSet<&str> = scalars::required_names(job.schema())
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let members = diagnostics::entries(properties)
        .into_iter()
        .filter_map(|(key, schema)| {
            if !(value.contains_key(key) || compiled && required.contains(key.as_str())) {
                return None;
            }
            Some((
                Mark::Key(key.clone()),
                schema,
                format!("{}/{key}", job.path),
            ))
        });
    let mut batch = members_batch(job, members);
    if compiled {
        for child in &mut batch.jobs {
            if let Instance::Key(key) = &child.value
                && required.contains(key.as_str())
            {
                child.value = Instance::RequiredKey(key.clone());
            }
        }
    }
    instructions.push(Instruction::Children(batch));
}

/// Queue dependency declarations without admitting their current triggers early.
fn dependencies<'a>(job: &Job<'a>, keyword: &'static str, instructions: &mut Vec<Instruction<'a>>) {
    if let Some(declarations) = dependency_map(job.schema(), keyword) {
        for (key, _) in diagnostics::entries(declarations) {
            instructions.push(Instruction::Dependency(job.clone(), keyword, key.clone()));
        }
    }
}
/// Read this trigger and its required members after earlier callbacks return.
pub(super) fn dependency<'a>(
    job: &Job<'a>,
    value: &Value,
    keyword: &str,
    key: &str,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Value::Object(value) = value else { return };
    if !value.contains_key(key) {
        return;
    }
    let Some(schema) = job
        .schema()
        .get(keyword)
        .and_then(Value::as_object)
        .and_then(|map| map.get(key))
    else {
        return;
    };
    if let Some(names) = schema.as_array() {
        let declared: Vec<_> = names.iter().filter_map(Value::as_str).collect();
        if keyword != "dependentSchemas" && declared.iter().any(|name| !value.contains_key(*name)) {
            instructions.push(Instruction::Errors(vec![scalars::render(
                &job.path,
                &format!(
                    "must have properties {} when property {key} is present",
                    declared.join(", ")
                ),
            )]));
        }
    } else if keyword != "dependentRequired" && is_schema(schema) {
        instructions.push(Instruction::Children(Batch::new(
            Mode::All,
            [job.same_instance(schema)],
        )));
    }
}

/// Borrow a dependency keyword's object only when every entry suits that keyword.
pub(super) fn dependency_map<'a>(
    schema: &'a Value,
    keyword: &str,
) -> Option<&'a Map<String, Value>> {
    schema
        .get(keyword)
        .and_then(Value::as_object)
        .filter(|dependencies| {
            dependencies.values().all(|entry| match keyword {
                "dependentSchemas" => is_schema(entry),
                "dependentRequired" => names(entry),
                _ => is_schema(entry) || names(entry),
            })
        })
}

/// Validate keys as strings while retaining an aggregate name failure.
fn property_names<'a>(
    job: &Job<'a>,
    value: &Map<String, Value>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(schema) = job
        .schema()
        .get("propertyNames")
        .filter(|schema| is_schema(schema))
    else {
        return;
    };
    let entries = diagnostics::entries(value);
    let names = entries.iter().map(|(key, _)| (*key).clone()).collect();
    let jobs = entries
        .into_iter()
        .map(|(key, _)| job.child(schema, Instance::Name(key.clone()), job.path.clone()));
    instructions.push(Instruction::Children(Batch::new(Mode::Names(names), jobs)));
}

/// Render minimum and maximum collection-cardinality failures.
fn bounds<'a>(job: &Job<'a>, size: usize, suffix: &str, noun: &str) -> Instruction<'a> {
    let mut errors = Vec::new();
    for (prefix, comparison) in [("min", "fewer"), ("max", "more")] {
        let Some(limit) = job
            .schema()
            .get(format!("{prefix}{suffix}"))
            .and_then(Value::as_f64)
        else {
            continue;
        };
        if (prefix == "min" && size.to_f64().is_some_and(|size| size < limit))
            || (prefix == "max" && size.to_f64().is_some_and(|size| size > limit))
        {
            errors.push(scalars::render(
                &job.path,
                &format!(
                    "must not have {comparison} than {} {noun}",
                    ryu_js::Buffer::new().format(limit)
                ),
            ));
        }
    }
    Instruction::Errors(errors)
}

/// Resolve prefix members at their reached phase.
fn prefix_items<'a>(job: &Job<'a>, values: &[Value], instructions: &mut Vec<Instruction<'a>>) {
    if let Some(prefix) = job.schema().get("prefixItems").and_then(schema_array) {
        let members = prefix
            .iter()
            .zip(values)
            .enumerate()
            .map(|(index, (schema, _))| {
                (Mark::Index(index), schema, format!("{}/{index}", job.path))
            });
        instructions.push(Instruction::Children(members_batch(job, members)));
    }
}
/// Resolve structural uniqueness after earlier member checks return.
fn unique_items<'a>(job: &Job<'a>, values: &[Value], instructions: &mut Vec<Instruction<'a>>) {
    if job.schema().get("uniqueItems") == Some(&Value::Bool(true))
        && values.iter().enumerate().any(|(index, value)| {
            values[..index]
                .iter()
                .any(|other| scalars::equal(value, other))
        })
    {
        instructions.push(Instruction::Errors(vec![scalars::render(
            &job.path,
            "must not have duplicate items",
        )]));
    }
}

/// Render one array-length constraint at its keyword position.
fn single_bound<'a>(
    job: &Job<'a>,
    length: usize,
    keyword: &str,
    comparison: &str,
) -> Instruction<'a> {
    let mut errors = Vec::new();
    if let Some(limit) = job.schema().get(keyword).and_then(Value::as_f64)
        && ((keyword == "maxItems" && length.to_f64().is_some_and(|length| length > limit))
            || (keyword == "minItems" && length.to_f64().is_some_and(|length| length < limit)))
    {
        errors.push(scalars::render(
            &job.path,
            &format!(
                "must not have {comparison} than {} items",
                ryu_js::Buffer::new().format(limit)
            ),
        ));
    }
    Instruction::Errors(errors)
}

/// Check the tail beyond a legacy tuple definition.
fn additional_items<'a>(job: &Job<'a>, values: &[Value], instructions: &mut Vec<Instruction<'a>>) {
    let (Some(items), Some(schema)) = (
        job.schema().get("items").and_then(schema_array),
        job.schema()
            .get("additionalItems")
            .filter(|schema| is_schema(schema)),
    ) else {
        return;
    };
    let members = values
        .iter()
        .enumerate()
        .skip(items.len())
        .map(|(index, _)| (Mark::Index(index), schema, format!("{}/{index}", job.path)));
    instructions.push(Instruction::Children(members_batch(job, members)));
}

/// Check tuple positions or homogeneous items after the declared prefix.
fn items<'a>(job: &Job<'a>, values: &[Value], instructions: &mut Vec<Instruction<'a>>) {
    let Some(items) = job.schema().get("items") else {
        return;
    };
    let prefix_length = job
        .schema()
        .get("prefixItems")
        .and_then(schema_array)
        .map_or(0, <[Value]>::len);
    let members = values.iter().enumerate().filter_map(|(index, _)| {
        let schema = match items {
            Value::Array(schemas) if schemas.iter().all(is_schema) => schemas.get(index)?,
            Value::Bool(_) | Value::Object(_) if index >= prefix_length => items,
            _ => return None,
        };
        Some((Mark::Index(index), schema, format!("{}/{index}", job.path)))
    });
    instructions.push(Instruction::Children(members_batch(job, members)));
}

/// Count matching items only when a contains schema is active.
fn contains<'a>(
    job: &Job<'a>,
    values: &[Value],
    keyword: &str,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(schema) = job
        .schema()
        .get("contains")
        .filter(|schema| is_schema(schema))
    else {
        return;
    };
    let (minimum, maximum, message) = match keyword {
        "contains" if job.schema().get("minContains").and_then(Value::as_f64) == Some(0.0) => {
            return;
        }
        "contains" => (
            Some(1.0),
            None,
            Cow::Borrowed("must contain at least 1 valid item"),
        ),
        "minContains" | "maxContains" => {
            let Some(limit) = job.schema().get(keyword).and_then(Value::as_f64) else {
                return;
            };
            let minimum = (keyword == "minContains").then_some(limit);
            let maximum = (keyword == "maxContains").then_some(limit);
            let comparison = if minimum.is_some() { "least" } else { "most" };
            let noun = if matches!(limit, 1.0) {
                "item"
            } else {
                "items"
            };
            (
                minimum,
                maximum,
                Cow::Owned(format!(
                    "must contain at {comparison} {} valid {noun}",
                    ryu_js::Buffer::new().format(limit)
                )),
            )
        }
        _ => return,
    };
    let jobs = values
        .iter()
        .enumerate()
        .map(|(i, _)| job.child(schema, Instance::Index(i), job.path.clone()));
    let mode = if keyword == "contains" {
        Mode::Some
    } else {
        Mode::Count {
            minimum,
            maximum,
            message,
        }
    };
    instructions.push(Instruction::Children(Batch::new(mode, jobs)));
}

/// Pair child evaluations with the locations they can mark.
fn members_batch<'a>(
    parent: &Job<'a>,
    members: impl Iterator<Item = (Mark, &'a Value, String)>,
) -> Batch<'a> {
    let mut marks = Vec::new();
    let jobs: Vec<_> = members
        .map(|(mark, schema, path)| {
            let target = match &mark {
                Mark::Key(k) => Instance::Key(k.clone()),
                Mark::Index(i) => Instance::Index(*i),
            };
            marks.push(mark);
            parent.child(schema, target, path)
        })
        .collect();
    Batch::new(Mode::Members(marks), jobs)
}

/// Schedule remaining members after successful location marks are known.
pub(super) fn unevaluated<'a>(
    job: &Job<'a>,
    evaluated: &Marks,
    value: &Value,
) -> Vec<Instruction<'a>> {
    let mut instructions = Vec::new();
    if let (Value::Array(values), Some(schema)) = (
        value,
        job.schema()
            .get("unevaluatedItems")
            .filter(|schema| is_schema(schema)),
    ) {
        let members = values
            .iter()
            .enumerate()
            .filter(|(index, _)| !evaluated.indices.contains(index))
            .map(|(index, _)| (Mark::Index(index), schema, format!("{}/{index}", job.path)));
        instructions.push(aggregate(job, members, "must not have unevaluated items"));
    }
    if let (Value::Object(values), Some(schema)) = (
        value,
        job.schema()
            .get("unevaluatedProperties")
            .filter(|schema| is_schema(schema)),
    ) {
        let members = diagnostics::entries(values)
            .into_iter()
            .filter(|(key, _)| !evaluated.keys.contains(*key))
            .map(|(key, _)| {
                (
                    Mark::Key(key.clone()),
                    schema,
                    format!("{}/{key}", job.path),
                )
            });
        instructions.push(aggregate(
            job,
            members,
            "must not have unevaluated properties",
        ));
    }
    instructions
}

/// Suppress member details in favor of one containing-instance failure.
fn aggregate<'a>(
    parent: &Job<'a>,
    members: impl Iterator<Item = (Mark, &'a Value, String)>,
    message: &'static str,
) -> Instruction<'a> {
    let mut batch = members_batch(parent, members);
    if let Mode::Members(marks) = batch.mode {
        batch.mode = Mode::Aggregate { marks, message };
    }
    Instruction::Children(batch)
}

/// Recognize an array consisting entirely of dependency property names.
fn names(schema: &Value) -> bool {
    schema
        .as_array()
        .is_some_and(|names| names.iter().all(Value::is_string))
}
