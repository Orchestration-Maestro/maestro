//! Token accounting for provider usage reports.

use super::chunk::RawUsage;
use crate::{Model, Usage, UsageCost, calculate_cost};

/// A reported count where an absent or zero report means zero; negative zero becomes zero.
fn or_zero(count: Option<f64>) -> f64 {
    count.filter(|count| *count != 0.0).unwrap_or(0.0)
}

/// A count raised to zero when it is zero or less; not-a-number stays not-a-number.
fn nonnegative(count: f64) -> f64 {
    if count <= 0.0 { 0.0 } else { count }
}

/// Convert a provider usage report into cache-aware token counts priced at the model's rates.
///
/// Cache writes are removed from reported cache hits, and the remaining prompt input
/// never drops below zero. Counts a provider reports beyond the range of a double stay
/// infinite, and arithmetic on them follows the double rules.
pub(super) fn parse_usage(raw: &RawUsage, model: &Model) -> Usage {
    let prompt = or_zero(raw.prompt_tokens);
    let details = raw.prompt_tokens_details.as_ref();
    let reported_cached = details
        .and_then(|details| details.cached_tokens)
        .or(raw.prompt_cache_hit_tokens)
        .unwrap_or(0.0);
    let cache_write = or_zero(details.and_then(|details| details.cache_write_tokens));
    let cache_read = if cache_write > 0.0 {
        nonnegative(reported_cached - cache_write)
    } else {
        reported_cached
    };
    let input = nonnegative(prompt - cache_read - cache_write);
    let output = or_zero(raw.completion_tokens);
    let mut usage = Usage {
        input,
        output,
        cache_read,
        cache_write,
        total_tokens: input + output + cache_read + cache_write,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    };
    calculate_cost(model, &mut usage);
    usage
}
