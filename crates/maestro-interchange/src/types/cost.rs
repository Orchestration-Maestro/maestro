//! Flat-rate cost estimates for supplied model usage.
use super::{Model, Usage, UsageCost};

/// Update each reported category's flat-rate cost and their left-to-right sum.
/// Token counts are unchanged; the returned borrow is the supplied usage's cost record.
/// The descriptor need not belong to the catalog.
pub fn calculate_cost<'a>(model: &Model, usage: &'a mut Usage) -> &'a mut UsageCost {
    usage.cost.input = (model.cost.input / 1_000_000.0) * usage.input;
    usage.cost.output = (model.cost.output / 1_000_000.0) * usage.output;
    usage.cost.cache_read = (model.cost.cache_read / 1_000_000.0) * usage.cache_read;
    usage.cost.cache_write = (model.cost.cache_write / 1_000_000.0) * usage.cache_write;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
    &mut usage.cost
}
