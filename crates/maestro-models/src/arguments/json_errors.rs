//! Syntax-error descriptions with UTF-16 source locations.
use crate::{Error, ThrownValue};

pub(super) fn error(text: &str) -> ThrownValue {
    let mut reader = Reader {
        text: text.encode_utf16().collect(),
        at: 0,
    };
    let message = reader
        .value()
        .and_then(|()| {
            reader.blank();
            if reader.at < reader.text.len() {
                Err(reader.location("Unexpected non-whitespace character after JSON"))
            } else {
                Ok(())
            }
        })
        .err()
        .unwrap_or_else(|| reader.location("Number outside binary64 range"));
    ThrownValue::Error(Box::new(Error {
        name: "SyntaxError".into(),
        message,
        stack: None,
        code: None,
    }))
}
struct Reader {
    text: Vec<u16>,
    at: usize,
}
impl Reader {
    fn peek(&self) -> Option<u16> {
        self.text.get(self.at).copied()
    }
    fn blank(&mut self) {
        while matches!(self.peek(), Some(0x20 | 10 | 13 | 9)) {
            self.at += 1;
        }
    }
    fn location(&self, message: &str) -> String {
        let prefix = &self.text[..self.at.min(self.text.len())];
        let line = prefix.iter().filter(|&&c| c == 10).count() + 1;
        let column = prefix
            .iter()
            .rposition(|&c| c == 10)
            .map_or(prefix.len() + 1, |i| prefix.len() - i);
        let suffix = if message.ends_with("JSON") {
            ""
        } else {
            " in JSON"
        };
        format!(
            "{message}{suffix} at position {} (line {line} column {column})",
            self.at
        )
    }
    fn unexpected(&self) -> String {
        let Some(c) = self.peek() else {
            return "Unexpected end of JSON input".into();
        };
        if (48..=57).contains(&c) {
            return self.location("Unexpected number");
        }
        if c == 34 {
            return self.location("Unexpected string");
        }
        let text = &self.text;
        let whole = String::from_utf16_lossy(text);
        if ["undefined", "NaN", "Infinity", "[object Object]"].contains(&whole.as_str()) {
            return format!("\"{whole}\" is not valid JSON");
        }
        let snippet = if text.len() < 20 {
            format!("\"{whole}\"")
        } else if self.at < 10 {
            format!("\"{}\"...", String::from_utf16_lossy(&text[..10]))
        } else if self.at >= text.len() - 10 {
            format!("...\"{}\"", String::from_utf16_lossy(&text[self.at - 10..]))
        } else {
            format!(
                "...\"{}\"...",
                String::from_utf16_lossy(&text[self.at - 10..self.at + 10])
            )
        };
        format!(
            "Unexpected token '{}', {snippet} is not valid JSON",
            String::from_utf16_lossy(&[c])
        )
    }
    fn value(&mut self) -> Result<(), String> {
        crate::scalar::grow(|| {
            self.blank();
            match self.peek() {
                Some(34) => self.string(),
                Some(123) => self.object(),
                Some(91) => self.array(),
                Some(116) => self.literal("true"),
                Some(102) => self.literal("false"),
                Some(110) => self.literal("null"),
                Some(45 | 48..=57) => self.number(),
                _ => Err(self.unexpected()),
            }
        })
    }
    fn literal(&mut self, word: &str) -> Result<(), String> {
        for c in word.encode_utf16() {
            if self.peek() != Some(c) {
                return Err(self.unexpected());
            }
            self.at += 1;
        }
        Ok(())
    }
    fn string(&mut self) -> Result<(), String> {
        self.at += 1;
        loop {
            match self.peek() {
                None => return Err(self.location("Unterminated string")),
                Some(34) => {
                    self.at += 1;
                    return Ok(());
                }
                Some(0..=31) => {
                    return Err(self.location("Bad control character in string literal"));
                }
                Some(92) => {
                    self.at += 1;
                    match self.peek() {
                        None => return Err("Unexpected end of JSON input".into()),
                        Some(117) => {
                            self.at += 1;
                            for _ in 0..4 {
                                if !matches!(self.peek(), Some(48..=57 | 65..=70 | 97..=102)) {
                                    return Err(self.location("Bad Unicode escape"));
                                }
                                self.at += 1;
                            }
                        }
                        Some(34 | 92 | 47 | 98 | 102 | 110 | 114 | 116) => self.at += 1,
                        _ => return Err(self.location("Bad escaped character")),
                    }
                }
                _ => self.at += 1,
            }
        }
    }
    fn number(&mut self) -> Result<(), String> {
        let start = self.at;
        if self.peek() == Some(45) {
            self.at += 1;
            if !matches!(self.peek(), Some(48..=57)) {
                return Err(self.location("No number after minus sign"));
            }
        }
        if self.peek() == Some(48) {
            self.at += 1;
            if matches!(self.peek(), Some(48..=57)) {
                return Err(self.location("Unexpected number"));
            }
        } else {
            while matches!(self.peek(), Some(48..=57)) {
                self.at += 1;
            }
        }
        if self.peek() == Some(46) {
            self.at += 1;
            if !matches!(self.peek(), Some(48..=57)) {
                return Err(self.location("Unterminated fractional number"));
            }
            while matches!(self.peek(), Some(48..=57)) {
                self.at += 1;
            }
        }
        if matches!(self.peek(), Some(101 | 69)) {
            self.at += 1;
            if matches!(self.peek(), Some(43 | 45)) {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(48..=57)) {
                return Err(self.location("Exponent part is missing a number"));
            }
            while matches!(self.peek(), Some(48..=57)) {
                self.at += 1;
            }
        }
        let number = String::from_utf16_lossy(&self.text[start..self.at]);
        if number.parse::<f64>().is_ok_and(|n| !n.is_finite()) {
            self.at = start;
            return Err(self.location("Number outside binary64 range"));
        }
        Ok(())
    }
    fn object(&mut self) -> Result<(), String> {
        self.at += 1;
        self.blank();
        if self.peek() == Some(125) {
            self.at += 1;
            return Ok(());
        }
        if self.peek() != Some(34) {
            return Err(self.location("Expected property name or '}'"));
        }
        loop {
            self.string()?;
            self.blank();
            if self.peek() != Some(58) {
                return Err(self.location("Expected ':' after property name"));
            }
            self.at += 1;
            self.value()?;
            self.blank();
            if self.peek() == Some(125) {
                self.at += 1;
                return Ok(());
            }
            if self.peek() != Some(44) {
                return Err(self.location("Expected ',' or '}' after property value"));
            }
            self.at += 1;
            self.blank();
            if self.peek() != Some(34) {
                return Err(self.location("Expected double-quoted property name"));
            }
        }
    }
    fn array(&mut self) -> Result<(), String> {
        self.at += 1;
        self.blank();
        if self.peek() == Some(93) {
            self.at += 1;
            return Ok(());
        }
        loop {
            self.value()?;
            self.blank();
            if self.peek() == Some(93) {
                self.at += 1;
                return Ok(());
            }
            if self.peek() != Some(44) {
                return Err(self.location("Expected ',' or ']' after array element"));
            }
            self.at += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reader_templates_match_golden_messages() {
        let rows: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/json_errors.json")).unwrap();
        for row in rows.as_array().unwrap() {
            let crate::ThrownValue::Error(error) = super::error(row["input"].as_str().unwrap())
            else {
                panic!("not an error");
            };
            assert_eq!(error.name, row["name"]);
            assert_eq!(error.message, row["message"], "{}", row["input"]);
        }
    }
}
