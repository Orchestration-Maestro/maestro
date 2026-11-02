//! Line syntax of Git configuration files.
use super::super::native::trim;

/// One meaningful line of a configuration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::workspace::git) enum Item {
    /// A section header such as `[remote "origin"]`.
    Header {
        /// Section name as written; an unreadable header leaves it empty.
        name: String,
        /// Quoted subsection with its escapes resolved.
        subsection: Option<String>,
    },
    /// A `key = value` line, or a bare key without a value.
    Entry {
        /// Variable name as written.
        key: String,
        /// Value with its quotes and any inline comment removed.
        value: Option<String>,
    },
}

/// Read every header and entry, skipping blank and comment lines.
pub(in crate::workspace::git) fn parse(text: &str) -> Vec<Item> {
    text.lines()
        .filter_map(|line| parse_line(trim(line)))
        .collect()
}

/// Entries under headers named `section` that carry no subsection, in file order.
pub(in crate::workspace::git) fn plain_entries<'a>(
    items: &'a [Item],
    section: &'a str,
) -> impl Iterator<Item = (&'a str, Option<&'a str>)> + 'a {
    items
        .iter()
        .scan(false, move |inside, item| {
            Some(match item {
                Item::Header { name, subsection } => {
                    *inside = subsection.is_none() && name.eq_ignore_ascii_case(section);
                    None
                }
                Item::Entry { key, value } => inside.then_some((key.as_str(), value.as_deref())),
            })
        })
        .flatten()
}

/// Whether a value reads as true: a bare key, `true`, `yes`, `on` or a nonzero integer.
pub(in crate::workspace::git) fn is_true(value: Option<&str>) -> bool {
    let Some(value) = value else { return true };
    ["true", "yes", "on"]
        .iter()
        .any(|word| value.eq_ignore_ascii_case(word))
        || value.parse::<i64>().is_ok_and(|number| number != 0)
}

/// Classify one trimmed line.
fn parse_line(line: &str) -> Option<Item> {
    if line.is_empty() || line.starts_with(['#', ';']) {
        return None;
    }
    if line.starts_with('[') {
        return Some(parse_header(line));
    }
    Some(match line.split_once('=') {
        Some((key, value)) => Item::Entry {
            key: trim(key).to_owned(),
            value: Some(normalize_value(value)),
        },
        None => Item::Entry {
            key: line.to_owned(),
            value: None,
        },
    })
}

/// Split a header into its name and quoted subsection; a malformed header names no section.
fn parse_header(line: &str) -> Item {
    let unnamed = Item::Header {
        name: String::new(),
        subsection: None,
    };
    let Some(inner) = header_body(line) else {
        return unnamed;
    };
    let Some((name, rest)) = inner.split_once(maestro_text::is_whitespace_scalar) else {
        return Item::Header {
            name: inner.to_owned(),
            subsection: None,
        };
    };
    match trim(rest)
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(subsection) => Item::Header {
            name: name.to_owned(),
            subsection: Some(unescape(subsection)),
        },
        None => unnamed,
    }
}

/// The text between the brackets when only a comment follows the closing one.
fn header_body(line: &str) -> Option<&str> {
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ']' if !in_quotes => {
                let rest = trim(&line[index + 1..]);
                return (rest.is_empty() || rest.starts_with(['#', ';'])).then(|| &line[1..index]);
            }
            _ => {}
        }
    }
    None
}

/// Resolve each backslash escape to the character it protects.
fn unescape(text: &str) -> String {
    let mut resolved = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            resolved.extend(chars.next());
        } else {
            resolved.push(ch);
        }
    }
    resolved
}

/// Drop an inline comment that follows whitespace outside quotes, then the surrounding quotes.
fn normalize_value(value: &str) -> String {
    let value = trim(value);
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' | ';'
                if !in_quotes
                    && value[..index]
                        .chars()
                        .next_back()
                        .is_some_and(maestro_text::is_whitespace_scalar) =>
            {
                return unquote(trim(&value[..index]));
            }
            _ => {}
        }
    }
    unquote(value)
}

/// Remove one pair of surrounding quotes.
fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_owned()
}
