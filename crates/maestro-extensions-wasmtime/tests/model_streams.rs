//! Compiled guest model dispatch reaches the original registered native adapter.
#![cfg(test)]
use maestro_models::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// Consuming adapter records original inputs at the actual dispatch boundary.
#[derive(Default)]
struct Recording(Mutex<Vec<Value>>);
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Recording
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let options = call.options.map(|options| match options.into() {
            StreamOptionsInput::Raw(raw) => json!({"common":raw.common,"extra":raw.extra,"simple":false}),
            StreamOptionsInput::Simple(simple) => json!({"common":simple.common,"reasoning":simple.reasoning,"budgets":simple.thinking_budgets,"simple":true}),
        });
        self.0
            .lock()
            .unwrap()
            .push(json!({"model":call.model,"context":call.context,"options":options}));
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::new(RwLock::new(faux_assistant_message(
                "native result 雪",
                FauxAssistantMessageOptions::default(),
            ))),
        });
        Ok(stream)
    }
}

#[test]
fn raw_and_simple_streams_forward_original_options() {
    let recording = Arc::new(Recording::default());
    register_api_provider(
        ApiProvider {
            api: "component-original-options".into(),
            stream: recording.clone(),
            stream_simple: recording.clone(),
        },
        Some("component-original-options".into()),
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
        let output = command("model-streams".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        let output: Value = serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
        assert_eq!(output.as_array().unwrap().len(), 6);
        for row in output.as_array().unwrap() {
            assert_eq!(row["sameOwner"], true);
            assert_eq!(row["eof"], true);
            assert_eq!(row["event"]["type"], "done");
            assert_eq!(
                row["event"]["message"]["content"][0]["text"],
                "native result 雪"
            );
        }
        bus.clear();
    });
    check_observed(&recording.0.lock().unwrap());
    unregister_api_providers("component-original-options");
}

/// Compare the literal operands at the consuming adapter for absent, empty and full options.
fn check_observed(observed: &[Value]) {
    assert_eq!(observed.len(), 6);
    for (index, call) in observed.iter().enumerate() {
        assert_eq!(
            call["model"],
            json!({"id":"unlisted-雪","name":"Unlisted","api":"component-original-options","provider":"custom","baseUrl":"https://example.invalid/literal","reasoning":true,"input":["text"],"cost":{"input":1.5,"output":2.5,"cacheRead":3.5,"cacheWrite":4.5},"contextWindow":12345.5,"maxTokens":88.5,"headers":{"x-model":"literal"}})
        );
        assert_eq!(
            call["context"],
            json!({"systemPrompt":"instruction 雪","messages":[{"role":"user","content":"literal input","timestamp":1.5}],"tools":[]})
        );
        let row = index % 3;
        let simple = index >= 3;
        if row == 0 {
            assert!(call["options"].is_null());
            continue;
        }
        assert_eq!(call["options"]["simple"], simple);
        if row == 1 {
            assert_eq!(
                call["options"],
                if simple {
                    json!({"simple":true,"common":{},"reasoning":null,"budgets":null})
                } else {
                    json!({"simple":false,"common":{},"extra":{}})
                }
            );
            continue;
        }
        check_full_options(call, simple);
    }
}

/// Full common and sibling policies must remain distinct, including signed zero and header order.
fn check_full_options(call: &Value, simple: bool) {
    assert_eq!(
        call["options"]["common"],
        json!({"temperature":-0.0,"maxTokens":17.5,"apiKey":"controlled","transport":"websocket-cached","cacheRetention":"long","sessionId":"session 雪","headers":{"z-header":"first","a-header":"second"},"timeoutMs":if simple {222.5} else {111.5},"maxRetries":if simple {4.5} else {2.5},"maxRetryDelayMs":if simple {888.5} else {444.5},"metadata":{"roots":[null,false,0,"",{},[]],"n":-0.0}})
    );
    assert!(
        call["options"]["common"]["temperature"]
            .as_f64()
            .unwrap()
            .is_sign_negative()
    );
    assert_eq!(
        call["options"]["common"]["headers"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["z-header", "a-header"]
    );
    if simple {
        assert_eq!(call["options"]["reasoning"], "high");
        assert_eq!(
            call["options"]["budgets"],
            json!({"minimal":1.25,"low":2.5,"medium":3.75,"high":4.5})
        );
    } else {
        assert_eq!(
            call["options"]["extra"],
            json!({"reasoning":"raw-policy","thinkingBudgets":{"high":99},"timeout":999,"custom":["z","a","z"]})
        );
    }
}

/// An adapter's setup failure and its later terminal failure are different channels.
struct Failing;
impl<E: ProviderExecution, O> Invoke<E, ProviderCall<E, O>> for Failing {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        if call.model.id == "setup" {
            return Err(DiagnosticErrorInfo::error("adapter setup failed"));
        }
        let stream = AssistantMessageEventStream::new();
        let message = faux_assistant_message(
            "",
            FauxAssistantMessageOptions {
                timestamp: Some(13.5),
                stop_reason: Some(StopReason::Error),
                error_message: Some("provider terminal failed".into()),
                ..Default::default()
            },
        );
        stream.push(AssistantMessageEvent::Error {
            reason: ErrorReason::Error,
            error: Arc::new(RwLock::new(message)),
        });
        Ok(stream)
    }
}

#[test]
fn stream_setup_errors_keep_their_call_boundary() {
    let failing = Arc::new(Failing);
    register_api_provider(
        ApiProvider {
            api: "component-errors".into(),
            stream: failing.clone(),
            stream_simple: failing,
        },
        Some("component-errors".into()),
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
        let output = command("model-errors".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        let output: Value = serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
        assert_eq!(output.as_array().unwrap().len(), 8);
        for row in output.as_array().unwrap() {
            assert_eq!(row["eof"], true);
            let expected = if row["api"] == "absent" {
                Some("No API provider registered for api: absent")
            } else if row["id"] == "setup" {
                Some("adapter setup failed")
            } else {
                None
            };
            if let Some(error) = expected {
                assert_eq!(row["callError"], error);
                assert_eq!(row["completion"], json!({"error":error}));
                assert!(row["terminal"].is_null());
            } else {
                assert!(row["callError"].is_null());
                assert_eq!(row["terminal"]["event"]["type"], "error");
                assert_eq!(row["terminal"]["event"]["reason"], "error");
                assert_eq!(
                    row["terminal"]["message"]["errorMessage"],
                    "provider terminal failed"
                );
                assert_eq!(row["completion"]["message"], row["terminal"]["message"]);
            }
        }
        bus.clear();
    });
    unregister_api_providers("component-errors");
}
