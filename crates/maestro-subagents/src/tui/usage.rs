//! Usage labels retain each child's last context size and aggregate additive counts.
use crate::runs::{SingleResult, Usage};

/// Formats counts with the same thresholds as the tool's compact displays.
fn tokens(count: u64) -> String {
    if count < 1000 {
        return count.to_string();
    }
    if count < 10_000 {
        return decimal(rounded(count, 100), "k");
    }
    if count < 1_000_000 {
        return format!("{}k", rounded(count, 1000));
    }
    decimal(rounded(count, 100_000), "M")
}
/// Rounds a nonnegative count without overflowing its integer representation.
fn rounded(count: u64, unit: u64) -> u64 {
    count / unit + u64::from(count % unit >= unit / 2)
}
/// Writes the retained decimal digit of a scaled count.
fn decimal(tenths: u64, suffix: &str) -> String {
    format!("{}.{}{suffix}", tenths / 10, tenths % 10)
}
/// Formats only nonzero usage and the optional model name.
pub(super) fn label(usage: &Usage, model: Option<&str>) -> String {
    let mut parts = Vec::new();
    if usage.turns > 0 {
        parts.push(format!(
            "{} turn{}",
            usage.turns,
            if usage.turns > 1 { "s" } else { "" }
        ));
    }
    for (prefix, count) in [
        ("↑", usage.input),
        ("↓", usage.output),
        ("R", usage.cache_read),
        ("W", usage.cache_write),
    ] {
        if count > 0 {
            parts.push(format!("{prefix}{}", tokens(count)));
        }
    }
    if usage.cost != 0.0 {
        parts.push(format!("${:.4}", usage.cost));
    }
    if usage.context_tokens > 0 {
        parts.push(format!("ctx:{}", tokens(usage.context_tokens)));
    }
    if let Some(model) = model {
        parts.push(model.into());
    }
    parts.join(" ")
}
/// Sums additive fields, deliberately excluding context sizes from different children.
pub(super) fn total(results: &[SingleResult]) -> Usage {
    let mut total = Usage::default();
    for result in results {
        total.input += result.usage.input;
        total.output += result.usage.output;
        total.cache_read += result.usage.cache_read;
        total.cache_write += result.usage.cache_write;
        total.cost += result.usage.cost;
        total.turns += result.usage.turns;
    }
    total
}
