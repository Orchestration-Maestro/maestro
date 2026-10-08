//! Token accounting for provider usage reports.

use super::chunk::RawUsage;
use crate::{Model, Usage, UsageCost, calculate_cost};

/// Convert a provider usage report into cache-aware token counts priced at the model's rates.
///
/// Cache writes are removed from reported cache hits, and the remaining prompt input
/// never drops below zero.
pub(super) fn parse_usage(raw: &RawUsage, model: &Model) -> Usage {
    let prompt = raw.prompt_tokens.unwrap_or(0.0);
    let details = raw.prompt_tokens_details.as_ref();
    let reported_cached = details
        .and_then(|details| details.cached_tokens)
        .or(raw.prompt_cache_hit_tokens)
        .unwrap_or(0.0);
    let cache_write = details
        .and_then(|details| details.cache_write_tokens)
        .unwrap_or(0.0);
    let cache_read = if cache_write > 0.0 {
        (reported_cached - cache_write).max(0.0)
    } else {
        reported_cached
    };
    let input = (prompt - cache_read - cache_write).max(0.0);
    let output = raw.completion_tokens.unwrap_or(0.0);
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
