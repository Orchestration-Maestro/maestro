//! Package path spellings and font-family formatting.
/// Split at the final forward slash, retaining the folder's slash.
pub(super) fn split_path(path: &str) -> (&str, &str) {
    path.split_at(path.rfind('/').map_or(0, |i| i + 1))
}
/// Remove exactly one package-root slash.
pub(super) fn normalize_path(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}
/// Resolve a relationship target against the package URL base.
pub(super) fn resolve_path(path: &str, base: &str) -> String {
    url::Url::parse(&format!("http://docx/{base}"))
        .and_then(|base| base.join(path))
        .map_or_else(
            |_| format!("{base}{path}"),
            |url| {
                url.as_str()
                    .strip_prefix("http://docx/")
                    .unwrap_or(url.as_str())
                    .into()
            },
        )
}
/// Admit archive dot segments without decoding escapes or backslashes.
pub(super) fn archive_name(path: &str) -> String {
    let mut result = Vec::new();
    let mut parts = path.split('/').enumerate().peekable();
    while let Some((index, part)) = parts.next() {
        match part {
            "." => {}
            "" if index != 0 && parts.peek().is_some() => {}
            ".." => {
                result.pop();
            }
            _ => result.push(part),
        }
    }
    result.join("/")
}

/// Preserve valid font-family values, enclosing names rejected by CSS.
pub(super) fn enclose_font_family(name: &str) -> String {
    let chars: Vec<_> = name.chars().collect();
    let interior = chars
        .get(1..chars.len().saturating_sub(1))
        .unwrap_or_default();
    let terminators = interior.iter().filter(|&&c| line_terminator(c)).count();
    let enclose = chars.first().is_some_and(|c| !matches!(c, '\'' | '"'))
        && chars.last().is_some_and(|c| !matches!(c, '\'' | '"'))
        && interior.iter().any(|&c| {
            maestro_text::is_whitespace_scalar(c) && terminators == usize::from(line_terminator(c))
        });
    let candidate = if enclose {
        format!("'{name}'")
    } else {
        name.into()
    };
    if name.is_empty() || valid_font_family(&candidate) {
        return candidate;
    }
    maestro_text::css::string(name)
}

/// Line boundaries excluded by the original font enclosure operation.
fn line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
/// Check the property's string, identifier, family-list and variable forms.
fn valid_font_family(value: &str) -> bool {
    if let Some(variable) = value.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) {
        return variable.starts_with("--") && valid_font_family(variable);
    }
    let mut chars = value.chars().peekable();
    let mut family = false;
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' | '\r' | '\x0c' => continue,
            ',' if family => {
                family = false;
                continue;
            }
            '\'' | '"' if !family => {
                if !font_string(&mut chars, c) {
                    return false;
                }
                while chars
                    .next_if(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c'))
                    .is_some()
                {}
                if chars.peek().is_some_and(|c| *c != ',') {
                    return false;
                }
            }
            _ if font_identifier(&mut chars, c) => {}
            _ => return false,
        }
        family = true;
    }
    family
}
/// Consume a quoted CSS string, including its valid implicit EOF close.
fn font_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) -> bool {
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if chars.next() == Some('\r') {
                    chars.next_if_eq(&'\n');
                }
            }
            '\n' | '\r' | '\x0c' => return false,
            c if c == quote => break,
            _ => {}
        }
    }
    true
}
/// Consume one CSS identifier, retaining escapes rather than decoding them.
fn font_identifier(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, first: char) -> bool {
    let start = if first == '-' {
        chars.peek().copied().unwrap_or(' ')
    } else {
        first
    };
    if !(start.is_ascii_alphabetic() || matches!(start, '_' | '-' | '\\') || !start.is_ascii()) {
        return false;
    }
    let mut current = Some(first);
    while let Some(c) = current {
        if c == '\\' {
            let Some(escaped) = chars.next().filter(|c| !matches!(c, '\n' | '\r' | '\x0c')) else {
                return false;
            };
            if escaped.is_ascii_hexdigit() {
                let _ = (1..6)
                    .take_while(|_| chars.next_if(char::is_ascii_hexdigit).is_some())
                    .count();
                chars.next_if(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c'));
            }
        } else if !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-') || !c.is_ascii()) {
            return false;
        }
        current = chars.next_if(|c| !matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c' | ','));
    }
    true
}
