use super::coercion::Kind;
use num_traits::ToPrimitive;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn type_errors(schema: &Value, value: &Value, path: &str) -> Vec<String> {
    if schema == &Value::Bool(false) {
        return vec![render(path, "schema is false")];
    }
    match schema.get("type") {
        Some(Value::String(_)) => {}
        Some(Value::Array(kinds)) if kinds.iter().all(Value::is_string) => {}
        _ => return Vec::new(),
    }
    let kinds = super::coercion::types(schema);
    if kinds.iter().any(|kind| type_matches(value, *kind)) {
        return Vec::new();
    }
    let message = if schema.get("type").is_some_and(Value::is_array) {
        format!(
            "must be either {}",
            kinds
                .iter()
                .map(|kind| kind.name())
                .collect::<Vec<_>>()
                .join(" or ")
        )
    } else {
        format!("must be {}", kinds[0].name())
    };
    vec![render(path, &message)]
}

fn type_matches(value: &Value, kind: Kind<'_>) -> bool {
    match kind {
        Kind::Unknown(_) => true,
        Kind::NonJson(_) => false,
        _ => super::coercion::matches(value, kind),
    }
}

pub(super) fn required_errors(
    schema: &Value,
    object: &serde_json::Map<String, Value>,
    path: &str,
) -> Vec<String> {
    let Some(required) = schema
        .get("required")
        .and_then(Value::as_array)
        .filter(|names| names.iter().all(Value::is_string))
    else {
        return Vec::new();
    };
    let missing: Vec<_> = required
        .iter()
        .filter_map(Value::as_str)
        .filter(|key| !object.contains_key(*key))
        .collect();
    let Some(first) = missing.first() else {
        return Vec::new();
    };
    vec![render(
        &format!("{path}/{first}"),
        &format!("must have required properties {}", missing.join(", ")),
    )]
}

pub(super) fn render(path: &str, message: &str) -> String {
    let display = path.strip_prefix('/').unwrap_or(path).replace('/', ".");
    format!(
        "  - {}: {message}",
        if display.is_empty() { "root" } else { &display }
    )
}

pub(super) fn number_errors(schema: &Value, value: &Value, path: &str) -> Vec<String> {
    let Some(number) = value.as_f64() else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    for (keyword, comparison) in [
        ("exclusiveMaximum", "<"),
        ("exclusiveMinimum", ">"),
        ("maximum", "<="),
        ("minimum", ">="),
        ("multipleOf", "multiple of"),
    ] {
        let Some(limit) = schema.get(keyword).and_then(Value::as_f64) else {
            continue;
        };
        let valid = match keyword {
            "exclusiveMaximum" => number < limit,
            "exclusiveMinimum" => number > limit,
            "maximum" => number <= limit,
            "minimum" => number >= limit,
            _ => multiple_of(number, limit),
        };
        if !valid {
            errors.push(render(
                path,
                &format!(
                    "must be {comparison} {}",
                    ryu_js::Buffer::new().format(limit)
                ),
            ));
        }
    }
    errors
}

fn multiple_of(dividend: f64, divisor: f64) -> bool {
    if dividend.fract() == 0.0 && (1.0 / divisor).fract() == 0.0 {
        return true;
    }
    let remainder = dividend % divisor;
    remainder.abs().min((remainder - divisor).abs()) < 1e-10
}

pub(super) fn literal_errors(schema: &Value, value: &Value, path: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(constant) = schema.get("const")
        && !equal(value, constant)
    {
        errors.push(render(path, "must be equal to constant"));
    }
    if let Some(choices) = schema.get("enum").and_then(Value::as_array)
        && !choices.iter().any(|choice| equal(value, choice))
    {
        errors.push(render(path, "must be equal to one of the allowed values"));
    }
    errors
}

pub(super) fn equal(left: &Value, right: &Value) -> bool {
    let mut pairs = vec![(left, right)];
    while let Some((left, right)) = pairs.pop() {
        match (left, right) {
            (Value::Number(left), Value::Number(right)) if left.as_f64() == right.as_f64() => {}
            (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
                pairs.extend(left.iter().zip(right));
            }
            (Value::Object(left), Value::Object(right)) if left.len() == right.len() => {
                let Some(members) = object_pairs(left, right) else {
                    return false;
                };
                pairs.extend(members);
            }
            _ if left == right => {}
            _ => return false,
        }
    }
    true
}

fn object_pairs<'a>(
    left: &'a serde_json::Map<String, Value>,
    right: &'a serde_json::Map<String, Value>,
) -> Option<Vec<(&'a Value, &'a Value)>> {
    left.iter()
        .map(|(key, value)| Some((value, right.get(key)?)))
        .collect()
}

pub(super) fn string_errors(schema: &Value, value: &Value, path: &str) -> Vec<String> {
    let Some(text) = value.as_str() else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    let Some(length) = text.graphemes(true).count().to_f64() else {
        return Vec::new();
    };
    for (keyword, comparison) in [("maxLength", "more"), ("minLength", "fewer")] {
        let Some(limit) = schema.get(keyword).and_then(Value::as_f64) else {
            continue;
        };
        let valid = if keyword == "maxLength" {
            length <= limit
        } else {
            length >= limit
        };
        if !valid {
            errors.push(render(
                path,
                &format!(
                    "must not have {comparison} than {} characters",
                    ryu_js::Buffer::new().format(limit)
                ),
            ));
        }
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str)
        && !super::formats::check(format, text)
    {
        errors.push(render(path, &format!("must match format \"{format}\"")));
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str)
        && !regress::Regex::with_flags(pattern, "u").is_ok_and(|regex| regex.find(text).is_some())
    {
        errors.push(render(path, &format!("must match pattern \"{pattern}\"")));
    }
    errors
}
