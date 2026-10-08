//! Array assertions with indexed child evaluations and contains cardinalities.

use num_traits::ToPrimitive;
use serde_json::Value;

use super::{
    Batch, Instance, Instruction, Job, Mark, Mode, is_schema, members_batch, scalars, schema_array,
};

/// Schedule array assertions in keyword order and detect structural duplicates.
pub(in super::super) fn array<'a>(job: &Job<'a>, instructions: &mut Vec<Instruction<'a>>) {
    let Instance::Json(Value::Array(values)) = job.value else {
        return;
    };
    additional_items(job, values, instructions);
    contains(job, values, "contains", instructions);
    items(job, values, instructions);
    contains(job, values, "maxContains", instructions);
    instructions.push(single_bound(job, values.len(), "maxItems", "more"));
    contains(job, values, "minContains", instructions);
    instructions.push(single_bound(job, values.len(), "minItems", "fewer"));
    if let Some(prefix) = job.schema.get("prefixItems").and_then(schema_array) {
        let members = prefix
            .iter()
            .zip(values)
            .enumerate()
            .map(|(index, (schema, value))| {
                (
                    Mark::Index(index),
                    schema,
                    value,
                    format!("{}/{index}", job.path),
                )
            });
        instructions.push(Instruction::Children(members_batch(members)));
    }
    if job.schema.get("uniqueItems") == Some(&Value::Bool(true))
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
    if let Some(limit) = job.schema.get(keyword).and_then(Value::as_f64)
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
fn additional_items<'a>(
    job: &Job<'a>,
    values: &'a [Value],
    instructions: &mut Vec<Instruction<'a>>,
) {
    let (Some(items), Some(schema)) = (
        job.schema.get("items").and_then(schema_array),
        job.schema
            .get("additionalItems")
            .filter(|schema| is_schema(schema)),
    ) else {
        return;
    };
    let members = values
        .iter()
        .enumerate()
        .skip(items.len())
        .map(|(index, value)| {
            (
                Mark::Index(index),
                schema,
                value,
                format!("{}/{index}", job.path),
            )
        });
    instructions.push(Instruction::Children(members_batch(members)));
}

/// Check tuple positions or homogeneous items after the declared prefix.
fn items<'a>(job: &Job<'a>, values: &'a [Value], instructions: &mut Vec<Instruction<'a>>) {
    let Some(items) = job.schema.get("items") else {
        return;
    };
    let prefix_length = job
        .schema
        .get("prefixItems")
        .and_then(schema_array)
        .map_or(0, <[Value]>::len);
    let members = values.iter().enumerate().filter_map(|(index, value)| {
        let schema = match items {
            Value::Array(schemas) if schemas.iter().all(is_schema) => schemas.get(index)?,
            Value::Bool(_) | Value::Object(_) if index >= prefix_length => items,
            _ => return None,
        };
        Some((
            Mark::Index(index),
            schema,
            value,
            format!("{}/{index}", job.path),
        ))
    });
    instructions.push(Instruction::Children(members_batch(members)));
}

/// Count matching items only when a contains schema is active.
fn contains<'a>(
    job: &Job<'a>,
    values: &'a [Value],
    keyword: &str,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(schema) = job
        .schema
        .get("contains")
        .filter(|schema| is_schema(schema))
    else {
        return;
    };
    let (minimum, maximum, message) = match keyword {
        "contains" if job.schema.get("minContains").and_then(Value::as_f64) == Some(0.0) => return,
        "contains" => (
            Some(1.0),
            None,
            "must contain at least 1 valid item".to_owned(),
        ),
        "minContains" | "maxContains" => {
            let Some(limit) = job.schema.get(keyword).and_then(Value::as_f64) else {
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
                format!(
                    "must contain at {comparison} {} valid {noun}",
                    ryu_js::Buffer::new().format(limit)
                ),
            )
        }
        _ => return,
    };
    let jobs = values
        .iter()
        .map(|value| Job {
            schema,
            value: Instance::Json(value),
            path: job.path.clone(),
        })
        .collect();
    instructions.push(Instruction::Children(Batch {
        mode: Mode::Count {
            minimum,
            maximum,
            message,
        },
        jobs,
        results: Vec::new(),
    }));
}
