//! Filesystem pattern preparation before component compilation.
use super::{Platform, drive_index, slash_split};
use std::collections::HashSet;
/// Expands, reduces and merges ordered filesystem alternatives.
pub(super) fn prepare(pattern: &str, platform: Platform) -> Vec<Vec<String>> {
    if pattern.is_empty() {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut parts: Vec<Vec<String>> = super::brace_expand(pattern, Some(10_000))
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .map(|p| {
            slash_split(&p, platform)
                .into_iter()
                .map(str::to_owned)
                .collect()
        })
        .collect();
    let mut at = 0;
    while at < parts.len() {
        while let Some(other) = reduce(&mut parts[at], platform) {
            parts.push(other);
        }
        at += 1;
    }
    for first in 0..parts.len().saturating_sub(1) {
        for next in first + 1..parts.len() {
            if let Some(merged) = merge(&parts[first], &parts[next]) {
                parts[first].clear();
                parts[next] = merged;
                break;
            }
        }
    }
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}
/// Reduces one alternative; a returned split still needs preparation.
fn reduce(parts: &mut Vec<String>, platform: Platform) -> Option<Vec<String>> {
    loop {
        parts.dedup_by(|a, b| a == "**" && b == "**");
        let root = root_length(parts, platform);
        while root > 0 && parts.get(root).is_some_and(|p| p == "..") {
            parts.remove(root);
        }
        let before = parts.len();
        let split = parts.windows(4).position(|w| {
            w[0] == "**"
                && w[1] == ".."
                && w[2..]
                    .iter()
                    .all(|p| !matches!(p.as_str(), "" | "." | ".."))
        });
        let other = split.map(|at| {
            parts.remove(at);
            let mut other = parts.clone();
            other[at] = "**".into();
            other
        });
        let end = parts.len().saturating_sub(1);
        *parts = parts
            .drain(..)
            .enumerate()
            .filter_map(|(at, part)| {
                (at == 0
                    || at == end
                    || (at == 1 && part.is_empty())
                    || !matches!(part.as_str(), "" | "."))
                .then_some(part)
            })
            .collect();
        if parts.first().is_some_and(|p| p == ".")
            && parts.len() == 2
            && matches!(parts[1].as_str(), "" | ".")
        {
            parts.pop();
        }
        while let Some(at) = parts.windows(2).enumerate().find_map(|(at, w)| {
            (w[1] == ".." && !matches!(w[0].as_str(), "" | "." | ".." | "**") && at >= root)
                .then_some(at)
        }) {
            let dot = at == 0 && parts.get(at + 2).is_some_and(|p| p == "**");
            parts.drain(at..at + 2);
            if dot {
                parts.insert(at, ".".into());
            }
        }
        if parts.is_empty() {
            parts.push(String::new());
        }
        if other.is_some() || parts.len() >= before {
            return other;
        }
    }
}
/// Protects native drive and UNC identity during parent cancellation.
fn root_length(parts: &[String], platform: Platform) -> usize {
    if platform != Platform::Windows {
        return 0;
    }
    let borrowed: Vec<_> = parts.iter().map(String::as_str).collect();
    drive_index(&borrowed).map_or_else(
        || {
            if borrowed.starts_with(&["", ""]) && borrowed.len() >= 4 {
                4
            } else {
                0
            }
        },
        |index| index + 1,
    )
}
/// Chooses a consistent covering alternative without sorting its components.
fn merge(a: &[String], b: &[String]) -> Option<Vec<String>> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let (mut left, mut right) = (a, b);
    let mut chosen = None;
    let mut result = Vec::new();
    while let (Some((x, xs)), Some((y, ys))) = (left.split_first(), right.split_first()) {
        if x == y {
            result.push(x.clone());
            (left, right) = (xs, ys);
        } else if x == "**" && xs.first() == Some(y) {
            result.push(x.clone());
            left = xs;
        } else if y == "**" && ys.first() == Some(x) {
            result.push(y.clone());
            right = ys;
        } else {
            let side = if covers(x, y) {
                false
            } else if covers(y, x) {
                true
            } else {
                return None;
            };
            if chosen.is_some_and(|chosen| chosen != side) {
                return None;
            }
            chosen = Some(side);
            result.push(if side { y.clone() } else { x.clone() });
            (left, right) = (xs, ys);
        }
    }
    Some(result)
}
/// A single wildcard covers non-dot non-recursive components.
fn covers(a: &str, b: &str) -> bool {
    a == "*" && !b.starts_with('.') && !matches!(b, "" | "**")
}
