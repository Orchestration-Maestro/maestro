use crate::source;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Default)]
pub(super) struct WitInputs {
    guest: BTreeSet<std::path::PathBuf>,
    host: BTreeSet<std::path::PathBuf>,
    guest_declared: bool,
    host_declared: bool,
    host_location: Option<String>,
}

impl WitInputs {
    pub(super) fn scan(
        &mut self,
        context: &ScanContext<'_>,
        tokens: &[source::Token<'_>],
    ) -> Result<(), String> {
        let guest_prefix = ["wit_bindgen", ":", ":", "generate", "!"];
        let host_prefix = ["wasmtime", ":", ":", "component", ":", ":", "bindgen", "!"];
        for index in 0..tokens.len() {
            let prefix = if has_prefix(&tokens[index..], &guest_prefix) {
                &guest_prefix[..]
            } else if has_prefix(&tokens[index..], &host_prefix) {
                &host_prefix[..]
            } else {
                continue;
            };
            let location = format!(
                "{}:{}",
                context.path.display(),
                source::line(context.contents, tokens[index].start)
            );
            let error = || {
                format!(
                    "{location}: WIT code-generation input requires review: expected static path"
                )
            };
            self.scan_input(context, &location, &tokens[index + prefix.len()..], error)?;
        }
        Ok(())
    }

    fn scan_input(
        &mut self,
        context: &ScanContext<'_>,
        location: &str,
        tail: &[source::Token<'_>],
        error: impl Fn() -> String + Copy,
    ) -> Result<(), String> {
        let body = macro_body(tail, error)?;
        let mut paths = Vec::new();
        let mut inline = false;
        if body
            .first()
            .and_then(|token| string_literal(token.text))
            .is_some()
        {
            if body.len() != 1 {
                return Err(error());
            }
            // A shorthand string names a world in the manifest-relative default directory.
            paths.push("wit".to_owned());
        } else {
            (paths, inline) = wit_fields(body, context.owner, location, error)?;
        }
        if paths.is_empty() && !inline {
            return Err(error());
        }
        let is_guest = context.owner == "maestro-extensions-wasm";
        if is_guest {
            self.guest_declared = true;
        } else {
            self.host_declared = true;
            self.host_location = Some(location.to_owned());
        }
        for input in paths {
            let root = context.canonical_input(&input, location, error)?;
            if is_guest {
                self.guest.insert(root);
            } else {
                self.host.insert(root);
            }
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<(), String> {
        if self.guest_declared && self.host_declared && self.guest != self.host {
            return Err(format!(
                "{}: host and guest WIT inputs must share the same canonical source roots",
                self.host_location.as_deref().unwrap_or("WIT")
            ));
        }
        Ok(())
    }
}

fn has_prefix(tokens: &[source::Token<'_>], prefix: &[&str]) -> bool {
    tokens.len() >= prefix.len()
        && tokens
            .iter()
            .zip(prefix)
            .all(|(token, text)| token.text == *text)
}

fn string_literal(text: &str) -> Option<String> {
    if let Some(body) = text.strip_prefix('"') {
        return cooked_string(body.strip_suffix('"')?);
    }
    let rest = text.strip_prefix('r')?;
    let quote = rest.find('"')?;
    let hashes = &rest[..quote];
    if !hashes.bytes().all(|byte| byte == b'#') {
        return None;
    }
    let body = rest[quote + 1..].strip_suffix(hashes)?.strip_suffix('"')?;
    Some(body.to_owned())
}

fn cooked_string(body: &str) -> Option<String> {
    let mut characters = body.chars().peekable();
    let mut result = String::new();
    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        let escaped = match characters.next()? {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            '0' => '\0',
            'x' => {
                let high = characters.next()?.to_digit(16)?;
                let low = characters.next()?.to_digit(16)?;
                let value = high * 16 + low;
                if value > 0x7f {
                    return None;
                }
                char::from_u32(value)?
            }
            'u' => unicode_escape(&mut characters)?,
            newline @ ('\n' | '\r') => {
                if newline == '\r' && characters.next()? != '\n' {
                    return None;
                }
                while characters
                    .peek()
                    .is_some_and(|character| matches!(character, ' ' | '\t' | '\n' | '\r'))
                {
                    characters.next();
                }
                continue;
            }
            _ => return None,
        };
        result.push(escaped);
    }
    Some(result)
}

pub(super) struct ScanContext<'a> {
    pub(super) owner: &'a str,
    pub(super) directory: &'a Path,
    pub(super) guest: Option<&'a Path>,
    pub(super) path: &'a Path,
    pub(super) contents: &'a str,
}

impl ScanContext<'_> {
    fn canonical_input(
        &self,
        input: &str,
        location: &str,
        error: impl Fn() -> String,
    ) -> Result<std::path::PathBuf, String> {
        let input = self
            .directory
            .join(input)
            .canonicalize()
            .map_err(|error| format!("{location}: cannot resolve WIT input: {error}"))?;
        let guest = self
            .guest
            .ok_or_else(|| format!("{location}: WIT input has no guest-owned source member"))?;
        if !input.starts_with(guest) {
            return Err(format!(
                "{location}: WIT input must use the guest-owned canonical source: {}",
                input.display()
            ));
        }
        if input.is_dir() {
            Ok(input)
        } else {
            Ok(input.parent().ok_or_else(error)?.to_path_buf())
        }
    }
}

fn macro_body<'a, 's>(
    tail: &'a [source::Token<'s>],
    error: impl Fn() -> String + Copy,
) -> Result<&'a [source::Token<'s>], String> {
    if tail
        .first()
        .is_none_or(|token| !matches!(token.text, "(" | "{" | "["))
    {
        return Err(error());
    }
    let mut stack = Vec::new();
    let mut end = None;
    for (offset, token) in tail.iter().enumerate() {
        match token.text {
            "(" => stack.push(")"),
            "{" => stack.push("}"),
            "[" => stack.push("]"),
            ")" | "}" | "]" => {
                if stack.pop() != Some(token.text) {
                    return Err(error());
                }
                if stack.is_empty() {
                    end = Some(offset);
                    break;
                }
            }
            _ => {}
        }
    }
    Ok(&tail[1..end.ok_or_else(error)?])
}

fn wit_fields(
    body: &[source::Token<'_>],
    owner: &str,
    location: &str,
    error: impl Fn() -> String + Copy,
) -> Result<(Vec<String>, bool), String> {
    let mut paths = Vec::new();
    let mut inline = false;
    let fields = if body.first().is_some_and(|token| token.text == "{") {
        &body[1..body.len() - 1]
    } else {
        body
    };
    let mut depth = 0_usize;
    let mut field_start = true;
    for (offset, token) in fields.iter().enumerate() {
        let is_field = depth == 0
            && field_start
            && fields
                .get(offset + 1)
                .is_some_and(|token| token.text == ":")
            && fields.get(offset + 2).is_none_or(|token| token.text != ":");
        match token.text {
            "(" | "{" | "[" => {
                depth += 1;
                field_start = false;
            }
            ")" | "}" | "]" => depth = depth.checked_sub(1).ok_or_else(error)?,
            "," if depth == 0 => field_start = true,
            _ if depth == 0 => field_start = false,
            _ => {}
        }
        if !is_field {
            continue;
        }
        if token.text == "inline" {
            inline = true;
            if owner != "maestro-extensions-wasm" {
                return Err(format!("{location}: inline WIT must be guest-owned"));
            }
        } else if token.text == "path" {
            paths.extend(path_values(&fields[offset + 2..], error)?);
        }
    }
    Ok((paths, inline))
}

fn path_values(
    value: &[source::Token<'_>],
    error: impl Fn() -> String + Copy,
) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    if let Some(path) = value.first().and_then(|token| string_literal(token.text)) {
        if value
            .get(1)
            .is_some_and(|token| !matches!(token.text, "," | "}"))
        {
            return Err(error());
        }
        paths.push(path);
    } else if value.first().is_some_and(|token| token.text == "[") {
        let mut cursor = 1;
        loop {
            if value.get(cursor).is_some_and(|token| token.text == "]") {
                break;
            }
            paths.push(
                value
                    .get(cursor)
                    .and_then(|token| string_literal(token.text))
                    .ok_or_else(error)?,
            );
            cursor += 1;
            match value.get(cursor).map(|token| token.text) {
                Some(",") => cursor += 1,
                Some("]") => break,
                _ => return Err(error()),
            }
        }
        if value
            .get(cursor + 1)
            .is_some_and(|token| !matches!(token.text, "," | "}"))
        {
            return Err(error());
        }
    } else {
        return Err(error());
    }
    Ok(paths)
}

fn unicode_escape(characters: &mut impl Iterator<Item = char>) -> Option<char> {
    if characters.next()? != '{' {
        return None;
    }
    let mut value = characters.next()?.to_digit(16)?;
    let mut digits = 1;
    loop {
        match characters.next()? {
            '}' => break,
            '_' => {}
            character => {
                digits += 1;
                if digits > 6 {
                    return None;
                }
                value = value * 16 + character.to_digit(16)?;
            }
        }
    }
    char::from_u32(value)
}
