//! Typed guest numeric options reach native consumers only when finite.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AssistantMessageEventStream, Context, Model, ProviderStreamOptions, SimpleStreamOptions,
    StreamOptions, ThinkingBudgets,
};
use serde_json::json;

/// Exercise every typed numeric common option and thinking budget through the real facade.
pub fn command() -> Result<String, String> {
    let model:Model = serde_json::from_value(json!({"id":"finite-model","name":"finite","api":"component-finite-options","provider":"custom","baseUrl":"https://example.invalid","reasoning":false,"input":[],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1})).map_err(|e|e.to_string())?;
    let mut output = Vec::new();
    for simple in [false, true] {
        for field in 0..if simple { 9 } else { 5 } {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                output.push(rejected(&model, field, value, simple)?);
            }
        }
    }
    let mut finite = 0;
    for simple in [false, true] {
        for field in 0..if simple { 9 } else { 5 } {
            for value in [0.0_f64, -0.0, -1.25, 1.5] {
                accepted(&model, field, value, simple)?;
                finite += 1;
            }
        }
    }
    Ok(json!({"rejected":output,"finite":finite}).to_string())
}
/// Select one common numeric slot independently of its serialized spelling.
fn common(field: usize, value: f64) -> StreamOptions {
    StreamOptions {
        temperature: (field == 0).then_some(value),
        max_tokens: (field == 1).then_some(value),
        timeout_ms: (field == 2).then_some(value),
        max_retries: (field == 3).then_some(value),
        max_retry_delay_ms: (field == 4).then_some(value),
        ..Default::default()
    }
}

/// Supply one numeric field through its original raw or simple facade.
fn invoke(
    model: &Model,
    field: usize,
    value: f64,
    simple: bool,
) -> Result<AssistantMessageEventStream, String> {
    let context = Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: None,
    };
    let common = common(field, value);
    if simple {
        maestro_extensions_wasm::stream_simple(
            model.clone(),
            context,
            Some(SimpleStreamOptions {
                common,
                thinking_budgets: Some(ThinkingBudgets {
                    minimal: (field == 5).then_some(value),
                    low: (field == 6).then_some(value),
                    medium: (field == 7).then_some(value),
                    high: (field == 8).then_some(value),
                }),
                ..Default::default()
            }),
        )
    } else {
        maestro_extensions_wasm::stream(
            model.clone(),
            context,
            Some(ProviderStreamOptions {
                common,
                extra: [("uninterpreted".into(), json!(false))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            }),
        )
    }
}

/// Nonfinite fields must fail at the guest encoder, before any native setup operation.
fn rejected(model: &Model, field: usize, value: f64, simple: bool) -> Result<String, String> {
    invoke(model, field, value, simple)
        .err()
        .ok_or_else(|| "nonfinite model options reached native dispatch".into())
}

/// Finite values must reach the registered native consumer with no guest-side substitution.
fn accepted(model: &Model, field: usize, value: f64, simple: bool) -> Result<(), String> {
    if invoke(model, field, value, simple).err().as_deref() != Some("native finite accepted") {
        return Err("finite guest option did not reach its native consumer".into());
    }
    Ok(())
}
