//! Guest aliases preserve finite data at native consumers and retain opaque local owners.
#![cfg(test)]
use maestro_models::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Only data actually consumed by the native registered adapter is recorded.
#[derive(Default)]
struct Consumed(Mutex<Vec<Value>>);
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Consumed
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        assert_eq!(call.model.id, "finite-model");
        assert!(call.context.messages.is_empty());
        let row = match call.options.unwrap().into() {
            StreamOptionsInput::Raw(options) => json!({
                "simple":false, "common":options.common, "extra":options.extra,
            }),
            StreamOptionsInput::Simple(options) => json!({
                "simple":true, "common":options.common, "budgets":options.thinking_budgets,
            }),
        };
        self.0.lock().unwrap().push(row);
        Err(DiagnosticErrorInfo::error("native finite accepted"))
    }
}
#[test]
fn guest_stream_options_keep_finite_data_and_opaque_owners() {
    let consumed = Arc::new(Consumed::default());
    register_api_provider(
        ApiProvider {
            api: "component-finite-options".into(),
            stream: consumed.clone(),
            stream_simple: consumed.clone(),
        },
        Some("component-finite-options".into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            std::rc::Rc::new(|_| panic!("no tails")),
            std::rc::Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let output = command("model-finite".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        let output: Value = serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
        let errors = output["rejected"].as_array().unwrap();
        assert_eq!(errors.len(), 42);
        assert!(
            errors
                .iter()
                .all(|error| error == "extension wrote a non-finite number (Infinity or NaN)")
        );
        assert_eq!(output["finite"], 56);
        check_finite(&consumed.0.lock().unwrap());
        bus.clear();
        super::model_lifetimes::retained(false, Some("first:opaque-drop")).await;
    });
    unregister_api_providers("component-finite-options");
}

/// Every finite common and budget field keeps its bits and leaves all negative neighbors absent.
fn check_finite(rows: &[Value]) {
    assert_eq!(rows.len(), 56);
    let fields = [
        "temperature",
        "maxTokens",
        "timeoutMs",
        "maxRetries",
        "maxRetryDelayMs",
        "minimal",
        "low",
        "medium",
        "high",
    ];
    let mut rows = rows.iter();
    for simple in [false, true] {
        for (index, field) in fields[..if simple { 9 } else { 5 }].iter().enumerate() {
            for value in [0.0_f64, -0.0, -1.25, 1.5] {
                let row = rows.next().unwrap();
                check_field(row, (simple, index), field, value);
            }
        }
    }
    assert!(rows.next().is_none());
}

/// Compare one supplied field without losing its independently known IEEE representation.
fn check_field(row: &Value, case: (bool, usize), field: &str, value: f64) {
    let (simple, index) = case;
    assert_eq!(row["simple"], simple);
    let selected = if index < 5 {
        &row["common"]
    } else {
        &row["budgets"]
    };
    assert_eq!(selected[field].as_f64().unwrap().to_bits(), value.to_bits());
    assert_eq!(selected.as_object().unwrap().len(), 1);
    check_neighbors(row, index);
}

/// Raw extras and empty common/budget records remain separate at the native consumer.
fn check_neighbors(row: &Value, index: usize) {
    if row["simple"] == true {
        if index < 5 {
            assert_eq!(row["budgets"], json!({}));
        } else {
            assert_eq!(row["common"], json!({}));
        }
    } else {
        assert_eq!(row["extra"], json!({"uninterpreted":false}));
    }
}
