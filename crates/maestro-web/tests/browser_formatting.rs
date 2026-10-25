//! Display formatting through the browser library's public interface.
#![cfg(test)]
use maestro_web::format_cost;
use serde::Deserialize;

/// Number spelling retained in the controlled corpus.
#[derive(Deserialize)]
#[serde(untagged)]
enum Number {
    /// Finite number.
    Finite(f64),
    /// Nonfinite or signed-zero spelling.
    Spelling(String),
}
impl Number {
    /// Decode corpus numbers without changing signed zero.
    fn value(&self) -> f64 {
        match self {
            Self::Finite(n) => *n,
            Self::Spelling(s) => s.parse().unwrap(),
        }
    }
}
/// Expected complete display text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumericCase {
    /// Supplied number.
    input: Number,
    /// Captured text.
    expected: String,
}
#[test]
fn format_cost_preserves_four_digit_rounding() {
    let mut cases: Vec<NumericCase> =
        serde_json::from_str(include_str!("fixtures/costs.json")).unwrap();
    cases.sort_by_key(|case| case.input.value().to_bits() != 0.03125_f64.to_bits());
    for case in cases {
        assert_eq!(format_cost(case.input.value()), case.expected);
    }
}

use crate::support;
use maestro_app::{ModelCost, Presence, Usage, UsageCost};
use maestro_web::{I18n, format_model_cost};
use std::rc::Rc;
use support::Host;

/// Optional numeric price fields in the captured dynamic input.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Price {
    /// Input price.
    input: Option<Number>,
    /// Output price.
    output: Option<Number>,
}
impl Price {
    /// Project onto the existing required numeric record.
    fn record(&self) -> ModelCost {
        ModelCost {
            input: self.input.as_ref().map_or(0.0, Number::value),
            output: self.output.as_ref().map_or(0.0, Number::value),
            ..ModelCost::default()
        }
    }
}
/// Captured model price output and lazy host effects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelCase {
    /// Language selected in storage.
    language: String,
    /// Optional complete price record.
    input: Option<Price>,
    /// Complete rendered result.
    expected: String,
    /// Required host effects.
    effects: Vec<String>,
}
#[test]
fn format_model_cost_selects_price_precision_and_localized_free() {
    let cases: Vec<ModelCase> = serde_json::from_str(include_str!("fixtures/models.json")).unwrap();
    for case in cases {
        let host = Rc::new(Host::default());
        *host.stored.borrow_mut() = Some(case.language);
        let context: I18n = I18n::new(host.clone()).unwrap();
        let cost = case.input.as_ref().map(Price::record);
        assert_eq!(
            format_model_cost(cost.as_ref(), &context).unwrap(),
            case.expected
        );
        assert_eq!(*host.effects.borrow(), case.effects);
    }
}
#[test]
fn format_model_cost_reads_language_only_for_free_prices() {
    let host = Rc::new(Host::default());
    *host.fail.borrow_mut() = "get".into();
    let context: I18n = I18n::new(host.clone()).unwrap();
    assert_eq!(
        format_model_cost(
            Some(&ModelCost {
                input: 1.0,
                output: 2.0,
                ..ModelCost::default()
            }),
            &context
        )
        .unwrap(),
        "$1/$2"
    );
    assert!(host.effects.borrow().is_empty());
    assert!(matches!(
        format_model_cost(None, &context),
        Err(maestro_web::I18nError::Host(_))
    ));
    assert_eq!(*host.effects.borrow(), ["get"]);
}
#[test]
fn format_token_count_preserves_thresholds_and_rounding() {
    let cases: Vec<NumericCase> =
        serde_json::from_str(include_str!("fixtures/tokens.json")).unwrap();
    for case in cases {
        assert_eq!(
            maestro_web::format_token_count(case.input.value()),
            case.expected
        );
    }
}

/// Controlled raw usage numbers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawUsage {
    /// Input count.
    input: Number,
    /// Output count.
    output: Number,
    /// Read count.
    cache_read: Number,
    /// Write count.
    cache_write: Number,
    /// Ignored supplied total, retained in the record.
    total_tokens: Option<Number>,
    /// Supplied component/total prices.
    cost: RawCost,
}
/// Raw component costs with optional unread components.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawCost {
    /// Input cost.
    input: Option<Number>,
    /// Output cost.
    output: Option<Number>,
    /// Read cost.
    cache_read: Option<Number>,
    /// Write cost.
    cache_write: Option<Number>,
    /// Authoritative total.
    total: Number,
}
/// Full usage text observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UsageCase {
    /// Optional supplied record.
    input: Option<RawUsage>,
    /// Full text.
    expected: String,
}
/// Keep captured component values without recomputing total.
fn usage_record(raw: RawUsage) -> Usage {
    let cost = raw.cost;
    Usage {
        input: raw.input.value(),
        output: raw.output.value(),
        cache_read: raw.cache_read.value(),
        cache_write: raw.cache_write.value(),
        total_tokens: raw
            .total_tokens
            .as_ref()
            .map_or(Presence::Missing, |number| {
                Presence::Present(number.value())
            }),
        cost: UsageCost {
            input: cost.input.as_ref().map_or(0.0, Number::value),
            output: cost.output.as_ref().map_or(0.0, Number::value),
            cache_read: cost.cache_read.as_ref().map_or(0.0, Number::value),
            cache_write: cost.cache_write.as_ref().map_or(0.0, Number::value),
            total: cost.total.value(),
        },
    }
}
#[test]
fn format_usage_emits_only_truthy_fields_in_fixed_order() {
    let cases: Vec<UsageCase> = serde_json::from_str(include_str!("fixtures/usage.json")).unwrap();
    for case in cases {
        let usage = case.input.map(usage_record);
        assert_eq!(maestro_web::format_usage(usage.as_ref()), case.expected);
    }
}
