//! Source-owned setup for selecting one message probe per provider.

use crate::chat::TestResult;
use icu_collator::{Collator, CollatorPreferences, options::CollatorOptions};
use maestro_models::{get_models, get_providers};
use serde::Deserialize;
use serde_json::{Value, json};

/// A distinguishing fixed or catalog-derived selector input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Case identity retained through stable ties.
    name: String,
    /// Provider encounter order.
    provider: String,
    /// Only the descriptor fields the selector reads.
    model: ProbeModel,
}

/// Descriptor projection used only by source test setup.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeModel {
    /// Authored model spelling.
    id: String,
    /// The two read cost categories.
    cost: Cost,
    /// Optional eager compatibility filter.
    compat: Option<Compat>,
}

/// Ordered arithmetic operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cost {
    /// Input price.
    input: f64,
    /// Output price.
    output: f64,
}

/// The exact-false selection predicate.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Compat {
    /// Absent/null and true remain eligible.
    supports_eager_tool_input_streaming: Option<bool>,
}

/// Source priority arithmetic preserves addition before one subtraction.
fn priority(model: &ProbeModel) -> f64 {
    let id = model.id.to_lowercase();
    let cost = model.cost.input + model.cost.output;
    if id.contains("haiku") && (id.contains("4-5") || id.contains("4.5")) {
        cost - 1000.0
    } else if id.contains("sonnet") && (id.contains("4-") || id.contains("4.")) {
        cost - 750.0
    } else if id.contains("claude") && (id.contains("4-") || id.contains("4.")) {
        cost - 500.0
    } else {
        cost
    }
}

/// Stable native collation breaks numeric ties without changing provider encounter order.
fn select(cases: &[Case], forced: bool) -> TestResult<Vec<&Case>> {
    let collator = Collator::try_new(CollatorPreferences::default(), CollatorOptions::default())?;
    let mut groups: indexmap::IndexMap<&str, Vec<&Case>> = indexmap::IndexMap::new();
    for case in cases.iter().filter(|case| {
        !forced
            || case
                .model
                .compat
                .as_ref()
                .and_then(|compat| compat.supports_eager_tool_input_streaming)
                != Some(false)
    }) {
        groups.entry(&case.provider).or_default().push(case);
    }
    let mut chosen = Vec::new();
    for (_, mut group) in groups {
        group.sort_by(|left, right| {
            let difference = priority(&left.model) - priority(&right.model);
            difference
                .partial_cmp(&0.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| collator.compare(&left.model.id, &right.model.id))
        });
        if let Some(first) = group.first() {
            chosen.push(*first);
        }
    }
    Ok(chosen)
}

/// Fixed inputs compare ordered selected names and every computed priority.
pub fn fixed(cases: &[Case]) -> TestResult<Value> {
    let configured: Vec<_> = select(cases, false)?
        .into_iter()
        .map(|case| &case.name)
        .collect();
    let forced: Vec<_> = select(cases, true)?
        .into_iter()
        .map(|case| &case.name)
        .collect();
    Ok(
        json!({"priorities": cases.iter().map(|case| priority(&case.model)).collect::<Vec<_>>(), "configured": configured, "forced": forced, "long": configured}),
    )
}

/// Enumerate the real catalog and compare independently recorded identities and selections.
pub fn catalog() -> TestResult<Value> {
    let cases: Vec<_> = get_providers()
        .iter()
        .flat_map(|provider| {
            get_models(provider)
                .into_iter()
                .filter(|model| model.api == "anthropic-messages")
                .map(|model| {
                    let eager = model
                        .compat
                        .as_ref()
                        .and_then(|compat| compat.0.get("supportsEagerToolInputStreaming"))
                        .and_then(Value::as_bool);
                    Case {
                        name: format!("{}/{}", model.provider, model.id),
                        provider: model.provider,
                        model: ProbeModel {
                            id: model.id,
                            cost: Cost {
                                input: model.cost.input,
                                output: model.cost.output,
                            },
                            compat: Some(Compat {
                                supports_eager_tool_input_streaming: eager,
                            }),
                        },
                    }
                })
        })
        .collect();
    let identities = |cases: Vec<&Case>| {
        cases
            .into_iter()
            .map(|case| json!({"provider": case.provider, "id": case.model.id}))
            .collect::<Vec<_>>()
    };
    let configured = identities(select(&cases, false)?);
    Ok(
        json!({"candidates": identities(cases.iter().collect()), "configured": configured, "forced": identities(select(&cases, true)?), "long": configured}),
    )
}

/// Verify that a configured/forced/long wire invocation uses the selector's actual choice.
pub fn selected(provider: &str, forced: bool) -> TestResult<String> {
    let output = catalog()?;
    let selected = output[if forced { "forced" } else { "configured" }]
        .as_array()
        .ok_or("selection array")?;
    Ok(selected
        .iter()
        .find(|entry| entry["provider"] == provider)
        .and_then(|entry| entry["id"].as_str())
        .ok_or("selected provider")?
        .to_owned())
}
