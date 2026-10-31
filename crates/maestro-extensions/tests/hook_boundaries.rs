//! Hook verdicts at executable tool and shell boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::local_support as support;
use crate::tool_support;
use maestro_extensions::*;
use maestro_session::Presence;
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

fn tool_call() -> ToolCallEvent {
    ToolCallEvent {
        tool_call_id: "call".into(),
        tool_name: "echo".into(),
        input: json!("admitted"),
    }
}
#[test]
fn tool_call_edits_reach_next_handler_and_execution_without_revalidation() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("tool", bus.controller.for_extension());
        let mut definition = tool_support::definition("echo", "Echo");
        definition.definition.parameters = json!({"type":"string"}).into();
        loaded.api.register_tool(definition).unwrap();
        let runner = Rc::new(support::runner(&loaded.owners));
        loaded
            .api
            .on::<ToolCallEvent, ToolCallEventResult>(
                "tool_call".into(),
                Rc::new(|event, _| {
                    event.input = json!(false);
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
        loaded
            .api
            .on::<ToolCallEvent, ToolCallEventResult>(
                "tool_call".into(),
                Rc::new(|event, _| {
                    assert_eq!(event.input, json!(false));
                    assert_eq!(event.tool_call_id, "call");
                    assert_eq!(event.tool_name, "echo");
                    Box::pin(std::future::ready(Ok(Some(ToolCallEventResult {
                        block: Presence::Present(false),
                        reason: Presence::Missing,
                    }))))
                }),
            )
            .unwrap();
        let mut event = tool_call();
        let result = runner.emit_tool_call(&mut event).await.unwrap().unwrap();
        assert!(matches!(result.block, Presence::Present(false)));
        let registered = runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&registered[0], runner.clone());
        let result = (tool.execute)(event.tool_call_id, event.input, None, None)
            .await
            .unwrap();
        assert_eq!(result.details, Some(json!(false)));
        bus.settle().await;
    });
}

#[test]
fn tool_call_keeps_last_nonblocking_result_and_first_block() {
    support::local(tool_call_keeps_last_nonblocking_result_and_first_block_case());
}
use crate::error_support;
#[test]
fn tool_call_failure_propagates_without_diagnostic_or_execution() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("call", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let trace = support::Trace::default();
        let diagnostics = trace.clone();
        let _remove = runner.on_error(Rc::new(move |_| {
            support::mark(&diagnostics, "diagnostic");
            Ok(())
        }));
        let failure = Box::new(error_support::Marker("original"));
        let identity = std::ptr::from_ref(failure.as_ref());
        let failure: RefCell<Option<EventError>> = RefCell::new(Some(failure));
        loaded
            .api
            .on::<ToolCallEvent, ToolCallEventResult>(
                "tool_call".into(),
                Rc::new(move |event, _| {
                    event.input = json!({"edited":true});
                    let error = failure.borrow_mut().take().unwrap();
                    Box::pin(std::future::ready(Err(error)))
                }),
            )
            .unwrap();
        let later = trace.clone();
        loaded
            .api
            .on::<ToolCallEvent, ToolCallEventResult>(
                "tool_call".into(),
                Rc::new(move |_, _| {
                    support::mark(&later, "later");
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
        let mut event = tool_call();
        let failure = runner.emit_tool_call(&mut event).await.unwrap_err();
        assert!(std::ptr::eq(
            identity,
            failure
                .downcast_ref::<error_support::Marker<&str>>()
                .unwrap()
        ));
        assert_eq!(event.input, json!({"edited":true}));
        assert!(trace.borrow().is_empty());
        bus.settle().await;
    });
}

fn tool_result() -> ToolResultEvent {
    ToolResultEvent {
        tool_call_id: "call".into(),
        tool_name: "echo".into(),
        input: json!({"original":true}),
        content: serde_json::from_value(json!([{ "type":"text","text":"original" }])).unwrap(),
        is_error: true,
        details: Presence::Present(json!({"original":true})),
    }
}
#[test]
fn tool_result_patches_present_fields_and_preserves_prior_changes() {
    support::local(tool_result_patches_present_fields_and_preserves_prior_changes_case());
}

#[test]
fn tool_result_empty_patch_leaves_absence_and_original_top_level() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("result", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        loaded
            .api
            .on::<ToolResultEvent, ToolResultEventResult>(
                "tool_result".into(),
                Rc::new(|event, _| {
                    event.tool_name = "edited".into();
                    event.input = json!({"edited":true});
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
        loaded
            .api
            .on::<ToolResultEvent, ToolResultEventResult>(
                "tool_result".into(),
                Rc::new(|event, _| {
                    assert_eq!(event.tool_name, "edited");
                    assert_eq!(event.input, json!({"edited":true}));
                    Box::pin(std::future::ready(Ok(Some(ToolResultEventResult {
                        content: Presence::Missing,
                        details: Presence::Missing,
                        is_error: Presence::Missing,
                    }))))
                }),
            )
            .unwrap();
        assert!(
            runner
                .emit_tool_result(&tool_result())
                .await
                .unwrap()
                .is_none()
        );
        bus.settle().await;
    });
}
#[path = "../../maestro-tools/tests/agent_shell_support/immediate.rs"]
mod shell_support;
#[test]
fn user_bash_keeps_first_result_and_invocable_operations() {
    support::local(user_bash_keeps_first_result_and_invocable_operations_case());
}

async fn tool_result_patches_present_fields_and_preserves_prior_changes_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("result", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    for patch in [
        json!({"content":[]}),
        json!({"details":null}),
        json!({"isError":false}),
        json!("failure"),
        json!({"details":{"final":true}}),
    ] {
        install_tool_result_patch(&loaded, patch);
    }
    let original = tool_result();
    let result = runner.emit_tool_result(&original).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({"content":[],"details":{"final":true},"isError":false})
    );
    bus.settle().await;
    combined_tool_result_patch().await;
}

async fn user_bash_keeps_first_result_and_invocable_operations_case() {
    use maestro_tools::{BashExecution, BashResult};
    use std::sync::Arc;
    type Ops = Arc<<maestro_models::Threaded as BashExecution>::Operations>;
    let bus = support::LocalBus::new();
    let loaded = support::loaded("bash", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    let mut event = UserBashEvent {
        command: "literal command".into(),
        cwd: "/work".into(),
        exclude_from_context: false,
    };
    assert!(
        runner
            .emit_user_bash::<Ops, BashResult>(&mut event)
            .await
            .unwrap()
            .is_none()
    );
    let operations: Ops = Arc::new(shell_support::Immediate(Arc::new(
        |command, cwd, options| {
            assert_eq!(command, "literal command");
            assert_eq!(cwd, "/work");
            (options.on_data)(b"controlled output");
            Ok(maestro_tools::BashExecResult {
                exit_code: Some(7.0),
            })
        },
    )));
    install_shell_operations(&loaded, &operations);
    let result = runner
        .emit_user_bash::<Ops, BashResult>(&mut event)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result.result, Presence::Missing));
    let returned = result.operations.unwrap();
    assert!(Arc::ptr_eq(&returned, &operations));
    let result = maestro_tools::execute_bash_with_operations(
        &event.command,
        &event.cwd,
        returned.as_ref(),
        maestro_tools::BashExecutorOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.output, "controlled output");
    assert_eq!(result.exit_code, Some(7.0));
    bus.settle().await;
    shell_result_answers(&mut event).await;
}

async fn combined_tool_result_patch() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("combined-patch", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    loaded
        .api
        .on::<ToolResultEvent, ToolResultEventResult>(
            "tool_result".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(
                    serde_json::from_value(json!({"content":[],"details":null,"isError":false}))
                        .unwrap(),
                ))))
            }),
        )
        .unwrap();
    let result = runner
        .emit_tool_result(&tool_result())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({"content":[],"details":null,"isError":false})
    );
    bus.settle().await;
}

fn install_tool_result_patch(loaded: &support::Loaded, patch: serde_json::Value) {
    loaded
        .api
        .on::<ToolResultEvent, ToolResultEventResult>(
            "tool_result".into(),
            Rc::new(move |event, _| {
                if patch == "failure" {
                    assert!(event.content.is_empty());
                    assert!(!event.is_error);
                    assert!(matches!(event.details, Presence::Null));
                }
                let patch = patch.clone();
                Box::pin(std::future::ready(if patch == "failure" {
                    Err("patch failure".into())
                } else {
                    Ok(Some(serde_json::from_value(patch).unwrap()))
                }))
            }),
        )
        .unwrap();
}

async fn shell_result_answers(event: &mut UserBashEvent) {
    use maestro_tools::{BashExecution, BashResult};
    use std::sync::Arc;
    type Ops = Arc<<maestro_models::Threaded as BashExecution>::Operations>;
    for answer in [
        json!({}),
        json!({"result":{"output":"replacement","exitCode":3.0,"cancelled":false,"truncated":false}}),
    ] {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("bash-result", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let expected = answer.clone();
        loaded
            .api
            .on::<UserBashEvent, UserBashEventResult<Ops, BashResult>>(
                "user_bash".into(),
                Rc::new(move |_, _| {
                    let answer = answer.clone();
                    Box::pin(std::future::ready(Ok(Some(
                        serde_json::from_value(answer).unwrap(),
                    ))))
                }),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(
                runner
                    .emit_user_bash::<Ops, BashResult>(event)
                    .await
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            expected
        );
        bus.settle().await;
    }
}

fn install_shell_operations(
    loaded: &support::Loaded,
    operations: &std::sync::Arc<
        <maestro_models::Threaded as maestro_tools::BashExecution>::Operations,
    >,
) {
    use maestro_tools::BashResult;
    type Ops =
        std::sync::Arc<<maestro_models::Threaded as maestro_tools::BashExecution>::Operations>;
    loaded
        .api
        .on::<UserBashEvent, UserBashEventResult<Ops, BashResult>>(
            "user_bash".into(),
            Rc::new(|_, _| Box::pin(std::future::ready(Err("bash failure".into())))),
        )
        .unwrap();
    loaded
        .api
        .on::<UserBashEvent, UserBashEventResult<Ops, BashResult>>(
            "user_bash".into(),
            Rc::new(|_, _| Box::pin(std::future::ready(Ok(None)))),
        )
        .unwrap();
    let selected = operations.clone();
    loaded
        .api
        .on::<UserBashEvent, UserBashEventResult<Ops, BashResult>>(
            "user_bash".into(),
            Rc::new(move |_, _| {
                let operations = selected.clone();
                Box::pin(std::future::ready(Ok(Some(UserBashEventResult {
                    operations: Some(operations),
                    result: Presence::Missing,
                }))))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<UserBashEvent, UserBashEventResult<Ops, BashResult>>(
            "user_bash".into(),
            Rc::new(|_, _| panic!("later hook")),
        )
        .unwrap();
}

async fn tool_call_keeps_last_nonblocking_result_and_first_block_case() {
    for (answers, expected, visits) in [
        (vec![None], None, 1),
        (vec![Some(json!({}))], Some(json!({})), 1),
        (
            vec![
                Some(json!({"block":false,"reason":"earlier"})),
                Some(json!({"reason":"last"})),
                None,
            ],
            Some(json!({"reason":"last"})),
            3,
        ),
        (
            vec![
                Some(json!({"block":false})),
                Some(json!({"block":true,"reason":"stop"})),
                Some(json!({"reason":"unreached"})),
            ],
            Some(json!({"block":true,"reason":"stop"})),
            2,
        ),
    ] {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("call", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let called = Rc::new(std::cell::Cell::new(0));
        for answer in answers {
            let called = called.clone();
            loaded
                .api
                .on::<ToolCallEvent, ToolCallEventResult>(
                    "tool_call".into(),
                    Rc::new(move |event, _| {
                        assert_eq!(event.tool_call_id, "call");
                        assert_eq!(event.tool_name, "echo");
                        assert_eq!(event.input, json!("admitted"));
                        called.set(called.get() + 1);
                        let answer = answer.clone();
                        Box::pin(std::future::ready(Ok(
                            answer.map(|value| serde_json::from_value(value).unwrap())
                        )))
                    }),
                )
                .unwrap();
        }
        let result = runner
            .emit_tool_call(&mut tool_call())
            .await
            .unwrap()
            .map(|value| serde_json::to_value(value).unwrap());
        assert_eq!(result, expected);
        assert_eq!(called.get(), visits);
        bus.settle().await;
    }
}
