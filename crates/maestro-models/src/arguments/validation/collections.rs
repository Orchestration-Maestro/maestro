use num_traits::ToPrimitive;
use serde_json::{Map, Value};

use super::{
    check::{Batch, Instance, Instruction, Job, Mark, Mode},
    check::{is_schema, schema_array},
    diagnostics, scalars,
};

type Patterns<'a> = Vec<(regress::Regex, &'a Value)>;

pub(super) fn object<'a>(job: &Job<'a>, instructions: &mut Vec<Instruction<'a>>) {
    let Instance::Json(Value::Object(value)) = job.value else {
        return;
    };
    let patterns: Result<Patterns<'a>, _> = job
        .schema
        .get("patternProperties")
        .and_then(Value::as_object)
        .filter(|properties| properties.values().all(is_schema))
        .into_iter()
        .flat_map(|patterns| diagnostics::entries(patterns).into_iter())
        .map(|(pattern, schema)| {
            regress::Regex::with_flags(pattern, "u").map(|regex| (regex, schema))
        })
        .collect();
    let patterns = match patterns {
        Ok(patterns) => patterns,
        Err(error) => {
            instructions.push(Instruction::Errors(vec![scalars::render(
                &job.path,
                &error.to_string(),
            )]));
            return;
        }
    };
    instructions.push(Instruction::Errors(scalars::required_errors(
        job.schema, value, &job.path,
    )));
    additional_properties(job, value, &patterns, instructions);
    for keyword in ["dependencies", "dependentRequired", "dependentSchemas"] {
        dependencies(job, value, keyword, instructions);
    }
    pattern_properties(job, value, &patterns, instructions);
    properties(job, value, instructions);
    property_names(job, value, instructions);
    instructions.push(bounds(job, value.len(), "Properties", "properties"));
}

fn additional_properties<'a>(
    job: &Job<'a>,
    value: &'a Map<String, Value>,
    patterns: &Patterns<'a>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(schema) = job
        .schema
        .get("additionalProperties")
        .filter(|schema| schema.is_boolean() || schema.is_object())
    else {
        return;
    };
    let declared = job
        .schema
        .get("properties")
        .and_then(Value::as_object)
        .filter(|properties| properties.values().all(is_schema));
    let members = diagnostics::entries(value)
        .into_iter()
        .filter(|(key, _)| {
            !declared.is_some_and(|declared| declared.contains_key(*key))
                && !patterns.iter().any(|(regex, _)| regex.find(key).is_some())
        })
        .map(|(key, value)| {
            (
                Mark::Key(key.clone()),
                schema,
                value,
                format!("{}/{key}", job.path),
            )
        });
    let mut batch = members_batch(members);
    let Mode::Members(marks) = batch.mode else {
        return;
    };
    batch.mode = Mode::Aggregate {
        marks,
        message: "must not have additional properties".to_owned(),
    };
    instructions.push(Instruction::Children(batch));
}

fn pattern_properties<'a>(
    job: &Job<'a>,
    value: &'a Map<String, Value>,
    patterns: &Patterns<'a>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    for (regex, schema) in patterns {
        let members = diagnostics::entries(value)
            .into_iter()
            .filter(|(key, _)| regex.find(key).is_some())
            .map(|(key, value)| {
                (
                    Mark::Key(key.clone()),
                    *schema,
                    value,
                    format!("{}/{key}", job.path),
                )
            });
        instructions.push(Instruction::Children(members_batch(members)));
    }
}

fn properties<'a>(
    job: &Job<'a>,
    value: &'a Map<String, Value>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    if let Some(properties) = job
        .schema
        .get("properties")
        .and_then(Value::as_object)
        .filter(|properties| properties.values().all(is_schema))
    {
        let members = diagnostics::entries(properties)
            .into_iter()
            .filter_map(|(key, schema)| {
                Some((
                    Mark::Key(key.clone()),
                    schema,
                    value.get(key)?,
                    format!("{}/{key}", job.path),
                ))
            });
        instructions.push(Instruction::Children(members_batch(members)));
    }
}

fn dependencies<'a>(
    job: &Job<'a>,
    value: &Map<String, Value>,
    keyword: &str,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(dependencies) = job.schema.get(keyword).and_then(Value::as_object) else {
        return;
    };
    if !dependencies.values().all(|schema| match keyword {
        "dependentSchemas" => is_schema(schema),
        "dependentRequired" => names(schema),
        _ => is_schema(schema) || names(schema),
    }) {
        return;
    }
    for (key, schema) in diagnostics::entries(dependencies) {
        if !value.contains_key(key) {
            continue;
        }
        if let Some(names) = schema.as_array() {
            let declared: Vec<_> = names.iter().filter_map(Value::as_str).collect();
            if declared.iter().any(|name| !value.contains_key(*name))
                && keyword != "dependentSchemas"
            {
                instructions.push(Instruction::Errors(vec![scalars::render(
                    &job.path,
                    &format!(
                        "must have properties {} when property {key} is present",
                        declared.join(", ")
                    ),
                )]));
            }
        } else if keyword != "dependentRequired" && (schema.is_boolean() || schema.is_object()) {
            let jobs = [Job {
                schema,
                value: job.value,
                path: job.path.clone(),
            }]
            .into();
            instructions.push(Instruction::Children(Batch {
                mode: Mode::All,
                jobs,
                results: Vec::new(),
            }));
        }
    }
}

fn property_names<'a>(
    job: &Job<'a>,
    value: &'a Map<String, Value>,
    instructions: &mut Vec<Instruction<'a>>,
) {
    let Some(schema) = job
        .schema
        .get("propertyNames")
        .filter(|schema| is_schema(schema))
    else {
        return;
    };
    let names = diagnostics::entries(value)
        .into_iter()
        .map(|(key, _)| key.clone())
        .collect();
    let jobs = diagnostics::entries(value)
        .into_iter()
        .map(|(key, _)| Job {
            schema,
            value: Instance::Name(key),
            path: job.path.clone(),
        })
        .collect();
    instructions.push(Instruction::Children(Batch {
        mode: Mode::Names(names),
        jobs,
        results: Vec::new(),
    }));
}

fn bounds<'a>(job: &Job<'a>, size: usize, suffix: &str, noun: &str) -> Instruction<'a> {
    let mut errors = Vec::new();
    for (prefix, comparison) in [("min", "fewer"), ("max", "more")] {
        let Some(limit) = job
            .schema
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

pub(super) fn array<'a>(job: &Job<'a>, instructions: &mut Vec<Instruction<'a>>) {
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

fn members_batch<'a>(
    members: impl Iterator<Item = (Mark, &'a Value, &'a Value, String)>,
) -> Batch<'a> {
    let mut marks = Vec::new();
    let jobs = members
        .map(|(mark, schema, value, path)| {
            marks.push(mark);
            Job {
                schema,
                value: Instance::Json(value),
                path,
            }
        })
        .collect();
    Batch {
        mode: Mode::Members(marks),
        jobs,
        results: Vec::new(),
    }
}

pub(super) fn unevaluated<'a>(
    job: &Job<'a>,
    result: &super::check::Evaluation,
) -> Vec<Instruction<'a>> {
    let mut instructions = Vec::new();
    if let (Instance::Json(Value::Array(values)), Some(schema)) = (
        job.value,
        job.schema
            .get("unevaluatedItems")
            .filter(|schema| is_schema(schema)),
    ) {
        let members = values
            .iter()
            .enumerate()
            .filter(|(index, _)| !result.indices.contains(index))
            .map(|(index, value)| {
                (
                    Mark::Index(index),
                    schema,
                    value,
                    format!("{}/{index}", job.path),
                )
            });
        instructions.push(aggregate(members, "must not have unevaluated items"));
    }
    if let (Instance::Json(Value::Object(values)), Some(schema)) = (
        job.value,
        job.schema
            .get("unevaluatedProperties")
            .filter(|schema| is_schema(schema)),
    ) {
        let members = diagnostics::entries(values)
            .into_iter()
            .filter(|(key, _)| !result.keys.contains(*key))
            .map(|(key, value)| {
                (
                    Mark::Key(key.clone()),
                    schema,
                    value,
                    format!("{}/{key}", job.path),
                )
            });
        instructions.push(aggregate(members, "must not have unevaluated properties"));
    }
    instructions
}

fn aggregate<'a>(
    members: impl Iterator<Item = (Mark, &'a Value, &'a Value, String)>,
    message: &str,
) -> Instruction<'a> {
    let mut batch = members_batch(members);
    if let Mode::Members(marks) = batch.mode {
        batch.mode = Mode::Aggregate {
            marks,
            message: message.to_owned(),
        };
    }
    Instruction::Children(batch)
}

fn names(schema: &Value) -> bool {
    schema
        .as_array()
        .is_some_and(|names| names.iter().all(Value::is_string))
}
