//! Numeric display text for browser consumers.
use super::localization::{I18n, I18nError};
use maestro_app::{ModelCost, browser::strip_decimal};

/// Render a currency amount using four-place fixed number formatting.
#[must_use]
pub fn format_cost(cost: f64) -> String {
    format!("${}", ryu_js::Buffer::new().format_to_fixed(cost, 4))
}

/// Render input/output prices, resolving localized text only for a free price.
/// # Errors
/// Propagates localization failures when the price is free.
pub fn format_model_cost<F: ?Sized>(
    cost: Option<&ModelCost>,
    i18n: &I18n<F>,
) -> Result<String, I18nError> {
    let input = cost.map_or(0.0, |c| c.input);
    let output = cost.map_or(0.0, |c| c.output);
    let input = if input.is_nan() { 0.0 } else { input };
    let output = if output.is_nan() { 0.0 } else { output };
    if input == 0.0 && output == 0.0 {
        return i18n.text("Free");
    }
    Ok(format!("${}/${}", model_price(input), model_price(output)))
}
/// Select a price's precision independently of its sibling.
fn model_price(number: f64) -> String {
    let precision = if number >= 100.0 {
        0
    } else if number >= 10.0 {
        1
    } else if number >= 1.0 {
        2
    } else {
        3
    };
    let mut buffer = ryu_js::Buffer::new();
    let text = buffer.format_to_fixed(number, precision);
    strip_decimal(text).to_owned()
}

/// Render a count with the thousand suffix and tier-specific rounding.
#[must_use]
pub fn format_token_count(count: f64) -> String {
    let mut buffer = ryu_js::Buffer::new();
    if count < 1000.0 {
        return buffer.format(count).to_owned();
    }
    if count < 10000.0 {
        return format!("{}k", buffer.format_to_fixed(count / 1000.0, 1));
    }
    format!("{}k", buffer.format((count / 1000.0).round()))
}

/// Render nonzero, non-NaN usage fields in input/output/read/write/cost order.
#[must_use]
pub fn format_usage(usage: Option<&maestro_app::Usage>) -> String {
    let Some(usage) = usage else {
        return String::new();
    };
    let mut parts: Vec<String> = [
        ("↑", usage.input),
        ("↓", usage.output),
        ("R", usage.cache_read),
        ("W", usage.cache_write),
    ]
    .into_iter()
    .filter(|(_, n)| *n != 0.0 && !n.is_nan())
    .map(|(marker, n)| format!("{marker}{}", format_token_count(n)))
    .collect();
    if usage.cost.total != 0.0 && !usage.cost.total.is_nan() {
        parts.push(format_cost(usage.cost.total));
    }
    parts.join(" ")
}
