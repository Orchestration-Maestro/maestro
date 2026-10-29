//! Usage labels retain each child's last context size and aggregate additive counts.
use crate::runs::{SingleResult, UsageStats};

/// Formats counts with the same thresholds as the tool's compact displays.
fn tokens(count: f64) -> String {
    if count < 1000.0 {
        return count.to_string();
    }
    if count < 10_000.0 {
        return format!(
            "{}k",
            ryu_js::Buffer::new().format_to_fixed(count / 1000.0, 1)
        );
    }
    if count < 1_000_000.0 {
        return format!("{:.0}k", (count / 1000.0).round());
    }
    format!(
        "{}M",
        ryu_js::Buffer::new().format_to_fixed(count / 1_000_000.0, 1)
    )
}
/// Formats only nonzero usage and the optional model name.
pub(super) fn label(usage: &UsageStats, model: Option<&str>) -> String {
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
        if count != 0.0 {
            parts.push(format!("{prefix}{}", tokens(count)));
        }
    }
    if usage.cost != 0.0 {
        parts.push(format!(
            "${}",
            ryu_js::Buffer::new().format_to_fixed(usage.cost, 4)
        ));
    }
    if usage.context_tokens > 0.0 {
        parts.push(format!("ctx:{}", tokens(usage.context_tokens)));
    }
    if let Some(model) = model {
        parts.push(model.into());
    }
    parts.join(" ")
}
/// Sums additive fields, deliberately excluding context sizes from different children.
pub(super) fn total(results: &[SingleResult]) -> UsageStats {
    let mut total = UsageStats::default();
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
