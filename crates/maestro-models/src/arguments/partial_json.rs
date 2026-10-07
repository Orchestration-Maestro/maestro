//! Default partial JSON parser. The bundled MIT notice covers the local port.
use serde_json::{Map, Value};

pub(super) fn parse(text: &str) -> Result<Value, ()> {
    let text = crate::scalar::trim(text);
    Parser { text, at: 0 }.any()
}
struct Parser<'a> {
    text: &'a str,
    at: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    fn blank(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }
    fn any(&mut self) -> Result<Value, ()> {
        crate::scalar::grow(|| {
            self.blank();
            match self.peek() {
                None => return Err(()),
                Some(b'"') => return self.string(),
                Some(b'{') => return Ok(self.object()),
                Some(b'[') => return Ok(self.array()),
                _ => {}
            }
            let remaining = &self.text[self.at..];
            for (word, value) in [
                ("null", Value::Null),
                ("true", Value::Bool(true)),
                ("false", Value::Bool(false)),
            ] {
                if remaining.starts_with(word) || word.starts_with(remaining) {
                    self.at += word.len();
                    return Ok(value);
                }
            }
            if ["Infinity", "-Infinity", "NaN"].iter().any(|word| {
                remaining.starts_with(word) || (word.starts_with(remaining) && (remaining != "-"))
            }) {
                return Err(());
            }
            self.number()
        })
    }
    fn string(&mut self) -> Result<Value, ()> {
        let start = self.at;
        let mut escape = false;
        self.at += 1;
        while self.at < self.text.len()
            && (self.peek() != Some(b'"') || (escape && self.text.as_bytes()[self.at - 1] == b'\\'))
        {
            escape = if self.peek() == Some(b'\\') {
                !escape
            } else {
                false
            };
            self.at += 1;
        }
        if self.peek() == Some(b'"') {
            self.at += 1;
            return super::json_parse::decode(&self.text[start..self.at - usize::from(escape)])
                .map_err(|_| ());
        }
        let candidate = format!("{}\"", &self.text[start..self.at - usize::from(escape)]);
        super::json_parse::decode(&candidate)
            .or_else(|_| {
                let end = self.text.rfind('\\').unwrap_or(0).max(start);
                super::json_parse::decode(&format!("{}\"", &self.text[start..end]))
            })
            .map_err(|_| ())
    }
    fn object(&mut self) -> Value {
        self.at += 1;
        self.blank();
        let mut object = Map::new();
        while self.peek() != Some(b'}') {
            self.blank();
            if self.at >= self.text.len() {
                return Value::Object(object);
            }
            let key = match self.string() {
                Ok(Value::String(key)) => key,
                Ok(value) => {
                    crate::scalar::drop_json(value);
                    return Value::Object(object);
                }
                Err(()) => return Value::Object(object),
            };
            self.blank();
            self.at += 1;
            let Ok(value) = self.any() else {
                return Value::Object(object);
            };
            if key != "__proto__" {
                if let Some(old) = object.insert(key, value) {
                    crate::scalar::drop_json(old);
                }
            } else {
                crate::scalar::drop_json(value);
            }
            self.blank();
            if self.peek() == Some(b',') {
                self.at += 1;
            }
        }
        self.at += 1;
        Value::Object(object)
    }
    fn array(&mut self) -> Value {
        self.at += 1;
        let mut array = Vec::new();
        while self.peek() != Some(b']') {
            let Ok(value) = self.any() else {
                return Value::Array(array);
            };
            array.push(value);
            self.blank();
            if self.peek() == Some(b',') {
                self.at += 1;
            }
        }
        self.at += 1;
        Value::Array(array)
    }
    fn number(&mut self) -> Result<Value, ()> {
        let start = self.at;
        if start == 0 {
            self.at = self.text.len();
            if self.text.parse::<f64>().is_ok_and(|n| !n.is_finite()) {
                return Err(());
            }
            return super::json_parse::decode(self.text)
                .or_else(|_| {
                    super::json_parse::decode(&self.text[..self.text.rfind('e').unwrap_or(0)])
                })
                .map_err(|_| ());
        }
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        while self.peek().is_some_and(|c| !b",]}".contains(&c)) {
            self.at += 1;
        }
        if self.text[start..self.at]
            .parse::<f64>()
            .is_ok_and(|n| !n.is_finite())
        {
            return Err(());
        }
        super::json_parse::decode(&self.text[start..self.at])
            .or_else(|_| {
                let end = self.text.rfind('e').unwrap_or(0).max(start);
                super::json_parse::decode(&self.text[start..end])
            })
            .map_err(|_| ())
    }
}
