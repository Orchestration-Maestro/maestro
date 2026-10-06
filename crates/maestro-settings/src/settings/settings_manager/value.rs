use super::{Map, Value};
pub(super) fn spread(value: &Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map.clone(),
        Value::Array(array) => array
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v.clone()))
            .collect(),
        Value::String(s) => s
            .encode_utf16()
            .enumerate()
            .map(|(i, c)| (i.to_string(), Value::String(String::from_utf16_lossy(&[c]))))
            .collect(),
        _ => Map::new(),
    }
}
pub(super) fn merge(base: &Value, overrides: &Value) -> Value {
    let mut result = spread(base);
    for (key, value) in spread(overrides) {
        if let (Some(base), Some(over)) =
            (base.get(&key).and_then(Value::as_object), value.as_object())
        {
            let mut merged = base.clone();
            merged.extend(over.clone());
            result.insert(key, Value::Object(merged));
        } else {
            result.insert(key, value);
        }
    }
    Value::Object(result)
}

pub(super) fn convert(mut value: Value) -> Result<Value, super::Error> {
    if !value.is_object() && !value.is_array() {
        return Err(std::io::Error::other(format!(
            "Cannot use 'in' operator to search for 'queueMode' in {}",
            primitive_text(&value)
        ))
        .into());
    }
    if let Some(map) = value.as_object_mut() {
        if !map.contains_key("steeringMode")
            && let Some(queue) = map.shift_remove("queueMode")
        {
            map.insert("steeringMode".into(), queue);
        }
        if !map.contains_key("transport")
            && let Some(enabled) = map.get("websockets").and_then(Value::as_bool)
        {
            map.insert(
                "transport".into(),
                Value::String(if enabled { "websocket" } else { "sse" }.into()),
            );
            map.shift_remove("websockets");
        }
        if let Some(skills) = map.get("skills").and_then(Value::as_object).cloned() {
            if !map.contains_key("enableSkillCommands")
                && let Some(enabled) = skills.get("enableSkillCommands")
            {
                map.insert("enableSkillCommands".into(), enabled.clone());
            }
            if let Some(dirs) = skills
                .get("customDirectories")
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
            {
                map.insert("skills".into(), Value::Array(dirs.clone()));
            } else {
                map.shift_remove("skills");
            }
        }
        if let Some(retry) = map.get_mut("retry").and_then(Value::as_object_mut) {
            let provider = retry
                .get("provider")
                .filter(|p| p.is_object() || p.is_array())
                .map(spread)
                .unwrap_or_default();
            if provider.get("maxRetryDelayMs").is_none_or(Value::is_null)
                && let Some(delay) = retry.get("maxDelayMs").filter(|v| v.is_number()).cloned()
            {
                let mut provider = provider;
                provider.insert("maxRetryDelayMs".into(), delay);
                retry.insert("provider".into(), Value::Object(provider));
            }
            retry.shift_remove("maxDelayMs");
        }
    }
    Ok(value)
}

pub(super) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
pub(super) fn strings(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}
pub(super) fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if value.is_nan() {
        value
    } else if value <= low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}
/// Lexical host join, including later absolute-looking segments as join components.
pub(super) fn join(parts: &[&std::path::Path]) -> std::path::PathBuf {
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let combined = parts
        .iter()
        .map(|p| p.to_string_lossy())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(&separator);
    let mut result = std::path::PathBuf::new();
    for component in std::path::Path::new(&combined).components() {
        match component {
            std::path::Component::CurDir => (),
            std::path::Component::ParentDir => {
                if result.file_name().is_some_and(|n| n != "..") {
                    result.pop();
                } else if !result.has_root() {
                    result.push("..");
                }
            }
            component => result.push(component.as_os_str()),
        }
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    if combined.ends_with(std::path::MAIN_SEPARATOR) || (cfg!(windows) && combined.ends_with('/')) {
        let mut text = result.into_os_string();
        if !text.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
            text.push(&separator);
        }
        return text.into();
    }
    result
}
fn array_index(key: &str) -> Option<u32> {
    let number = key.parse::<u32>().ok()?;
    (number != u32::MAX && number.to_string() == key).then_some(number)
}
pub(super) fn stringify(value: &Value) -> String {
    fn write(value: &Value, depth: usize, out: &mut String) {
        match value {
            Value::Number(number) => {
                let number = number.as_f64().unwrap();
                if number == 0.0 {
                    out.push('0');
                } else if number.is_finite() {
                    out.push_str(ryu_js::Buffer::new().format_finite(number));
                } else {
                    out.push_str("null");
                }
            }
            Value::Object(map) if !map.is_empty() => {
                let mut entries: Vec<_> = map.iter().collect();
                entries.sort_by_key(|(key, _)| array_index(key).map_or((1, 0), |n| (0, n)));
                out.push_str("{\n");
                let len = entries.len();
                for (i, (key, value)) in entries.into_iter().enumerate() {
                    out.push_str(&"  ".repeat(depth + 1));
                    out.push_str(&serde_json::to_string(key).unwrap());
                    out.push_str(": ");
                    write(value, depth + 1, out);
                    if i + 1 < len {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&"  ".repeat(depth));
                out.push('}');
            }
            Value::Array(values) if !values.is_empty() => {
                out.push_str("[\n");
                for (i, value) in values.iter().enumerate() {
                    out.push_str(&"  ".repeat(depth + 1));
                    write(value, depth + 1, out);
                    if i + 1 < values.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&"  ".repeat(depth));
                out.push(']');
            }
            value => out.push_str(&serde_json::to_string(value).unwrap()),
        }
    }
    let mut out = String::new();
    write(value, 0, &mut out);
    out
}
pub(super) fn parse(text: &str) -> Result<Value, super::Error> {
    fn doubles(value: &mut Value) {
        match value {
            Value::Number(number) => {
                let n = number.as_f64().unwrap();
                *value = if n.fract() == 0.0
                    && !(n == 0.0 && n.is_sign_negative())
                    && n >= i64::MIN as f64
                    && n < -(i64::MIN as f64)
                {
                    Value::from(n as i64)
                } else {
                    Value::from(n)
                };
            }
            Value::Array(array) => array.iter_mut().for_each(doubles),
            Value::Object(map) => map.values_mut().for_each(doubles),
            _ => (),
        }
    }
    let mut value = serde_json::from_str(text)?;
    doubles(&mut value);
    Ok(value)
}

pub(super) fn ordered(mut map: Map<String, Value>, keys: &[String]) -> Map<String, Value> {
    let mut result = Map::new();
    for key in keys {
        if let Some(value) = map.shift_remove(key) {
            result.insert(key.clone(), value);
        }
    }
    result.extend(map);
    result
}

pub(super) fn primitive_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        value => stringify(value),
    }
}
