//! Filesystem pattern preparation before component compilation.
use super::{Platform, drive_index, slash_split};
use std::collections::{BTreeSet, HashMap, HashSet};
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
    merge_covered(&mut parts);
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}
/// Alternative positions grouped by their first component.
type ByFirst = HashMap<String, BTreeSet<usize>>;
/// Folds each alternative into the first later alternative it merges with.
///
/// Two alternatives can only merge when their first components are equal, one is `**`
/// followed by the other, or one is `*`, so only those groups are compared.
fn merge_covered(parts: &mut [Vec<String>]) {
    let mut by_first = ByFirst::new();
    for (at, part) in parts.iter().enumerate() {
        track(&mut by_first, part, at);
    }
    for first in 0..parts.len().saturating_sub(1) {
        let Some((next, merged)) = merge_target(parts, &by_first, first) else {
            continue;
        };
        untrack(&mut by_first, &parts[first], first);
        untrack(&mut by_first, &parts[next], next);
        parts[first].clear();
        track(&mut by_first, &merged, next);
        parts[next] = merged;
    }
}
/// Records where an alternative sits under its first component.
fn track(by_first: &mut ByFirst, part: &[String], at: usize) {
    if let Some(head) = part.first() {
        by_first.entry(head.clone()).or_default().insert(at);
    }
}
/// Forgets an alternative that is about to change.
fn untrack(by_first: &mut ByFirst, part: &[String], at: usize) {
    if let Some(positions) = part.first().and_then(|head| by_first.get_mut(head)) {
        positions.remove(&at);
    }
}
/// Finds the earliest later alternative that merges with the one at `first`.
fn merge_target(
    parts: &[Vec<String>],
    by_first: &ByFirst,
    first: usize,
) -> Option<(usize, Vec<String>)> {
    let part = &parts[first];
    let head = part.first()?;
    let attempt = |next: usize| merge(part, &parts[next]).map(|merged| (next, merged));
    if head == "*" {
        return (first + 1..parts.len()).find_map(attempt);
    }
    let mut keys = vec![head.as_str(), "*", "**"];
    if head == "**" {
        keys.extend(part.get(1).map(String::as_str));
    }
    let mut best: Option<(usize, Vec<String>)> = None;
    for key in keys {
        let end = best.as_ref().map_or(parts.len(), |(next, _)| *next);
        let found = by_first.get(key).and_then(|positions| {
            positions
                .range(first + 1..end)
                .find_map(|&next| attempt(next))
        });
        best = found.or(best);
    }
    best
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
#[cfg(test)]
mod tests {
    use super::{merge, merge_covered};
    /// The pairwise reference: each alternative folds into the first later one it merges with.
    fn merge_pairwise(parts: &mut [Vec<String>]) {
        for first in 0..parts.len().saturating_sub(1) {
            let target = (first + 1..parts.len())
                .find_map(|next| merge(&parts[first], &parts[next]).map(|merged| (next, merged)));
            if let Some((next, merged)) = target {
                parts[first].clear();
                parts[next] = merged;
            }
        }
    }
    /// A copy of `prefix` with `last` appended.
    fn appended<T: Clone>(prefix: &[T], last: T) -> Vec<T> {
        let mut grown = prefix.to_vec();
        grown.push(last);
        grown
    }
    /// Every alternative of one to `longest` components drawn from `alphabet`.
    fn alternatives(alphabet: &[&str], longest: usize) -> Vec<Vec<String>> {
        let symbols: Vec<String> = alphabet.iter().map(|symbol| (*symbol).to_owned()).collect();
        let mut all = Vec::new();
        let mut layer = vec![Vec::new()];
        for _ in 0..longest {
            layer = layer
                .iter()
                .flat_map(|prefix| {
                    symbols
                        .iter()
                        .map(|symbol| appended(prefix, symbol.clone()))
                })
                .collect();
            all.extend(layer.iter().cloned());
        }
        all
    }
    /// Checks every list of up to `count` alternatives against the pairwise reference.
    fn assert_same_folds(alphabet: &[&str], longest: usize, count: usize) {
        let pool = alternatives(alphabet, longest);
        let mut lists: Vec<Vec<Vec<String>>> = vec![Vec::new()];
        for _ in 0..count {
            lists = lists
                .iter()
                .flat_map(|list| pool.iter().map(|part| appended(list, part.clone())))
                .collect();
            for list in &lists {
                let (mut indexed, mut pairwise) = (list.clone(), list.clone());
                merge_covered(&mut indexed);
                merge_pairwise(&mut pairwise);
                assert_eq!(indexed, pairwise, "{list:?}");
            }
        }
    }
    #[test]
    fn indexed_merge_folds_alternatives_like_the_pairwise_scan() {
        assert_same_folds(&["a", "b", "*", "**", ".d", ""], 2, 3);
        assert_same_folds(&["a", "*", "**"], 3, 3);
    }
}
