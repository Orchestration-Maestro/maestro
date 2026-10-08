//! A small terminal model that replays escapes to learn the style of each visible scalar.

use std::collections::BTreeSet;

/// Colours, attributes and hyperlink in effect for one scalar.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Style {
    attributes: BTreeSet<u32>,
    foreground: Option<String>,
    background: Option<String>,
    link: Option<String>,
}

/// Every visible scalar of `text` with the style a fresh terminal shows it in.
pub fn replay(text: &str) -> Vec<(char, Style)> {
    let mut style = Style::default();
    let mut cells = Vec::new();
    let mut rest = text;
    while let Some(scalar) = rest.chars().next() {
        if let Some(length) = escape_length(rest).filter(|_| scalar == '\u{1b}') {
            apply(&rest[..length], &mut style);
            rest = &rest[length..];
            continue;
        }
        cells.push((scalar, style.clone()));
        rest = &rest[scalar.len_utf8()..];
    }
    cells
}

/// Length of the escape at the start of `rest`, if it is one the model understands.
fn escape_length(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    match bytes.get(1)? {
        b'[' => rest.find(['m', 'G', 'K', 'H', 'J']).map(|end| end + 1),
        b']' | b'_' => (2..bytes.len()).find_map(|at| match bytes[at] {
            0x07 => Some(at + 1),
            0x1b if bytes.get(at + 1) == Some(&b'\\') => Some(at + 2),
            _ => None,
        }),
        _ => None,
    }
}

/// Applies one escape to the style.
fn apply(code: &str, style: &mut Style) {
    if let Some(body) = code.strip_prefix("\u{1b}]8;") {
        let body = body.trim_end_matches(['\u{7}', '\\', '\u{1b}']);
        let uri = body.split_once(';').map_or("", |(_, uri)| uri);
        style.link = (!uri.is_empty()).then(|| body.to_owned());
    } else if let Some(params) = code
        .strip_prefix("\u{1b}[")
        .and_then(|c| c.strip_suffix('m'))
    {
        sgr(params, style);
    }
}

/// Applies the parameters of a select-graphic-rendition escape.
fn sgr(params: &str, style: &mut Style) {
    let parts: Vec<&str> = params.split(';').collect();
    let mut at = 0;
    while at < parts.len() {
        let code = parts[at].parse::<u32>().ok();
        let colour = |count: usize| parts.get(at..at + count).map(|c| c.join(";"));
        match (code, parts.get(at + 1).copied()) {
            (Some(38 | 48), Some("5")) if at + 2 < parts.len() => {
                set_colour(style, code, colour(3));
                at += 3;
                continue;
            }
            (Some(38 | 48), Some("2")) if at + 4 < parts.len() => {
                set_colour(style, code, colour(5));
                at += 5;
                continue;
            }
            _ => {}
        }
        match code {
            None | Some(0) if params.is_empty() || code == Some(0) => {
                *style = Style {
                    link: style.link.clone(),
                    ..Style::default()
                }
            }
            Some(on @ (1 | 2 | 3 | 4 | 5 | 7 | 8 | 9)) => {
                style.attributes.insert(on);
            }
            Some(21) => {
                style.attributes.remove(&1);
            }
            Some(22) => {
                style.attributes.remove(&1);
                style.attributes.remove(&2);
            }
            Some(off @ (23 | 24 | 25 | 27 | 28 | 29)) => {
                style.attributes.remove(&(off - 20));
            }
            Some(39) => style.foreground = None,
            Some(49) => style.background = None,
            Some(c @ (30..=37 | 90..=97)) => style.foreground = Some(c.to_string()),
            Some(c @ (40..=47 | 100..=107)) => style.background = Some(c.to_string()),
            _ => {}
        }
        at += 1;
    }
}

/// Stores an extended colour on the layer its leading parameter selects.
fn set_colour(style: &mut Style, code: Option<u32>, colour: Option<String>) {
    if code == Some(38) {
        style.foreground = colour;
    } else {
        style.background = colour;
    }
}

/// Visible non-whitespace scalars of every line, each replayed from a fresh terminal.
pub fn replay_lines(lines: &[String]) -> Vec<(char, Style)> {
    lines
        .iter()
        .flat_map(|line| replay(line))
        .filter(|(scalar, _)| !scalar.is_whitespace())
        .collect()
}
