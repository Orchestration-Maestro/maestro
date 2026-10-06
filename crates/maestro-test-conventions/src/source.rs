use std::path::{Path, PathBuf};

pub(crate) fn files(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", path.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir()
            && path
                .file_name()
                .is_none_or(|name| name != "target" && name != ".git")
        {
            result.extend(files(&path)?);
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}

pub(crate) fn line(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

struct Region {
    start: usize,
    end: usize,
    comment: Option<(usize, usize)>,
}

pub(crate) fn comments(source: &str) -> Vec<(usize, &str)> {
    regions(source)
        .into_iter()
        .filter_map(|region| {
            region
                .comment
                .map(|(start, end)| (start, &source[start..end]))
        })
        .collect()
}

pub(crate) struct Token<'a> {
    pub(crate) text: &'a str,
    pub(crate) start: usize,
}

pub(crate) fn tokens(source: &str) -> Vec<Token<'_>> {
    let mut regions = regions(source).into_iter().peekable();
    let mut result = Vec::new();
    let mut index = 0;
    while index < source.len() {
        if let Some(region) = regions.peek()
            && index == region.start
        {
            if region.comment.is_none() {
                result.push(Token {
                    text: &source[region.start..region.end],
                    start: region.start,
                });
            }
            index = region.end;
            regions.next();
            continue;
        }
        let start = index;
        let raw_identifier = source[index..].starts_with("r#")
            && source[index + 2..]
                .chars()
                .next()
                .is_some_and(|character| character.is_alphabetic() || character == '_');
        if raw_identifier {
            index += 2;
        }
        let character = source[index..]
            .chars()
            .next()
            .expect("nonempty source tail");
        index += character.len_utf8();
        if character.is_alphabetic() || character == '_' {
            while let Some(character) = source[index..].chars().next() {
                if regions.peek().is_some_and(|region| region.start == index) {
                    break;
                }
                if !character.is_alphanumeric() && character != '_' {
                    break;
                }
                index += character.len_utf8();
            }
        }
        if !character.is_whitespace() {
            result.push(Token {
                text: &source[if raw_identifier { start + 2 } else { start }..index],
                start,
            });
        }
    }
    result
}

// Lex strings and character literals before recognizing comment delimiters.
// Nested block comments are included; raw strings may contain arbitrary hashes.
fn regions(source: &str) -> Vec<Region> {
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            let start = index + 2;
            index = start;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            result.push(Region {
                start: start - 2,
                end: index,
                comment: Some((start, index)),
            });
        } else if bytes[index..].starts_with(b"/*") {
            let start = index + 2;
            index = start;
            let mut depth = 1;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            let end = if depth == 0 { index - 2 } else { index };
            result.push(Region {
                start: start - 2,
                end: index,
                comment: Some((start, end)),
            });
        } else if let Some(end) = raw_string_end(bytes, index) {
            result.push(Region {
                start: index,
                end,
                comment: None,
            });
            index = end;
        } else if bytes[index] == b'"' {
            let end = quoted_end(bytes, index, b'"').unwrap_or(bytes.len());
            result.push(Region {
                start: index,
                end,
                comment: None,
            });
            index = end;
        } else if bytes[index] == b'\'' {
            // Lifetimes have no closing quote; only consume a valid character shape.
            let tail = &source[index + 1..];
            let length = if tail.starts_with('\\') {
                quoted_end(bytes, index, b'\'').map(|end| end - index)
            } else {
                tail.chars()
                    .next()
                    .map(|character| 2 + character.len_utf8())
            };
            if let Some(length) = length
                && bytes.get(index + length - 1) == Some(&b'\'')
            {
                result.push(Region {
                    start: index,
                    end: index + length,
                    comment: None,
                });
                index += length;
            } else {
                index += 1;
            }
        } else {
            // Advancing bytes is safe here: slicing only follows ASCII delimiters.
            index += 1;
        }
    }
    result
}

fn quoted_end(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
        } else if bytes[index] == quote {
            return Some(index + 1);
        } else {
            index += 1;
        }
    }
    None
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes[start] != b'r' {
        return None;
    }
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - start - 1;
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes
                .get(index + 1..index + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }
    Some(bytes.len())
}
