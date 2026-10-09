//! Extended URL and email recognition on authored text runs.
use super::parse::{Kind, Node};
use regress::Regex;
use std::{ops::Range, sync::LazyLock};

/// URL/www/email candidates from the published extended-link grammar.
static CANDIDATES: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"(?:https?://|www\.)[^\s<]+|[A-Za-z0-9._+-]+@[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+")
        .ok()
});
/// Domain labels permit Unicode letters and numbers.
static DOMAIN: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^[\p{L}\p{N}_-]+(?:\.[\p{L}\p{N}_-]+)+", "u").ok());

/// Selects eligible literal link spans without decoding their contents.
fn links(raw: &str) -> Vec<(Range<usize>, String)> {
    let Some(regex) = CANDIDATES.as_ref() else {
        return Vec::new();
    };
    regex
        .find_iter(raw)
        .filter_map(|matched| {
            let start = matched.range().start;
            let candidate = &raw[matched.range()];
            let email = !candidate.starts_with("http://")
                && !candidate.starts_with("https://")
                && !candidate.starts_with("www.");
            let label = if email {
                if candidate.ends_with(['-', '_']) {
                    return None;
                }
                candidate.trim_end_matches('.')
            } else {
                if raw[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| !c.is_whitespace() && !"*_~(".contains(c))
                {
                    return None;
                }
                let mut label =
                    candidate.trim_end_matches(['?', '!', '.', ',', ':', '*', '_', '~']);
                let opens = label.matches('(').count();
                let mut closes = label.matches(')').count();
                while label.ends_with(')') && closes > opens {
                    label = &label[..label.len() - 1];
                    closes -= 1;
                }
                if label.ends_with(';')
                    && let Some((head, tail)) = label.rsplit_once('&')
                    && tail[..tail.len() - 1]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric())
                    && tail.len() > 1
                {
                    label = head;
                }
                if !domain(label) {
                    return None;
                }
                label
            };
            let href = if email {
                format!("mailto:{label}")
            } else if label.starts_with("www.") {
                format!("http://{label}")
            } else {
                label.to_owned()
            };
            Some((start..start + label.len(), href))
        })
        .collect()
}

/// Replaces links within one original run while retaining decoded non-link parts.
pub(super) fn extend(parts: Vec<Node>, source: &str) -> Vec<Node> {
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
        return parts;
    };
    let extent = first.range.start..last.range.end;
    let raw = &source[extent.clone()];
    let mut output = Vec::new();
    let mut end = 0;
    for (range, href) in links(raw) {
        if range.start > end {
            output.push(text(
                &parts,
                source,
                extent.start + end..extent.start + range.start,
            ));
        }
        let label = raw[range.clone()].to_owned();
        let absolute = extent.start + range.start..extent.start + range.end;
        let child = Node {
            kind: Kind::Text(label.clone()),
            range: absolute.clone(),
        };
        output.push(Node {
            kind: Kind::Link(vec![child], label, href),
            range: absolute,
        });
        end = range.end;
    }
    if end == 0 {
        return vec![text(&parts, source, extent)];
    }
    if end < raw.len() {
        output.push(text(&parts, source, extent.start + end..extent.end));
    }
    output
}
/// Decodes the selected non-link interval through its original native text parts.
fn text(parts: &[Node], source: &str, range: Range<usize>) -> Node {
    let decoded: String = parts
        .iter()
        .filter_map(|part| {
            let Kind::Text(value) = &part.kind else {
                return None;
            };
            let start = range.start.max(part.range.start);
            let end = range.end.min(part.range.end);
            if start >= end {
                None
            } else if source[part.range.clone()] == *value {
                Some(source[start..end].to_owned())
            } else {
                Some(value.clone())
            }
        })
        .collect();
    Node {
        kind: Kind::Text(decoded),
        range,
    }
}

/// Checks domain labels and the final two underscore-free segments.
fn domain(label: &str) -> bool {
    let Some(regex) = DOMAIN.as_ref() else {
        return false;
    };
    let text = label
        .strip_prefix("https://")
        .or_else(|| label.strip_prefix("http://"))
        .unwrap_or(label);
    regex.find(text).is_some_and(|domain| {
        !text[domain.range()]
            .rsplit('.')
            .take(2)
            .any(|part| part.contains('_'))
    })
}
