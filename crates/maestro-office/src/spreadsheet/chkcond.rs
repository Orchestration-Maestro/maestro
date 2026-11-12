//! Numeric condition recognition and comparison.

/// Six spreadsheet threshold comparisons.
#[derive(Clone, Copy)]
enum Comparison {
    /// Equal to the threshold.
    Equal,
    /// Above the threshold.
    Greater,
    /// Below the threshold.
    Less,
    /// Different from the threshold.
    NotEqual,
    /// At or above the threshold.
    GreaterEqual,
    /// At or below the threshold.
    LessEqual,
}

/// Parsed comparison and its numeric threshold.
pub(super) struct Condition {
    /// Comparison operator.
    comparison: Comparison,
    /// Parsed numeric threshold.
    threshold: f64,
}

/// Find the first bracket with the ASCII threshold grammar.
pub(super) fn parse_condition(format: &str) -> Option<Condition> {
    format.match_indices('[').find_map(|(start, _)| {
        let body = format.get(start + 1..)?.split_once(']')?.0;
        let (operator, digits) = body.split_at(body.find(|c: char| !matches!(c, '=' | '>' | '<'))?);
        let comparison = match operator {
            "=" => Comparison::Equal,
            ">" => Comparison::Greater,
            "<" => Comparison::Less,
            "<>" => Comparison::NotEqual,
            ">=" => Comparison::GreaterEqual,
            "<=" => Comparison::LessEqual,
            _ => return None,
        };
        let unsigned = digits.strip_prefix('-').unwrap_or(digits);
        let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if integer.is_empty()
            || !integer
                .bytes()
                .chain(fraction.bytes())
                .all(|c| c.is_ascii_digit())
        {
            return None;
        }
        Some(Condition {
            comparison,
            threshold: digits.parse().ok()?,
        })
    })
}

/// Compare a value without coercion; an absent condition is false.
pub(super) fn chkcond(value: f64, condition: Option<Condition>) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    condition.is_some_and(|c| match c.comparison {
        Comparison::Equal => value.partial_cmp(&c.threshold) == Some(Equal),
        Comparison::Greater => value.partial_cmp(&c.threshold) == Some(Greater),
        Comparison::Less => value.partial_cmp(&c.threshold) == Some(Less),
        Comparison::NotEqual => value.partial_cmp(&c.threshold) != Some(Equal),
        Comparison::GreaterEqual => {
            matches!(value.partial_cmp(&c.threshold), Some(Greater | Equal))
        }
        Comparison::LessEqual => matches!(value.partial_cmp(&c.threshold), Some(Less | Equal)),
    })
}
