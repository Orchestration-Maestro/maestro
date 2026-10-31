//! Tool hook observations through awaited conversation operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{BeforeToolCallContext, run_agent_loop};
use std::sync::Arc;
use support::*;

/// Validated arguments can be changed to values outside their original schema.
#[test]
fn before_hook_mutation_reaches_execution_without_revalidation() {
    run(async {
        mutation_case().await;
        support::corpus::replay_controls(&[
            "prepare-then-mutate",
            "mutate-without-revalidation",
            "string-schema-mutation",
        ])
        .await;
    });
}
/// Exercise schema edits through the public operation.
async fn mutation_case() {
    for (schema, raw, edited) in [
        (
            "integer",
            serde_json::json!("raw"),
            serde_json::json!("nonnumeric"),
        ),
        (
            "string",
            serde_json::json!("original"),
            serde_json::json!(123),
        ),
    ] {
        let context = mutation_context(schema, edited.clone());
        let mut config = config();
        let raw_events = raw.clone();
        let original = raw.clone();
        config.before_tool_call = Some(Arc::new(move |c: BeforeToolCallContext<'_, _>, _| {
            assert_eq!(c.tool_call.arguments["value"], original);
            if schema == "integer" {
                assert_eq!(c.args["value"].as_f64(), Some(8.0));
            }
            let edited = edited.clone();
            Box::pin(async move {
                c.args["value"] = edited;
                Ok(None)
            })
        }));
        let call = tool_call();
        if let maestro_models::AssistantContent::ToolCall(c) = &mut call.write().unwrap().content[0]
        {
            c.arguments
                .as_object_mut()
                .expect("object root")
                .insert("value".into(), raw);
        }
        let returned = run_agent_loop(
            vec![],
            context,
            &config,
            Arc::new(move |e| {
                if let maestro_agent::AgentEvent::ToolExecutionStart { tool_call }
                | maestro_agent::AgentEvent::ToolExecutionUpdate { tool_call, .. } = e
                {
                    assert_eq!(tool_call.arguments["value"], raw_events);
                }
                ready(Ok(()))
            }),
            responses(vec![call]),
        )
        .await
        .unwrap();
        assert_eq!(returned.len(), 2);
    }
}

/// After hooks can recover execution errors and their own error takes precedence.
#[test]
fn after_hook_recovers_execution_errors_and_reports_own_failures() {
    run(async {
        recovery_case().await;
        support::corpus::replay_controls(&[
            "failure-execute",
            "failure-after",
            "after-execution-error-40",
            "after-execution-error-41",
            "after-execution-error-42",
            "after-execution-error-43",
        ])
        .await;
    });
}
/// Exercise execution and finalization error precedence.
async fn recovery_case() {
    for failing_hook in [false, true] {
        let context = with_execute(Arc::new(|_, _, _, _| {
            ready(Err(diagnostic("execute failed")))
        }));
        let mut config = config();
        config.after_tool_call = Some(Arc::new(move |c, _| {
            assert!(c.is_error);
            assert_eq!(c.result.details, Some(serde_json::json!({})));
            assert!(
                matches!(&c.result.content[0], maestro_models::UserBlock::Text(t) if t.text == "execute failed")
            );
            let outcome = if failing_hook {
                Err(diagnostic("after failed"))
            } else {
                Ok(Some(maestro_agent::AfterToolCallResult {
                    content: Some(vec![]),
                    is_error: Some(false),
                    terminate: Some(true),
                    ..Default::default()
                }))
            };
            Box::pin(std::future::ready(outcome))
        }));
        let returned = run_agent_loop(
            vec![],
            context,
            &config,
            Arc::new(|_| ready(Ok(()))),
            responses(vec![tool_call(), assistant("done")]),
        )
        .await
        .unwrap();
        let maestro_agent::AgentMessage::ToolResult(r) = &returned[1] else {
            panic!("tool result")
        };
        let r = r.read().unwrap();
        assert_eq!(r.is_error, failing_hook);
        if failing_hook {
            assert!(
                matches!(&r.content[0], maestro_models::UserBlock::Text(t) if t.text == "after failed")
            );
            assert_eq!(returned.len(), 3);
        } else {
            assert!(r.content.is_empty());
            assert_eq!(returned.len(), 2);
        }
    }
}
/// Construct controlled, nonsecret diagnostic text.
fn diagnostic(message: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::new(None, message)
}

/// Blocking retains exact nonempty reason bytes and permits false blocks.
#[test]
fn blocked_tools_keep_exact_nonempty_reasons() {
    run(support::corpus::replay_controls(&[
        "before-3",
        "before-4",
        "before-5",
        "before-6",
        "before-7",
        "before-8",
        "before-9",
        "before-10",
        "before-11",
        "before-12",
    ]));
}

/// Preparation, validation and blocking failures bypass executable finalization.
#[test]
fn preflight_failures_skip_execution_and_after_hooks() {
    run(support::corpus::replay_controls(&[
        "failure-prepare",
        "failure-validation",
        "failure-before",
        "preparation-wins-before-hook-error",
        "validation-wins-before-hook-error",
        "before-block-skips-after",
    ]));
}

/// After overrides preserve nullish fields and replace complete admitted values.
#[test]
fn after_hook_overrides_replace_whole_nonnull_fields() {
    run(support::corpus::replay_controls(&[
        "after-23",
        "after-24",
        "after-25",
        "after-26",
        "after-27",
        "after-28",
        "after-29",
        "after-30",
        "after-31",
        "after-32",
        "after-33",
        "after-34",
        "after-35",
        "after-36",
        "after-37",
        "after-38",
        "after-39",
        "retain-details-44",
        "retain-details-45",
        "retain-details-46",
        "retain-details-47",
        "retain-details-48",
        "retain-details-49",
    ]));
}

/// Hooks observe requesting records, edited arguments and invocation cancellation.
#[test]
fn hooks_receive_current_records_and_invocation_signal() {
    run(async {
        signal_identity_case().await;
        support::corpus::replay_controls(&["signal-absent", "signal-present", "signal-aborted"])
            .await;
    });
}

/// Borrowed hooks permit reentry and their captures release with the last alias.
#[test]
fn hook_reentry_releases_guards_and_retained_captures() {
    run(async {
        let lifetime = Arc::new(());
        let weak = Arc::downgrade(&lifetime);
        let context = with_execute(Arc::new(|_, args, _, _| {
            assert_eq!(args["edited"], true);
            ready(Ok(result()))
        }));
        let inventory = Arc::clone(context.tools.as_ref().unwrap());
        let mut config = config();
        config.before_tool_call = Some(Arc::new(move |c, _| {
            assert!(c.context.messages.try_write().is_ok());
            assert!(c.context.tools.as_ref().unwrap().try_write().is_ok());
            let retained = Arc::clone(&lifetime);
            Box::pin(async move {
                c.args["edited"] = serde_json::json!(true);
                assert_eq!(Arc::strong_count(&retained), 2);
                Ok(None)
            })
        }));
        config.after_tool_call = Some(Arc::new(|c, _| {
            assert!(c.context.messages.try_write().is_ok());
            assert!(c.context.tools.as_ref().unwrap().try_write().is_ok());
            Box::pin(async { Ok(None) })
        }));
        config.should_stop_after_turn = Some(Arc::new(|c| {
            assert!(c.context.messages.try_write().is_ok());
            assert!(c.context.tools.as_ref().unwrap().try_write().is_ok());
            Box::pin(async { Ok(false) })
        }));
        let captured = Arc::clone(&inventory);
        config.get_steering_messages = Some(Arc::new(move || {
            assert!(captured.try_write().is_ok());
            ready(Ok(vec![]))
        }));
        config.get_follow_up_messages = Some(Arc::new(move || {
            assert!(inventory.try_write().is_ok());
            ready(Ok(vec![]))
        }));
        let returned = run_agent_loop(
            vec![],
            context,
            &config,
            Arc::new(|_| ready(Ok(()))),
            responses(vec![tool_call()]),
        )
        .await
        .unwrap();
        assert_eq!(returned.len(), 2);
        let alias = config.before_tool_call.take().unwrap();
        drop(config);
        assert!(weak.upgrade().is_some());
        drop(alias);
        assert!(weak.upgrade().is_none());
    });
}

/// Aborting a hook's signal changes the original invocation, not configured options.
async fn signal_identity_case() {
    let invocation = maestro_models::Cancellation::new();
    let configured = maestro_models::Cancellation::new();
    let requesting = tool_call();
    let expected = Arc::clone(&requesting);
    let mut config = config();
    config.options.common.signal = Some(configured.clone());
    config.before_tool_call = Some(Arc::new(move |c, signal| {
        assert!(Arc::ptr_eq(c.assistant_message, &expected));
        assert!(signal.is_some(), "invocation signal reaches before-hook");
        signal.unwrap().abort();
        Box::pin(async { Ok(None) })
    }));
    config.after_tool_call = Some(Arc::new(|_, _| {
        panic!("an aborted call reaches no after hook");
    }));
    let context = with_execute(Arc::new(|_, _, _, _| {
        panic!("an aborted call is not executed");
    }));
    let mut options = responses(vec![requesting, assistant("done")]);
    options.signal = Some(invocation.clone());
    let returned = run_agent_loop(
        vec![],
        context,
        &config,
        Arc::new(|_| ready(Ok(()))),
        options,
    )
    .await
    .unwrap();
    assert_eq!(returned.len(), 3);
    assert!(invocation.is_aborted());
    assert!(!configured.is_aborted());
}

/// Supply distinct authored and prepared arguments for schema-mutation cases.
fn mutation_context(schema: &str, expected: serde_json::Value) -> maestro_agent::AgentContext {
    let context = with_execute(Arc::new(move |_, args, _, update| {
        assert_eq!(args["value"], expected);
        update.unwrap()(result());
        ready(Ok(result()))
    }));
    context.tools.as_ref().unwrap().read().unwrap()[0]
        .write()
        .unwrap()
        .definition
        .parameters = serde_json::json!({"type":"object","properties":{"value":{"type":schema}},"required":["value"]}).into();
    if schema == "integer" {
        context.tools.as_ref().unwrap().read().unwrap()[0]
            .write()
            .unwrap()
            .prepare_arguments = Some(Arc::new(|_| Ok(serde_json::json!({"value":"8"}))));
    }
    context
}

/// Cancellation during start observation prevents execution even without a hook.
#[test]
fn abort_during_tool_start_without_hook_skips_execution() {
    run(async {
        let signal = maestro_models::Cancellation::new();
        let aborter = signal.clone();
        let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called = Arc::clone(&executed);
        let context = with_execute(Arc::new(move |_, _, _, _| {
            called.store(true, std::sync::atomic::Ordering::Relaxed);
            ready(Ok(result()))
        }));
        let mut options = responses(vec![tool_call()]);
        options.signal = Some(signal);
        let returned = run_agent_loop(
            vec![],
            context,
            &config(),
            Arc::new(move |event| {
                if matches!(event, maestro_agent::AgentEvent::ToolExecutionStart { .. }) {
                    aborter.abort();
                }
                ready(Ok(()))
            }),
            options,
        )
        .await
        .unwrap();
        assert!(!executed.load(std::sync::atomic::Ordering::Relaxed));
        let maestro_agent::AgentMessage::ToolResult(message) = &returned[1] else {
            panic!("tool result");
        };
        let message = message.read().unwrap();
        assert!(message.is_error);
        assert!(
            matches!(&message.content[0], maestro_models::UserBlock::Text(text)
            if text.text == "Operation aborted")
        );
    });
}
