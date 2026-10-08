//! Repair of JSON string literals and parsing of streamed JSON values.
use std::fmt::Write;

/// Escape raw controls and invalid backslashes inside JSON string literals.
#[must_use]
pub fn repair_json(json: &str) -> String {
    let mut output = String::with_capacity(json.len());
    let mut chars = json.chars().peekable();
    let mut in_string = false;
    while let Some(ch) = chars.next() {
        match (in_string, ch) {
            (_, '"') => {
                in_string = !in_string;
                output.push(ch);
            }
            (true, '\\') => {
                output.push('\\');
                if chars
                    .peek()
                    .is_some_and(|next| "\"\\/bfnrtu".contains(*next))
                    && let Some(next) = chars.next()
                {
                    output.push(next);
                } else {
                    output.push('\\');
                }
            }
            (true, '\u{00}'..='\u{1f}') => match ch {
                '\u{08}' => output.push_str("\\b"),
                '\u{0c}' => output.push_str("\\f"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                _ => {
                    let _ = write!(output, "\\u{:04x}", u32::from(ch));
                }
            },
            _ => output.push(ch),
        }
    }
    output
}

/// Parse the original JSON, retrying repaired string literals only when changed.
///
/// # Errors
/// Returns the native strict-reader cause from the last attempted parse.
pub fn parse_json_with_repair(json: &str) -> Result<serde_json::Value, crate::DiagnosticErrorInfo> {
    serde_json::from_str(json)
        .or_else(|original| {
            let repaired = repair_json(json);
            if repaired == json {
                Err(original)
            } else {
                serde_json::from_str(&repaired)
            }
        })
        .map_err(|error| crate::DiagnosticErrorInfo {
            name: None,
            message: error.to_string(),
            stack: None,
            code: None,
        })
}

/// Recognize the whitespace accepted around streamed JSON.
pub(super) fn whitespace(ch: char) -> bool {
    matches!(ch, '\u{09}'..='\u{0d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// Parse complete or useful incomplete JSON, falling back to an empty object.
/// Strictly parsed scalar values, including null, are preserved.
#[must_use]
pub fn parse_streaming_json(partial_json: Option<&str>) -> serde_json::Value {
    let Some(input) = partial_json.filter(|text| !text.trim_matches(whitespace).is_empty()) else {
        return serde_json::json!({});
    };
    parse_json_with_repair(input)
        .ok()
        .or_else(|| partial(input))
        .or_else(|| partial(&repair_json(input)))
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Byte cursor over the incomplete JSON input.
struct Cursor<'a> {
    /// Borrowed JSON text being parsed.
    input: &'a str,
    /// Byte offset of the next token.
    position: usize,
}

impl Cursor<'_> {
    /// Read the current byte without advancing.
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.position).copied()
    }

    /// Advance past JSON whitespace.
    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.position += 1;
        }
    }

    /// Decode a string, completing or trimming an unfinished escape when needed.
    fn string(&mut self) -> Option<String> {
        let start = self.position;
        self.position += 1;
        let mut escape = None;
        while let Some(byte) = self.peek() {
            self.position += 1;
            match byte {
                b'"' => return serde_json::from_str(&self.input[start..self.position]).ok(),
                b'\\' => {
                    escape = Some(self.position - 1);
                    self.position += usize::from(self.peek().is_some());
                }
                _ => {}
            }
        }
        let completed = format!("{}\"", &self.input[start..]);
        serde_json::from_str(&completed).ok().or_else(|| {
            let end = escape?;
            serde_json::from_str(&format!("{}\"", &self.input[start..end])).ok()
        })
    }

    /// Read a boolean, null or numeric token, completing partial literals.
    fn atom(&mut self) -> Option<serde_json::Value> {
        let tail = &self.input[self.position..];
        for (literal, value) in [
            ("true", serde_json::Value::Bool(true)),
            ("false", serde_json::Value::Bool(false)),
            ("null", serde_json::Value::Null),
        ] {
            if tail.starts_with(literal) || literal.starts_with(tail) {
                self.position += literal.len().min(tail.len());
                return Some(value);
            }
        }
        self.number()
    }

    /// Parse a number after removing a trailing incomplete fraction or exponent.
    fn number(&mut self) -> Option<serde_json::Value> {
        let start = self.position;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.position += 1;
        }
        let token = &self.input[start..self.position];
        let completed = ["e+", "e-", "E+", "E-", "e", "E", "."]
            .iter()
            .find_map(|suffix| {
                token
                    .strip_suffix(suffix)
                    .filter(|prefix| *suffix != "." || !prefix.contains('.'))
            })
            .filter(|prefix| !prefix.contains(['e', 'E']));
        let number = serde_json::from_str::<serde_json::Number>(token)
            .ok()
            .or_else(|| serde_json::from_str::<serde_json::Number>(completed?).ok())?;
        self.skip_space();
        if !matches!(self.peek(), None | Some(b',' | b']' | b'}')) {
            return None;
        }
        Some(serde_json::Value::Number(number))
    }
}

/// Accumulated members of an unfinished JSON container.
enum Members {
    /// Object members with a pending key awaiting its value.
    Object {
        /// Completed key-value pairs in insertion order.
        values: serde_json::Map<String, serde_json::Value>,
        /// Decoded key for the next value.
        key: Option<String>,
    },
    /// Array elements accumulated in input order.
    Array(Vec<serde_json::Value>),
}

/// One open container on the iterative parsing stack.
struct Frame {
    /// Values accumulated for this container.
    members: Members,
    /// Whether this container has stopped accepting members.
    finished: bool,
}

impl Frame {
    /// Start an empty object or array from its opening delimiter.
    fn new(open: u8) -> Self {
        let members = if open == b'{' {
            Members::Object {
                values: serde_json::Map::new(),
                key: None,
            }
        } else {
            Members::Array(Vec::new())
        };
        Self {
            members,
            finished: false,
        }
    }

    /// Read an object key and colon, or recognize a container boundary.
    fn prepare(&mut self, cursor: &mut Cursor<'_>) {
        cursor.skip_space();
        if let Members::Object { key, .. } = &mut self.members {
            if cursor.peek() != Some(b'"') {
                self.finished = true;
                return;
            }
            *key = cursor.string();
            cursor.skip_space();
            if key.is_none() || cursor.peek() != Some(b':') {
                self.finished = true;
                return;
            }
            cursor.position += 1;
            cursor.skip_space();
        }
        if matches!(cursor.peek(), None | Some(b']' | b'}')) {
            self.finished = true;
        }
    }

    /// Attach a completed value and consume its following separator.
    fn accept(&mut self, value: serde_json::Value, cursor: &mut Cursor<'_>) {
        match &mut self.members {
            Members::Object { values, key } => {
                if let Some(key) = key.take() {
                    values.insert(key, value);
                }
            }
            Members::Array(values) => values.push(value),
        }
        cursor.skip_space();
        if cursor.peek() == Some(b',') {
            cursor.position += 1;
        } else {
            self.finished = true;
        }
    }

    /// Build the container value and consume an available closing delimiter.
    fn finish(self, cursor: &mut Cursor<'_>) -> serde_json::Value {
        let (close, value) = match self.members {
            Members::Object { values, .. } => (b'}', serde_json::Value::Object(values)),
            Members::Array(values) => (b']', serde_json::Value::Array(values)),
        };
        if cursor.peek() == Some(close) {
            cursor.position += 1;
        }
        value
    }
}

/// Recover a useful value from incomplete JSON without recursive descent.
fn partial(input: &str) -> Option<serde_json::Value> {
    let mut cursor = Cursor {
        input: input.trim_matches(whitespace),
        position: 0,
    };
    let mut frames: Vec<Frame> = Vec::new();
    loop {
        if let Some(frame) = frames.last_mut().filter(|frame| !frame.finished) {
            frame.prepare(&mut cursor);
        }
        if frames.last().is_some_and(|frame| frame.finished) {
            let value = frames.pop()?.finish(&mut cursor);
            if let Some(parent) = frames.last_mut() {
                parent.accept(value, &mut cursor);
                continue;
            }
            return Some(value);
        }
        cursor.skip_space();
        if let Some(open @ (b'{' | b'[')) = cursor.peek() {
            cursor.position += 1;
            frames.push(Frame::new(open));
            continue;
        }
        let value = if cursor.peek() == Some(b'"') {
            cursor.string().map(serde_json::Value::String)
        } else {
            cursor.atom()
        };
        if let Some(parent) = frames.last_mut() {
            if let Some(value) = value {
                parent.accept(value, &mut cursor);
            } else {
                parent.finished = true;
            }
        } else {
            return value.filter(|value| {
                !value.is_null() && (!value.is_number() || cursor.position == cursor.input.len())
            });
        }
    }
}
