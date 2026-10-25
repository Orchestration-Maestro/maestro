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
}

/// The phase reached by a prepared call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Validated {
    /// `preparation`, `validation` or `execution`.
    phase: String,
    /// Arguments the executor received, `null` included.
    #[serde(default, deserialize_with = "present")]
    arguments: Option<Value>,
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
            (wrapped.execute)("call".into(), valid, None, None)
                .await
                .unwrap();
            outcome.phase = "execution";
            outcome.executed = pair.payloads.borrow_mut().pop();
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
            if let Some(arguments) = &expected.arguments {
                let executed = outcome.executed.unwrap().to_string();
                assert_eq!(executed, arguments.to_string(), "case {index}");
            }
        }
    });
}

#[test]
fn prepare_callbacks_drain_before_execution_and_release() {
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
        (wrapped.execute)("call".into(), valid, None, None)
            .await
            .unwrap();
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
