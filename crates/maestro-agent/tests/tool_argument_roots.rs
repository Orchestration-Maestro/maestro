//! Tool argument roots through preparation, validation and execution.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::{Arc, Mutex, RwLock};

use crate::support;
use maestro_agent::{AgentEvent, AgentTool, AgentToolResult, run_agent_loop_continue};
use maestro_models::{AssistantContent, DiagnosticErrorInfo, Tool, ToolCall, UserBlock};
use serde_json::{Value, json};

/// One argument preparation callback.
type Prepare = Arc<dyn Fn(&mut Value) -> Result<Value, DiagnosticErrorInfo> + Send + Sync>;

/// The loop's event observer.
type Emit = Arc<
    dyn Fn(AgentEvent) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> + Send + Sync,
>;

/// One preparation case: label, input root, declaration, preparation, prepared and retained.
type Case = (&'static str, Value, Value, Option<Prepare>, Value, Value);

/// What one loop turn observed.
struct Turn {
    /// Arguments each execution received, in call order.
    executed: Vec<Value>,
    /// Validated arguments the hook observed.
    hooked: Vec<Value>,
    /// Arguments the retained assistant message still carries.
    retained: Value,
    /// Text of every failed tool result, in call order.
    failures: Vec<String>,
}

/// Borrow one value into the shared slot a polled turn writes.
fn shared<T>(value: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(value))
}

/// The echo tool that records the arguments each execution receives.
fn echo_tool(schema: Value, prepare: Option<Prepare>, seen: Arc<Mutex<Vec<Value>>>) -> AgentTool {
    AgentTool {
        description: None,
        definition: Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: schema.into(),
        },
        label: "Echo".into(),
        execution_mode: None,
        prepare_arguments: prepare,
        execute: Arc::new(move |_, args, _, _| {
            seen.lock().expect("arguments").push(args);
            Box::pin(async {
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: Some(Value::Null),
                    terminate: Some(true),
                    usage: None,
                })
            })
        }),
    }
}

/// The loop configuration that records validated arguments and failed results.
fn observing_config(
    hooked: Arc<Mutex<Vec<Value>>>,
    failures: Arc<Mutex<Vec<String>>>,
) -> (maestro_agent::AgentLoopConfig, Emit) {
    let mut config = support::config();
    config.before_tool_call = Some(Arc::new(move |request, _| {
        let hooked = Arc::clone(&hooked);
        Box::pin(async move {
            hooked.lock().expect("hook").push(request.args.clone());
            Ok(None)
        })
    }));
    let emit = Arc::new(move |event| {
        let failures = Arc::clone(&failures);
        Box::pin(async move {
            if let AgentEvent::ToolExecutionEnd {
                result,
                is_error: true,
                ..
            } = event
            {
                failures
                    .lock()
                    .expect("failures")
                    .push(UserBlock::text_output(&result.content));
            }
            Ok(())
        }) as maestro_models::BoxFuture<_>
    });
    (config, emit)
}

/// Drive one turn, optionally with the reply a failed tool result needs to settle.
async fn turn_with_follow_up(
    arguments: Value,
    schema: Value,
    prepare: Option<Prepare>,
    follow_up: bool,
) -> Turn {
    let seen = shared(Vec::new());
    let hooked = shared(Vec::new());
    let failures = shared(Vec::new());
    let response = support::assistant("call");
    response.write().expect("assistant").content = vec![AssistantContent::ToolCall(ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments: arguments.clone(),
        thought_signature: None,
    })];
    let tool = echo_tool(schema, prepare, Arc::clone(&seen));
    let mut context = support::context();
    context.tools = Some(Arc::new(RwLock::new(vec![Arc::new(RwLock::new(tool))])));
    let (config, emit) = observing_config(Arc::clone(&hooked), Arc::clone(&failures));
    let mut queued = vec![response.clone()];
    if follow_up {
        queued.push(support::assistant("done"));
    }
    run_agent_loop_continue(context, &config, emit, support::responses(queued))
        .await
        .expect("one loop turn");
    let retained = match &response.read().expect("response").content[0] {
        AssistantContent::ToolCall(call) => call.arguments.clone(),
        other => panic!("expected a retained call, found {other:?}"),
    };
    Turn {
        executed: seen.lock().expect("arguments").clone(),
        hooked: hooked.lock().expect("hook").clone(),
        retained,
        failures: failures.lock().expect("failures").clone(),
    }
}

/// Drive one successful turn.
async fn turn(arguments: Value, schema: Value, prepare: Option<Prepare>) -> Turn {
    turn_with_follow_up(arguments, schema, prepare, false).await
}

/// One mutation the preparation makes before returning its value.
fn editing(edit: fn(&mut Value), returned: Value) -> Prepare {
    Arc::new(move |value: &mut Value| {
        edit(value);
        Ok(returned.clone())
    })
}

/// A converted root that fails its schema never reaches execution.
#[test]
fn failed_root_conversion_never_reaches_execution() {
    support::run(async {
        let rejected = turn_with_follow_up(
            json!("42"),
            json!({"type": "number", "minimum": 100}),
            None,
            true,
        )
        .await;
        assert!(rejected.executed.is_empty(), "execution stays uninvoked");
        assert!(rejected.hooked.is_empty(), "the hook stays uninvoked");
        assert_eq!(
            rejected.failures,
            vec![
                "Validation failed for tool \"echo\":\n  - root: must be >= 100\n\nReceived arguments:\n\"42\""
                    .to_owned()
            ]
        );

        let accepted = turn(json!("42"), json!({"type": "number"}), None).await;
        assert_eq!(accepted.executed, vec![json!(42.0)]);
        assert_eq!(accepted.hooked, vec![json!(42.0)]);

        let wrong = turn_with_follow_up(json!("text"), json!({"type": "object"}), None, true).await;
        assert!(wrong.executed.is_empty());
        assert_eq!(
            wrong.failures,
            vec![
                "Validation failed for tool \"echo\":\n  - root: must be object\n\nReceived arguments:\n\"text\""
                    .to_owned()
            ]
        );
    });
}

/// Execution receives the validated root while the message keeps the original one.
#[test]
fn agent_execution_receives_validated_root_values() {
    support::run(async {
        for (arguments, schema) in [
            (json!("text"), json!({"type": "string"})),
            (json!(42), json!({"type": "number"})),
            (json!(false), json!({"type": "boolean"})),
            (Value::Null, json!({"type": "null"})),
            (
                json!([1, 1]),
                json!({"type": "array", "items": {"type": "number"}}),
            ),
            (json!({"q": 1}), json!({"type": "object"})),
        ] {
            let observed = turn(arguments.clone(), schema, None).await;
            assert!(observed.failures.is_empty(), "{arguments}");
            assert_eq!(observed.executed, vec![arguments.clone()], "{arguments}");
            assert_eq!(observed.hooked, vec![arguments.clone()], "{arguments}");
            assert_eq!(observed.retained, arguments, "{arguments}");
        }
    });
}

/// The recorded preparation actions, with the value each returns and keeps.
fn preparation_cases() -> Vec<Case> {
    let mut cases = replacement_cases();
    cases.extend(editing_cases());
    cases.extend(replacement_after_edit_cases());
    cases
}

/// The actions that return another value without editing their input.
fn replacement_cases() -> Vec<Case> {
    vec![
        (
            "unchanged scalar",
            json!(42),
            json!({"type": "number"}),
            None,
            json!(42),
            json!(42),
        ),
        (
            "replace scalar",
            json!(42),
            json!(true),
            Some(Arc::new(|_: &mut Value| Ok(json!(false)))),
            json!(false),
            json!(42),
        ),
        (
            "replace array",
            json!([1, 2]),
            json!(true),
            Some(Arc::new(|_: &mut Value| Ok(Value::Null))),
            Value::Null,
            json!([1, 2]),
        ),
    ]
}

/// The actions that edit their input before returning.
fn editing_cases() -> Vec<Case> {
    vec![
        (
            "in-place array edit",
            json!([1, 2]),
            json!({"type": "array"}),
            Some(editing(
                |value| {
                    if let Value::Array(items) = value {
                        items.push(json!(3));
                    }
                },
                json!([1, 2, 3]),
            )),
            json!([1, 2, 3]),
            json!([1, 2, 3]),
        ),
        (
            "member addition",
            json!({"a": 1}),
            json!({"type": "object"}),
            Some(editing(
                |value| {
                    if let Value::Object(members) = value {
                        members.insert("b".into(), json!(2));
                    }
                },
                json!({"a": 1, "b": 2}),
            )),
            json!({"a": 1, "b": 2}),
            json!({"a": 1, "b": 2}),
        ),
        (
            "member deletion",
            json!({"a": 1, "b": 2}),
            json!({"type": "object"}),
            Some(editing(
                |value| {
                    if let Value::Object(members) = value {
                        members.shift_remove("a");
                    }
                },
                json!({"b": 2}),
            )),
            json!({"b": 2}),
            json!({"b": 2}),
        ),
    ]
}

/// The action that edits its input and then returns another value.
fn replacement_after_edit_cases() -> Vec<Case> {
    vec![(
        "mutate then replace",
        json!({"a": 1}),
        json!(true),
        Some(editing(
            |value| {
                if let Value::Object(members) = value {
                    members.insert("a".into(), json!(2));
                }
            },
            json!("replacement"),
        )),
        json!("replacement"),
        json!({"a": 2}),
    )]
}

/// Preparation keeps its own edits and the caller's original value apart.
#[test]
fn argument_preparation_preserves_original_and_returned_roots() {
    support::run(async {
        for (label, arguments, schema, prepare, prepared, retained) in preparation_cases() {
            let observed = turn(arguments.clone(), schema, prepare).await;
            assert!(observed.failures.is_empty(), "{label}");
            assert_eq!(observed.executed, vec![prepared.clone()], "{label}");
            assert_eq!(observed.hooked, vec![prepared], "{label}");
            assert_eq!(observed.retained, retained, "{label}");
        }
    });
}

/// A preparation that fails keeps the edits it already made and runs nothing.
#[test]
fn argument_preparation_failure_keeps_its_edits() {
    support::run(async {
        let thrown = turn_with_follow_up(
            json!({"a": 1}),
            json!({"type": "object"}),
            Some(editing(
                |value| {
                    if let Value::Object(members) = value {
                        members.insert("a".into(), json!(2));
                    }
                },
                json!({"a": 2}),
            ))
            .map(|prepare| {
                Arc::new(move |value: &mut Value| {
                    prepare(value)?;
                    Err(DiagnosticErrorInfo::error("prepare failed".to_owned()))
                }) as Prepare
            }),
            true,
        )
        .await;
        assert!(thrown.executed.is_empty());
        assert_eq!(thrown.failures, vec!["prepare failed".to_owned()]);
        assert_eq!(
            thrown.retained,
            json!({"a": 2}),
            "the input edits the preparation made stay in history"
        );
    });
}

/// A call whose root the declaration accepts reaches execution unchanged.
#[test]
fn argument_replacement_keeps_the_original_root() {
    support::run(async {
        let observed = turn(
            json!(false),
            json!(true),
            Some(Arc::new(|_: &mut Value| Ok(json!([])))),
        )
        .await;
        assert_eq!(observed.executed, vec![json!([])]);
        assert_eq!(observed.retained, json!(false));
    });
}
