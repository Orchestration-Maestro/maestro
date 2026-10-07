//! Default partial JSON parser. The bundled MIT notice covers the local port.
use serde_json::{Map, Value};

pub(super) fn parse_json(text: &str) -> Result<Value, ()> {
    let text = crate::scalar::trim(text);
    Parser {
        text: text.encode_utf16().collect(),
        at: 0,
    }
    .parse_any()
}
struct Parser {
    text: Vec<u16>,
    at: usize,
}
impl Parser {
    fn peek(&self) -> Option<u16> {
        self.text.get(self.at).copied()
    }
    fn skip_blank(&mut self) {
        while matches!(self.peek(), Some(32 | 10 | 13 | 9)) {
            self.at += 1;
        }
    }
    fn parse_any(&mut self) -> Result<Value, ()> {
        crate::scalar::grow(|| {
            self.skip_blank();
            match self.peek() {
                None => return Err(()),
                Some(34) => return self.parse_str(),
                Some(123) => return Ok(self.parse_obj()),
                Some(91) => return Ok(self.parse_arr()),
                _ => {}
            }
            let remaining = String::from_utf16_lossy(&self.text[self.at..]);
            for (word, value) in [
                ("null", Value::Null),
                ("true", Value::Bool(true)),
                ("false", Value::Bool(false)),
            ] {
                if remaining.starts_with(word) || word.starts_with(&remaining) {
                    self.at += word.len();
                    return Ok(value);
                }
            }
            if ["Infinity", "-Infinity", "NaN"].iter().any(|word| {
                remaining.starts_with(word) || (word.starts_with(&remaining) && (remaining != "-"))
            }) {
                return Err(());
            }
            self.parse_num()
        })
    }
    fn parse_str(&mut self) -> Result<Value, ()> {
        let start = self.at;
        let mut escape = false;
        self.at += 1;
        while self.at < self.text.len()
            && (self.peek() != Some(34) || (escape && self.text[self.at - 1] == 92))
        {
            escape = if self.peek() == Some(92) {
                !escape
            } else {
                false
            };
            self.at += 1;
        }
        if self.peek() == Some(34) {
            self.at += 1;
            return super::json_parse::decode(&String::from_utf16_lossy(
                &self.text[start..self.at - usize::from(escape)],
            ))
            .map_err(|_| ());
        }
        let candidate = format!(
            "{}\"",
            String::from_utf16_lossy(&self.text[start..self.at - usize::from(escape)])
        );
        super::json_parse::decode(&candidate)
            .or_else(|_| {
                let end = self
                    .text
                    .iter()
                    .rposition(|c| *c == 92)
                    .unwrap_or(0)
                    .max(start);
                super::json_parse::decode(&format!(
                    "{}\"",
                    String::from_utf16_lossy(&self.text[start..end])
                ))
            })
            .map_err(|_| ())
    }
    fn parse_obj(&mut self) -> Value {
        self.at += 1;
        self.skip_blank();
        let mut object = Map::new();
        while self.peek() != Some(125) {
            self.skip_blank();
            if self.at >= self.text.len() {
                return Value::Object(object);
            }
            let key = match self.parse_str() {
                Ok(Value::String(key)) => key,
                Ok(value) => {
                    crate::scalar::drop_json(value);
                    return Value::Object(object);
                }
                Err(()) => return Value::Object(object),
            };
            self.skip_blank();
            self.at += 1;
            let Ok(value) = self.parse_any() else {
                return Value::Object(object);
            };
            if key != "__proto__" {
                if let Some(old) = object.insert(key, value) {
                    crate::scalar::drop_json(old);
                }
            } else {
                crate::scalar::drop_json(value);
            }
            self.skip_blank();
            if self.peek() == Some(44) {
                self.at += 1;
            }
        }
        self.at += 1;
        Value::Object(object)
    }
    fn parse_arr(&mut self) -> Value {
        self.at += 1;
        let mut array = Vec::new();
        while self.peek() != Some(93) {
            let Ok(value) = self.parse_any() else {
                return Value::Array(array);
            };
            array.push(value);
            self.skip_blank();
            if self.peek() == Some(44) {
                self.at += 1;
            }
        }
        self.at += 1;
        Value::Array(array)
    }
    fn parse_num(&mut self) -> Result<Value, ()> {
        let start = self.at;
        if start == 0 {
            self.at = self.text.len();
            let text = String::from_utf16_lossy(&self.text);
            if text
                .trim_end_matches([' ', '\n', '\r', '\t'])
                .parse::<f64>()
                .is_ok_and(|n| !n.is_finite())
            {
                return Err(());
            }
            return super::json_parse::decode(&text)
                .or_else(|_| super::json_parse::decode(&text[..text.rfind('e').unwrap_or(0)]))
                .map_err(|_| ());
        }
        if self.peek() == Some(45) {
            self.at += 1;
        }
        while self.peek().is_some_and(|c| ![44, 93, 125].contains(&c)) {
            self.at += 1;
        }
        let text = String::from_utf16_lossy(&self.text[start..self.at]);
        if text
            .trim_end_matches([' ', '\n', '\r', '\t'])
            .parse::<f64>()
            .is_ok_and(|n| !n.is_finite())
        {
            return Err(());
        }
        super::json_parse::decode(&text)
            .or_else(|_| {
                let end = text.rfind('e').unwrap_or(0);
                super::json_parse::decode(&text[..end])
            })
            .map_err(|_| ())
    }
}
