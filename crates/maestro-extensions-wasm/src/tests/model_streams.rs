//! Guest invocation options share records while retaining local capability owners.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    ExtensionResult, OnPayload, OnResponse, PayloadHookInput, ProviderObjects, ProviderResponse,
    ProviderStreamOptions, SimpleStreamOptions, StreamOptions, ThinkingBudgets,
};
use std::{cell::Cell, rc::Rc};

#[test]
fn guest_stream_options_keep_finite_data_and_opaque_owners() {
    local_owners();
    common_numbers();
    budget_numbers();
}

/// Nonserializable objects and local callbacks keep their original owners.
fn local_owners() {
    let host = super::controlled::Controlled::default();
    let flag = super::controlled::Flag::default();
    flag.abort();
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    let payload: OnPayload = Rc::new(move |input, _| {
        seen.set(seen.get() + 1);
        Box::pin(async move { Ok(input) })
    });
    let seen = calls.clone();
    let response: OnResponse = Rc::new(move |_, _| {
        seen.set(seen.get() + 1);
        Box::pin(async { Ok(()) })
    });
    let opaque: ProviderObjects = Some(Rc::new(Cell::new(42_u32)));
    let mut raw = ProviderStreamOptions {
        common: StreamOptions {
            signal: Some(crate::component_adapter::signal(&host, host.signal(&flag))),
            on_payload: Some(payload),
            on_response: Some(response),
            ..Default::default()
        },
        objects: opaque.clone(),
        ..Default::default()
    };
    raw.extra
        .insert("uninterpreted".into(), serde_json::json!(false));
    assert!(raw.common.signal.as_ref().unwrap().aborted());
    assert!(Rc::ptr_eq(
        raw.objects.as_ref().unwrap(),
        opaque.as_ref().unwrap()
    ));
    assert_eq!(
        raw.objects
            .as_ref()
            .unwrap()
            .downcast_ref::<Cell<u32>>()
            .unwrap()
            .get(),
        42
    );
    let copied = raw.clone();
    assert_eq!(copied.extra["uninterpreted"], false);
    invoke_hooks(&raw.common);
    assert_eq!(calls.get(), 2);
    let simple = SimpleStreamOptions {
        common: raw.common,
        thinking_budgets: Some(ThinkingBudgets::default()),
        ..Default::default()
    };
    assert!(simple.common.on_payload.is_some() && simple.common.on_response.is_some());
    assert!(simple.tool_choice.is_none());
    assert_eq!(
        crate::types::finite::encode_value(&simple.common).unwrap(),
        "{}"
    );
}

/// Invokes both retained local hooks without translating their selected payload or signal.
fn invoke_hooks(common: &StreamOptions) {
    let model: std::sync::Arc<crate::Model> = std::sync::Arc::new(serde_json::from_str(r#"{"id":"model","name":"model","api":"absent","provider":"fixture","baseUrl":"unused","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1}"#).unwrap());
    let selected = super::block_on(common.on_payload.as_ref().unwrap()(
        PayloadHookInput {
            payload: serde_json::Value::Null,
            signal: common.signal.clone(),
        },
        model.clone(),
    ))
    .unwrap();
    assert!(selected.payload.is_null() && selected.signal.unwrap().aborted());
    super::block_on(common.on_response.as_ref().unwrap()(
        ProviderResponse::default(),
        model,
    ))
    .unwrap();
}

/// Checks every numeric common field through the finite-number encoding boundary.
fn common_numbers() {
    for index in 0..5 {
        for value in [
            0.0_f64,
            -0.0,
            -1.25,
            1.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let mut fields = [None; 5];
            fields[index] = Some(value);
            let options = StreamOptions {
                temperature: fields[0],
                max_tokens: fields[1],
                timeout_ms: fields[2],
                max_retries: fields[3],
                max_retry_delay_ms: fields[4],
                ..Default::default()
            };
            finite_field(
                crate::types::finite::encode_value(&options),
                [
                    "temperature",
                    "maxTokens",
                    "timeoutMs",
                    "maxRetries",
                    "maxRetryDelayMs",
                ][index],
                value,
            );
        }
    }
}

/// Checks every numeric budget field at the same boundary as common options.
fn budget_numbers() {
    for index in 0..4 {
        for value in [
            0.0_f64,
            -0.0,
            -1.25,
            1.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let mut fields = [None; 4];
            fields[index] = Some(value);
            let budgets = ThinkingBudgets {
                minimal: fields[0],
                low: fields[1],
                medium: fields[2],
                high: fields[3],
            };
            finite_field(
                crate::types::finite::encode_value(&budgets),
                ["minimal", "low", "medium", "high"][index],
                value,
            );
        }
    }
}

/// Finite fields keep their bits; nonfinite fields fail rather than becoming null.
fn finite_field(encoded: ExtensionResult<String>, key: &str, value: f64) {
    if value.is_finite() {
        let decoded: serde_json::Value = serde_json::from_str(&encoded.unwrap()).unwrap();
        assert_eq!(decoded[key].as_f64().unwrap().to_bits(), value.to_bits());
        assert_eq!(decoded.as_object().unwrap().len(), 1);
    } else {
        assert_eq!(
            encoded.unwrap_err(),
            "extension wrote a non-finite number (Infinity or NaN)"
        );
    }
}

#[test]
fn requested_thinking_levels_keep_agent_selection_unchanged() {
    for (level, wire) in [
        (crate::RequestedThinkingLevel::Minimal, "minimal"),
        (crate::RequestedThinkingLevel::Low, "low"),
        (crate::RequestedThinkingLevel::Medium, "medium"),
        (crate::RequestedThinkingLevel::High, "high"),
        (crate::RequestedThinkingLevel::Xhigh, "xhigh"),
    ] {
        let options = SimpleStreamOptions {
            reasoning: Some(level),
            ..Default::default()
        };
        assert_eq!(serde_json::to_value(options.reasoning).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<crate::RequestedThinkingLevel>(&format!("\"{wire}\"")).unwrap(),
            level
        );
    }
    assert!(serde_json::from_str::<crate::RequestedThinkingLevel>("\"off\"").is_err());
    let selection: crate::ThinkingLevel = crate::ModelThinkingLevel::Off;
    assert_eq!(serde_json::to_value(selection).unwrap(), "off");
}
