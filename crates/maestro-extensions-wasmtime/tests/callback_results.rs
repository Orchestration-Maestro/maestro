//! Values returned by component callbacks reach their native consumers unchanged.
#![cfg(test)]
use maestro_extensions::{
    CompactOptions, CompactionResult, ExtensionContextActions, ExtensionExecution, Local,
    types::ProviderActions, wrap_registered_tool,
};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

use super::callback_ordering::{Instance, Pair, local};

/// Recorded preparation and validation outcomes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cases {
    /// Values selected by preparation.
    preparation: Vec<Case<Prepared>>,
    /// Phases reached after preparation and validation.
    validation: Vec<Case<Validated>>,
}

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case<T> {
    /// Supplied arguments and preparation mode.
    input: Input,
    /// Recorded outcome.
    expected: T,
}

/// Supplied arguments and the authored preparation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// `absent`, or how the authored preparation answers.
    mode: String,
    /// Raw arguments of any JSON root.
    #[serde(default)]
    args: Value,
    /// Argument text whose parsing is part of the case.
    literal: Option<String>,
}

/// The value preparation selects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    /// Selected arguments, `null` included.
    #[serde(default, deserialize_with = "present")]
    arguments: Option<Value>,
    /// Preparation invocations.
    calls: Option<usize>,
    /// Authored preparation failure.
    error: Option<String>,
    /// Whether the selected value is negative zero.
    negative_zero: Option<bool>,
    /// Phase reached after preparation, when the case records one.
    phase: Option<String>,
    /// Arguments the executor received, when the case records them.
    #[serde(default, deserialize_with = "present")]
    received: Option<Value>,
}

/// The phase reached by a prepared call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Validated {
    /// `preparation`, `validation` or `execution`.
    phase: String,
    /// Arguments the executor received, `null` included.
    #[serde(default, deserialize_with = "present")]
    received: Option<Value>,
    /// Failure of the phase that stopped the call.
    error: Option<String>,
    /// Whether the wrapped executor ran.
    executed: bool,
}

/// Keeps a supplied `null` distinct from an absent member.
fn present<'de, D: Deserializer<'de>>(value: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(value).map(Some)
}

/// What one prepared call produced at each phase.
struct Outcome {
    /// Preparation invocations.
    calls: usize,
    /// Selected value or authored failure.
    selected: Result<Value, String>,
    /// Phase that stopped or finished the call.
    phase: &'static str,
    /// Validation failure.
    invalid: Option<String>,
    /// Arguments the executor received.
    executed: Option<Value>,
}

/// Prepares, validates and executes `echo` as the agent loop orders those phases.
async fn call(pair: &Pair, instance: &Instance, mut raw: Value) -> Outcome {
    let tools = instance.runner.get_all_registered_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition.metadata().definition.name == "echo")
        .unwrap();
    let wrapped = wrap_registered_tool(tool, instance.runner.clone());
    pair.take();
    let selected = match &wrapped.prepare_arguments {
        Some(prepare) => prepare(&mut raw).map_err(|error| error.message),
        None => Ok(raw),
    };
    let calls = pair
        .take()
        .iter()
        .filter(|entry| entry.ends_with(":prepare"))
        .count();
    let mut outcome = Outcome {
        calls,
        selected: selected.clone(),
        phase: "preparation",
        invalid: None,
        executed: None,
    };
    let Ok(arguments) = selected else {
        return outcome;
    };
    let candidate = maestro_models::ToolCall {
        id: "call".into(),
        name: "echo".into(),
        arguments,
        thought_signature: None,
    };
    match maestro_models::validate_tool_arguments(&wrapped.definition, &candidate) {
        Err(error) => (outcome.phase, outcome.invalid) = ("validation", Some(error.message)),
        Ok(valid) => {
            let result = (wrapped.execute)("call".into(), valid, None, None)
                .await
                .unwrap();
            outcome.phase = "execution";
            outcome.executed = pair.payloads.borrow_mut().pop();
            assert!(result.content.is_empty());
            assert_eq!(result.details, outcome.executed);
        }
    }
    outcome
}

/// Loads `a` with an authored preparation and `b` without one.
async fn echo_pair() -> Pair {
    let pair = Pair::new().await;
    pair.a.program(r#"[{"op":"echo","prepare":true}]"#).await;
    pair.b.program(r#"[{"op":"echo","prepare":false}]"#).await;
    pair
}

/// Runs one recorded input against the instance its mode selects.
async fn recorded(pair: &Pair, input: &Input) -> Outcome {
    let args = match &input.literal {
        Some(literal) => serde_json::from_str(literal).unwrap(),
        None => input.args.clone(),
    };
    if input.mode == "absent" {
        return call(pair, &pair.b, args).await;
    }
    let mode = if input.mode == "signed_zero" {
        "same"
    } else {
        &input.mode
    };
    pair.a
        .program(&json!([{"op":"prepare-mode", "mode":mode}]).to_string())
        .await;
    call(pair, &pair.a, args).await
}

/// Checks the value and invocation count one preparation case selected.
fn assert_prepared(index: usize, outcome: &Outcome, expected: &Prepared) {
    match (&outcome.selected, &expected.error) {
        (Err(error), Some(authored)) => assert_eq!(error, authored, "case {index}"),
        (Ok(selected), None) => {
            if let Some(arguments) = &expected.arguments {
                assert_eq!(selected.to_string(), arguments.to_string(), "case {index}");
            }
            if let Some(negative) = expected.negative_zero {
                let zero = selected.as_f64().unwrap();
                assert_eq!((zero, zero.is_sign_negative()), (0.0, negative));
            }
            if let Some(phase) = expected.phase.as_deref() {
                assert_eq!(outcome.phase, phase, "case {index}");
            }
            if let Some(received) = &expected.received {
                let executed = outcome.executed.as_ref().unwrap().to_string();
                assert_eq!(executed, received.to_string(), "case {index}");
            }
        }
        _ => panic!("case {index} selected the other result"),
    }
    if let Some(calls) = expected.calls {
        assert_eq!(outcome.calls, calls, "case {index}");
    }
}

#[test]
fn prepared_arguments_reach_validation_before_execution() {
    local(async {
        let pair = echo_pair().await;
        let cases: Cases =
            serde_json::from_str(include_str!("fixtures/prepared_arguments.json")).unwrap();
        assert_eq!((cases.preparation.len(), cases.validation.len()), (32, 28));
        for (index, case) in cases.preparation.iter().enumerate() {
            let outcome = recorded(&pair, &case.input).await;
            assert_prepared(index, &outcome, &case.expected);
        }
        for (index, case) in cases.validation.iter().enumerate() {
            let outcome = recorded(&pair, &case.input).await;
            let expected = &case.expected;
            assert_eq!(outcome.phase, expected.phase, "case {index}");
            assert_eq!(
                outcome.executed.is_some(),
                expected.executed,
                "case {index}"
            );
            let error = match outcome.phase {
                "preparation" => outcome.selected.err(),
                _ => outcome.invalid,
            };
            assert_eq!(error, expected.error, "case {index}");
            if let Some(received) = &expected.received {
                let executed = outcome.executed.unwrap().to_string();
                assert_eq!(executed, received.to_string(), "case {index}");
            }
        }
    });
}

#[test]
fn prepare_callbacks_drain_before_execution() {
    local(async {
        let pair = echo_pair().await;
        let foreign =
            r#"[{"op":"on","channel":"prepare-foreign","id":"foreign","name":"foreign"}]"#;
        let same = r#"[{"op":"on","channel":"prepare-foreign","id":"same","name":"same"},{"op":"prepare-mode","mode":"drain"}]"#;
        pair.b.program(foreign).await;
        pair.a.program(same).await;
        let tools = pair.a.runner.get_all_registered_tools();
        let wrapped = wrap_registered_tool(&tools[0], pair.a.runner.clone());
        pair.take();
        let prepared = wrapped.prepare_arguments.as_ref().unwrap()(&mut json!({"n":1})).unwrap();
        pair.trace.borrow_mut().push("caller:return".into());
        assert_eq!(prepared, json!({"prepared":{"n":1}}));
        assert_eq!(
            pair.take(),
            [
                "a:prepare",
                "same:prefix",
                "a:prepare:end",
                "foreign:prefix",
                "caller:return",
            ]
        );
        let candidate = maestro_models::ToolCall {
            id: "call".into(),
            name: "echo".into(),
            arguments: prepared,
            thought_signature: None,
        };
        let valid =
            maestro_models::validate_tool_arguments(&wrapped.definition, &candidate).unwrap();
        let result = (wrapped.execute)("call".into(), valid, None, None)
            .await
            .unwrap();
        assert_eq!(result.details, Some(json!({"prepared":{"n":1}})));
        assert_eq!(
            pair.payloads.borrow().as_slice(),
            [json!({"prepared":{"n":1}})]
        );
        assert!(pair.take().is_empty());
    });
}

/// Requests retained by the supplied compaction action.
type Requests = Rc<RefCell<Vec<Option<CompactOptions>>>>;

/// Binds a compaction action that records each request and returns before it finishes.
fn compaction(pair: &Pair) -> Requests {
    let (requests, trace) = (Requests::default(), pair.trace.clone());
    let retained = requests.clone();
    pair.a
        .runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(|| Ok("renderer barrier".into())),
                compact: Rc::new(move |options| {
                    trace.borrow_mut().push("operation".into());
                    retained.borrow_mut().push(options);
                    Ok(())
                }),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    requests
}

#[test]
fn compaction_callbacks_preserve_results_errors_and_return_order() {
    local(async {
        let pair = Pair::new().await;
        let requests = compaction(&pair);
        pair.b
            .program(r#"[{"op":"on","channel":"compaction-foreign","id":"foreign","name":"compaction-foreign"}]"#)
            .await;
        for options in ["absent", "empty", "full"] {
            pair.a
                .program(&json!([{"op":"compact", "options":options}]).to_string())
                .await;
            assert_eq!(pair.take(), ["operation", "request:return"], "{options}");
        }
        let mut requests = std::mem::take(&mut *requests.borrow_mut()).into_iter();
        assert!(requests.next().unwrap().is_none());
        let empty = requests.next().unwrap().unwrap();
        assert!(empty.custom_instructions.is_none());
        assert!(empty.on_complete.is_none() && empty.on_error.is_none());
        let full = requests.next().unwrap().unwrap();
        assert_eq!(full.custom_instructions.as_deref(), Some("keep"));
        let complete = full.on_complete.unwrap();
        let failed = full.on_error.unwrap();
        let result: CompactionResult = serde_json::from_value(
            json!({"summary":"kept","firstKeptEntryId":"entry-2","tokensBefore":17}),
        )
        .unwrap();
        let notified = [
            "a:complete",
            "a:complete:end",
            "compaction-foreign:prefix",
            "done",
        ];
        for _ in 0..2 {
            complete(result.clone()).unwrap();
            pair.trace.borrow_mut().push("done".into());
            assert_eq!(pair.take(), notified);
            let forwarded = pair.payloads.borrow_mut().pop().unwrap();
            assert_eq!(forwarded, serde_json::to_value(&result).unwrap());
        }
        failed("compaction failed \u{3a9}\nline".into()).unwrap();
        pair.trace.borrow_mut().push("done".into());
        assert_eq!(
            pair.take(),
            [
                "a:error:compaction failed \u{3a9}\nline",
                "a:error:end",
                "compaction-foreign:prefix",
                "done",
            ]
        );
    });
}

#[test]
fn callback_failures_keep_authored_errors_and_healthy_delivery() {
    local(async {
        let errors = Rc::new(RefCell::new(Vec::<String>::new()));
        let reported = errors.clone();
        let mut pair = Pair::reporting(Rc::new(move |error| {
            reported.borrow_mut().push(error.to_owned());
        }))
        .await;
        let requests = compaction(&pair);
        let foreign = json!([
            {"op":"on", "channel":"probe-foreign", "id":"probe", "name":"probe"},
            {"op":"on", "channel":"prepare-foreign", "id":"prepare", "name":"prepare"},
            {"op":"on", "channel":"compaction-foreign", "id":"compaction", "name":"compaction"},
        ]);
        pair.b.program(&foreign.to_string()).await;
        pair.a.program(r#"[{"op":"echo","prepare":true}]"#).await;
        for (index, error) in ["", "multi\nline \u{3a9}"].into_iter().enumerate() {
            renderer_and_component_errors(&pair, error);
            preparation_and_compaction_errors(&pair, &requests, error).await;
            let channel = format!("authored-{index}");
            let steps = json!([
                {"op":"on", "channel":channel, "id":channel, "name":"bad", "fail":error,
                    "edits":[{"op":"emit", "channel":"authored-foreign", "data":null}]},
            ]);
            pair.b.program(&steps.to_string()).await;
            let healthy =
                json!([{"op":"on", "channel":"authored-foreign", "id":channel, "name":"healthy"}]);
            pair.a.program(&healthy.to_string()).await;
            pair.bus.emit(&channel, Value::Null).await;
            assert_eq!(pair.take(), ["bad:prefix", "healthy:prefix"]);
            let diagnostic = format!("Event handler error ({channel}): {error}\n");
            assert_eq!(std::mem::take(&mut *errors.borrow_mut()), [diagnostic]);
            pair.a
                .program(&json!([{"op":"off", "id":channel}]).to_string())
                .await;
        }
        prefix_and_tail_failures_report_separately(&mut pair, &errors).await;
        faulted_listener_keeps_healthy_delivery(&pair, &errors).await;
    });
}

/// Renders a probe view of `category`, keeping whatever the renderer returned.
fn probe(
    pair: &Pair,
    category: &str,
    error: &str,
) -> Result<Option<maestro_extensions::ComponentHandle>, maestro_extensions::EventError> {
    let renderer = pair
        .a
        .runner
        .get_message_renderer_call::<Value>("probe-view")
        .unwrap();
    let message = serde_json::from_value(json!({"role":"custom", "customType": format!("{category}|{error}"), "content": "", "display": true, "timestamp": 0})).unwrap();
    renderer.call(
        Rc::new(message),
        maestro_extensions::MessageRenderOptions { expanded: false },
        maestro_theme::LiveTheme::default(),
    )
}

/// Each renderer and view method keeps its authored error and its queued delivery.
fn renderer_and_component_errors(pair: &Pair, error: &str) {
    let failed = probe(pair, "renderer", error).err().unwrap();
    assert_eq!(failed.to_string(), error);
    assert_eq!(pair.take(), ["probe:prefix"]);
    let view = probe(pair, "render", error).unwrap().unwrap();
    assert_eq!(pair.take(), ["probe:prefix"]);
    assert_eq!(view.try_render(5).unwrap_err().to_string(), error);
    assert_eq!(pair.take(), ["probe:prefix"]);
    for method in ["input", "key-release", "invalidate"] {
        let view = probe(pair, method, error).unwrap().unwrap();
        pair.take();
        match method {
            "input" => view.input_handler().unwrap().handle_input("x"),
            "key-release" => assert!(!view.wants_key_release()),
            _ => view.invalidate(),
        }
        assert_eq!(pair.take(), ["probe:prefix"], "{method}");
        assert_eq!(
            view.try_render(5).unwrap_err().to_string(),
            error,
            "{method}"
        );
        assert_eq!(view.try_render(6).unwrap(), ["probe:6"], "{method}");
        pair.take();
    }
}

/// Preparation and both compaction notifications keep their authored errors and delivery.
async fn preparation_and_compaction_errors(pair: &Pair, requests: &Requests, error: &str) {
    let mode = json!([{"op":"prepare-mode", "mode":format!("fail:{error}")}]);
    pair.a.program(&mode.to_string()).await;
    let tools = pair.a.runner.get_all_registered_tools();
    let wrapped = wrap_registered_tool(&tools[0], pair.a.runner.clone());
    let failed = wrapped.prepare_arguments.as_ref().unwrap()(&mut json!({})).unwrap_err();
    assert_eq!(failed.message, error);
    assert_eq!(pair.take(), ["a:prepare", "prepare:prefix"]);
    let options = json!([{"op":"compact", "options":format!("fail:{error}")}]);
    pair.a.program(&options.to_string()).await;
    assert_eq!(pair.take(), ["operation", "request:return"]);
    let request = requests.borrow_mut().pop().unwrap().unwrap();
    let result: CompactionResult = serde_json::from_value(
        json!({"summary":"kept","firstKeptEntryId":"entry-2","tokensBefore":17}),
    )
    .unwrap();
    let complete = request.on_complete.unwrap()(result).unwrap_err();
    assert_eq!(complete.to_string(), error);
    assert_eq!(pair.take(), ["compaction:prefix"]);
    let notified = request.on_error.unwrap()("operation failed".into()).unwrap_err();
    assert_eq!(notified.to_string(), error);
    assert_eq!(pair.take(), ["compaction:prefix"]);
}

/// A prefix failure reports during the emit; a tail failure reports when the tail settles.
async fn prefix_and_tail_failures_report_separately(
    pair: &mut Pair,
    errors: &Rc<RefCell<Vec<String>>>,
) {
    let steps = json!([
        {"op":"on", "channel":"x", "id":"prefix-bad", "name":"bad", "fail":"prefix failure"},
        {"op":"on", "channel":"y", "id":"tail-bad", "name":"bad", "tail":{"fail":"tail failure"}},
    ]);
    pair.b.program(&steps.to_string()).await;
    let healthy = json!([
        {"op":"on", "channel":"x", "id":"x-healthy", "name":"healthy"},
        {"op":"on", "channel":"y", "id":"y-healthy", "name":"healthy"},
    ]);
    pair.a.program(&healthy.to_string()).await;
    pair.bus.emit("x", Value::Null).await;
    pair.trace.borrow_mut().push("return".into());
    assert_eq!(pair.take(), ["bad:prefix", "healthy:prefix", "return"]);
    let prefix = std::mem::take(&mut *errors.borrow_mut());
    assert_eq!(prefix, ["Event handler error (x): prefix failure\n"]);
    pair.bus.emit("y", Value::Null).await;
    pair.trace.borrow_mut().push("return".into());
    assert_eq!(pair.take(), ["bad:prefix", "healthy:prefix", "return"]);
    assert!(errors.borrow().is_empty());
    for tail in pair.queued() {
        tail.await;
    }
    let tail = std::mem::take(&mut *errors.borrow_mut());
    assert_eq!(tail, ["Event handler error (y): tail failure\n"]);
}

/// A trapped listener reports its fault on each delivery; another instance keeps receiving.
async fn faulted_listener_keeps_healthy_delivery(pair: &Pair, errors: &Rc<RefCell<Vec<String>>>) {
    pair.b
        .program(r#"[{"op":"on","channel":"z","id":"trap","name":"trap","trap":true}]"#)
        .await;
    pair.a
        .program(r#"[{"op":"on","channel":"z","id":"z-healthy","name":"healthy"}]"#)
        .await;
    pair.bus.emit("z", Value::Null).await;
    pair.bus.emit("z", Value::Null).await;
    assert_eq!(
        pair.take(),
        ["trap:prefix", "healthy:prefix", "healthy:prefix"]
    );
    let faults = std::mem::take(&mut *errors.borrow_mut());
    assert_eq!(faults.len(), 2);
    assert_eq!(faults[0], faults[1]);
    let fault = &faults[0];
    assert!(fault.starts_with("Event handler error (z): "), "{fault}");
    assert!(
        fault.contains("component fault") && fault.contains("wasm trap"),
        "{fault}"
    );
}
