//! Flat per-attempt measurements and catalog-rate estimates.

/// Supplied catalog prices in USD per million tokens; valid rates are finite and nonnegative.
/// Numerical zero defaults do not establish that prices were supplied or access is free.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TokenRates {
    /// Rate for input excluding both cache categories.
    pub input: f64,
    /// Rate for output, including reported reasoning tokens.
    pub output: f64,
    /// Rate for cache reads excluding current cache writes.
    pub cache_read: f64,
    /// Rate for cache writes.
    pub cache_write: f64,
}

/// Flat category estimates in USD, without currency rounding; not a bill or balance.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UsageCost {
    /// Input estimate in USD.
    pub input: f64,
    /// Output estimate in USD.
    pub output: f64,
    /// Cache-read estimate in USD.
    pub cache_read: f64,
    /// Cache-write estimate in USD.
    pub cache_write: f64,
    /// Sum of the four category estimates in USD.
    pub total: f64,
    /// Whether supplied catalog rates were used, including explicit zero rates.
    /// This is independent of reporting and never proves billing or free access.
    pub priced: bool,
}

/// One attempt's non-overlapping reported counters and derived flat estimate.
/// Default means unreported, not a measured zero; explicit reports replace prior counters.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Usage {
    /// Reported input tokens excluding cache reads and writes.
    pub input: u64,
    /// Reported output tokens including reasoning, counted once.
    pub output: u64,
    /// Reported cache-read tokens excluding current cache writes.
    pub cache_read: u64,
    /// Reported cache-write tokens.
    pub cache_write: u64,
    /// Derived sum of the four non-overlapping categories.
    pub total_tokens: u64,
    /// Whether an explicit report was accepted, even if all counters are zero.
    pub reported: bool,
    /// Derived catalog-rate estimate, independent of actual response identity.
    pub cost: UsageCost,
}

pub(crate) fn normalize(
    mut usage: Usage,
    rates: Option<&TokenRates>,
) -> Result<Usage, crate::Failure> {
    usage.total_tokens = usage
        .input
        .checked_add(usage.output)
        .and_then(|total| total.checked_add(usage.cache_read))
        .and_then(|total| total.checked_add(usage.cache_write))
        .ok_or(crate::Failure::MalformedStream)?;
    usage.reported = true;
    let fallback = TokenRates::default();
    let rate = rates.unwrap_or(&fallback);
    if [rate.input, rate.output, rate.cache_read, rate.cache_write]
        .iter()
        .any(|rate| !rate.is_finite() || *rate < 0.0)
    {
        return Err(crate::Failure::MalformedStream);
    }
    let input = (rate.input / 1_000_000.0) * usage.input as f64;
    let output = (rate.output / 1_000_000.0) * usage.output as f64;
    let cache_read = (rate.cache_read / 1_000_000.0) * usage.cache_read as f64;
    let cache_write = (rate.cache_write / 1_000_000.0) * usage.cache_write as f64;
    usage.cost = UsageCost {
        input,
        output,
        cache_read,
        cache_write,
        total: input + output + cache_read + cache_write,
        priced: rates.is_some(),
    };
    if [input, output, cache_read, cache_write, usage.cost.total]
        .iter()
        .any(|cost| !cost.is_finite())
    {
        return Err(crate::Failure::MalformedStream);
    }
    Ok(usage)
}
