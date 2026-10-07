//! Shared scalar text semantics for replay and argument coercion.

pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(|c| {
        matches!(c,
            '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
            '\u{205f}' | '\u{3000}' | '\u{feff}'
        )
    })
}

pub(crate) fn number_string(number: &serde_json::Number) -> Option<String> {
    number
        .as_f64()
        .map(|value| ryu_js::Buffer::new().format(value).to_string())
}

pub(crate) fn pretty_json(value: &serde_json::Value) -> String {
    fn write(value: &serde_json::Value, depth: usize, out: &mut String) {
        grow(|| {
            use serde_json::Value;
            match value {
                Value::Number(n) => out.push_str(ryu_js::Buffer::new().format(n.as_f64().unwrap())),
                Value::Array(values) if !values.is_empty() => {
                    out.push_str("[\n");
                    for (i, v) in values.iter().enumerate() {
                        out.push_str(&"  ".repeat(depth + 1));
                        write(v, depth + 1, out);
                        if i + 1 < values.len() {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    out.push_str(&"  ".repeat(depth));
                    out.push(']');
                }
                Value::Object(values) if !values.is_empty() => {
                    out.push_str("{\n");
                    for (i, (key, v)) in values.iter().enumerate() {
                        out.push_str(&"  ".repeat(depth + 1));
                        out.push_str(&serde_json::to_string(key).unwrap());
                        out.push_str(": ");
                        write(v, depth + 1, out);
                        if i + 1 < values.len() {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    out.push_str(&"  ".repeat(depth));
                    out.push('}');
                }
                _ => out.push_str(&serde_json::to_string(value).unwrap()),
            }
        });
    }
    let mut out = String::new();
    let ordered = OwnedJson::new(crate::records::types::ordered_json(clone_json(value)));
    write(&ordered, 0, &mut out);
    out
}

// Use the serialization adapter's stack-growth parameters for owned recursion.
pub(crate) fn grow<T>(f: impl FnOnce() -> T) -> T {
    let defaults = serde_stacker::Deserializer::new(());
    stacker::maybe_grow(defaults.red_zone, defaults.stack_size, f)
}
pub(crate) fn clone_json(value: &serde_json::Value) -> serde_json::Value {
    grow(|| match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(clone_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, v)| (key.clone(), clone_json(v)))
                .collect(),
        ),
        value => value.clone(),
    })
}
pub(crate) fn drop_json(value: serde_json::Value) {
    let mut values = vec![value];
    while let Some(value) = values.pop() {
        match value {
            serde_json::Value::Array(items) => values.extend(items),
            serde_json::Value::Object(items) => values.extend(items.into_values()),
            _ => {}
        }
    }
}

// Keeps intermediate JSON safe to discard on ordinary and early returns.
pub(crate) struct OwnedJson(serde_json::Value);
impl OwnedJson {
    pub(crate) fn new(value: serde_json::Value) -> Self {
        Self(value)
    }
    pub(crate) fn into_value(mut self) -> serde_json::Value {
        std::mem::take(&mut self.0)
    }
}
impl std::ops::Deref for OwnedJson {
    type Target = serde_json::Value;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for OwnedJson {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for OwnedJson {
    fn drop(&mut self) {
        drop_json(std::mem::take(&mut self.0));
    }
}
pub(crate) fn replace_json(target: &mut serde_json::Value, next: serde_json::Value) {
    drop_json(std::mem::replace(target, next));
}
