//! Repair of JSON string literals.

/// Escape controls inside strings and double invalid or trailing backslashes.
/// Punctuation and incomplete Unicode escapes remain unchanged.
pub fn repair_json(json: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    let mut chars = json.chars().peekable();
    while let Some(c) = chars.next() {
        if !quoted {
            out.push(c);
            quoted = c == '"';
        } else if c == '"' {
            out.push(c);
            quoted = false;
        } else if c == '\\' {
            match chars.peek().copied() {
                Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u') => {
                    out.push('\\');
                    out.push(chars.next().unwrap());
                }
                _ => out.push_str("\\\\"),
            }
        } else {
            match c {
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\0'..='\u{1f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
                _ => out.push(c),
            }
        }
    }
    out
}

/// Parse complete JSON, retrying only if string repair changes the text.
/// Errors describe the failed original or repaired attempt. Lone surrogate
/// escapes become U+FFFD; numbers outside binary64 range are unreadable.
/// Parsing accepts any JSON root and never validates or executes a tool.
pub fn parse_json_with_repair(json: &str) -> Result<serde_json::Value, crate::ThrownValue> {
    decode(json).or_else(|original| {
        let repaired = repair_json(json);
        if repaired == json {
            Err(original)
        } else {
            decode(&repaired)
        }
    })
}

pub(super) fn decode(json: &str) -> Result<serde_json::Value, crate::ThrownValue> {
    let mut normalized = String::new();
    let mut quoted = false;
    let mut at = 0;
    while at < json.len() {
        let rest = &json[at..];
        let c = rest.chars().next().unwrap();
        if quoted && c == '\\' {
            fn unit(text: &str) -> Option<u16> {
                text.strip_prefix("\\u")
                    .and_then(|s| s.get(..4))
                    .filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
                    .and_then(|s| u16::from_str_radix(s, 16).ok())
            }
            if let Some(high) = unit(rest) {
                if (0xd800..=0xdbff).contains(&high) {
                    if unit(&rest[6..]).is_some_and(|low| (0xdc00..=0xdfff).contains(&low)) {
                        normalized.push_str(&rest[..12]);
                        at += 12;
                        continue;
                    }
                    normalized.push_str("\\ufffd");
                    at += 6;
                    continue;
                }
                if (0xdc00..=0xdfff).contains(&high) {
                    normalized.push_str("\\ufffd");
                    at += 6;
                    continue;
                }
            }
            normalized.push(c);
            at += 1;
            if let Some(c) = json[at..].chars().next() {
                normalized.push(c);
                at += c.len_utf8();
            }
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        normalized.push(c);
        at += c.len_utf8();
    }
    use serde::Deserialize;
    let mut reader = serde_json::Deserializer::from_str(&normalized);
    reader.disable_recursion_limit();
    let value = crate::scalar::OwnedJson::new(
        serde_json::Value::deserialize(serde_stacker::Deserializer::new(&mut reader))
            .map_err(|_| super::json_errors::error(json))?,
    );
    reader.end().map_err(|_| super::json_errors::error(json))?;
    Ok(value.into_value())
}

/// Return an owned best-effort display value from cumulative JSON text.
/// Complete parsing with repair precedes original and repaired partial parsing.
/// Only a partial nullish result or total failure becomes an empty object.
/// This neither validates arguments nor executes a tool.
pub fn parse_streaming_json(partial_json: Option<&str>) -> serde_json::Value {
    let Some(text) = partial_json.filter(|text| !crate::scalar::trim(text).is_empty()) else {
        return serde_json::json!({});
    };
    if let Ok(value) = parse_json_with_repair(text) {
        return value;
    }
    super::partial_json::parse(text)
        .or_else(|_| super::partial_json::parse(&repair_json(text)))
        .ok()
        .filter(|v| !v.is_null())
        .unwrap_or_else(|| serde_json::json!({}))
}
