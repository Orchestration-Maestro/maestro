//! Ordered-character matching with stable multi-token ranking.

use crate::text::utils::is_whitespace_scalar;

/// Whether a query matches, together with its ranking score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuzzyMatch {
    /// Whether the query or its letter/digit-swapped form matched.
    pub matches: bool,
    /// Lower matching scores rank first.
    pub score: f64,
}

/// Match case-insensitively, trying a letter/digit swap only after a failed match.
#[must_use]
pub fn fuzzy_match(query: &str, text: &str) -> FuzzyMatch {
    let query = query.to_lowercase();
    let text = text.to_lowercase();
    let primary = match_ordered(&query, &text);
    if primary.matches {
        return primary;
    }
    let Some(swapped) = swap_tokens(&query) else {
        return primary;
    };
    let secondary = match_ordered(&swapped, &text);
    if secondary.matches {
        FuzzyMatch {
            matches: true,
            score: secondary.score + 5.0,
        }
    } else {
        primary
    }
}

/// Match normalized scalar sequences while retaining the score's operation order.
fn match_ordered(query: &str, text: &str) -> FuzzyMatch {
    let mut wanted = query.chars().peekable();
    let mut score = 0.0;
    let mut last = None;
    let mut consecutive = 0_u32;
    let mut previous = None;
    for (index, character) in text.chars().enumerate() {
        if wanted.peek().is_none() {
            break;
        }
        if wanted.peek() != Some(&character) {
            previous = Some(character);
            continue;
        }
        let boundary = previous.is_none_or(|c| is_whitespace_scalar(c) || "-_./:".contains(c));
        if last == index.checked_sub(1) {
            consecutive += 1;
            score -= f64::from(consecutive) * 5.0;
        } else {
            consecutive = 0;
            if let Some(last) = last {
                score += count_as_float(index - last - 1) * 2.0;
            }
        }
        if boundary {
            score -= 10.0;
        }
        score += count_as_float(index) * 0.1;
        last = Some(index);
        wanted.next();
        previous = Some(character);
    }
    if wanted.peek().is_some() {
        return FuzzyMatch {
            matches: false,
            score: 0.0,
        };
    }
    if !query.is_empty() && query == text {
        score -= 100.0;
    }
    FuzzyMatch {
        matches: true,
        score,
    }
}

/// Convert a position to the floating-point score domain.
fn count_as_float(value: usize) -> f64 {
    u32::try_from(value).map_or_else(
        |_| value.to_string().parse().unwrap_or(f64::INFINITY),
        f64::from,
    )
}

/// Swap exactly one nonempty ASCII letter run with one nonempty digit run.
fn swap_tokens(query: &str) -> Option<String> {
    let first = query.chars().next()?;
    let predicate: fn(char) -> bool = if first.is_ascii_lowercase() {
        |c| c.is_ascii_lowercase()
    } else if first.is_ascii_digit() {
        |c| c.is_ascii_digit()
    } else {
        return None;
    };
    let split = query.find(|c| !predicate(c))?;
    let (left, right) = query.split_at(split);
    let valid = if first.is_ascii_lowercase() {
        right.chars().all(|c| c.is_ascii_digit())
    } else {
        right.chars().all(|c| c.is_ascii_lowercase())
    };
    valid.then(|| format!("{right}{left}"))
}

/// Retain original items matching every token, in stable score order.
/// An empty whitespace-only query does not evaluate the projection.
pub fn fuzzy_filter<'a, T, S, F>(items: &'a [T], query: &str, get_text: F) -> Vec<&'a T>
where
    F: Fn(&'a T) -> S,
    S: AsRef<str>,
{
    let tokens: Vec<_> = query
        .split(is_whitespace_scalar)
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return items.iter().collect();
    }
    let mut ranked: Vec<_> = items
        .iter()
        .filter_map(|item| {
            let text = get_text(item);
            tokens
                .iter()
                .try_fold(0.0, |score, token| {
                    let found = fuzzy_match(token, text.as_ref());
                    found.matches.then_some(score + found.score)
                })
                .map(|score| (item, score))
        })
        .collect();
    ranked.sort_by(|left, right| left.1.total_cmp(&right.1));
    ranked.into_iter().map(|(item, _)| item).collect()
}
