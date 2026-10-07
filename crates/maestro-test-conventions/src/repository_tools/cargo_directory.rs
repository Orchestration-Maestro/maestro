use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn resolve(
    cargo: &OsStr,
    root: &Path,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<PathBuf, String> {
    let output = Command::new(cargo)
        .env_clear()
        .envs(environment.iter().cloned())
        .args(["metadata", "--format-version=1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("repository tools: resolve Cargo directory: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let json = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let value = field(&json, "target_directory")
        .ok_or("repository tools: Cargo target_directory missing")?;
    decode(string(value)).map(PathBuf::from)
}

pub(super) fn decode(text: &str) -> Result<String, String> {
    let mut decoded = String::new();
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let character = match chars.next() {
            Some('"') => '"',
            Some('\\') => '\\',
            Some('/') => '/',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('b') => '\u{0008}',
            Some('f') => '\u{000c}',
            Some('u') => {
                let hex = chars.by_ref().take(4).collect::<String>();
                let value = u32::from_str_radix(&hex, 16).map_err(|error| error.to_string())?;
                char::from_u32(value).ok_or("repository tools: invalid Cargo path escape")?
            }
            _ => return Err("repository tools: invalid Cargo path escape".into()),
        };
        decoded.push(character);
    }
    Ok(decoded)
}

// Cargo supplies valid JSON; spans are traversed without parsing unrelated data.
pub(super) fn end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut nesting = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, &byte) in bytes.iter().enumerate().skip(start) {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
                if nesting == 0 {
                    return index + 1;
                }
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => nesting += 1,
                b'}' | b']' if nesting > 0 => {
                    nesting -= 1;
                    if nesting == 0 {
                        return index + 1;
                    }
                }
                b',' | b'}' | b']' if nesting == 0 => return index,
                _ => (),
            }
        }
    }
    text.len()
}

pub(super) fn field<'a>(object: &'a str, name: &str) -> Option<&'a str> {
    let mut cursor = object.find('{')? + 1;
    loop {
        while object
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b',')
        {
            cursor += 1;
        }
        if object.as_bytes().get(cursor) != Some(&b'"') {
            return None;
        }
        let key_end = end(object, cursor);
        let key = string(&object[cursor..key_end]);
        cursor = key_end;
        while object
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b':')
        {
            cursor += 1;
        }
        let value_end = end(object, cursor);
        if key == name {
            return Some(object[cursor..value_end].trim());
        }
        cursor = value_end;
    }
}

pub(super) fn string(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or("")
}
