//! Live registered hook dispatch through the native runner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::local_support as support;
use maestro_extensions::*;
use std::rc::Rc;

#[test]
fn handler_presence_observes_live_known_and_open_registration_names() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("extension", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        assert!(!runner.has_handlers("agent_start"));
        loaded
            .owner
            .write()
            .unwrap()
            .handlers
            .insert("empty".into(), vec![]);
        assert!(!runner.has_handlers("empty"));
        runner
            .bind_core(
                maestro_models::Local::unbound_actions(),
                ExtensionContextActions::default(),
                types::ProviderActions::default(),
            )
            .unwrap();
        for name in ["agent_start", "", " unknown "] {
            loaded
                .api
                .on::<AgentStartEvent, ()>(
                    name.into(),
                    Rc::new(|_, _| Box::pin(std::future::ready(Ok(None)))),
                )
                .unwrap();
            assert!(runner.has_handlers(name));
        }
        assert!(!runner.has_handlers("Agent_start"));
        assert!(!runner.has_handlers("unknown"));
        bus.settle().await;
    });
}

use crate::model_support;
use maestro_extensions::runner::RunnerEmitEvent;
use maestro_session::Presence;
use serde_json::{Value, json};
use std::cell::RefCell;

fn register_notifications<V: RunnerEmitEvent<Result = ()>>(
    loaded: &support::Loaded,
    trace: &support::Trace,
    cases: &[(&'static str, bool)],
) {
    for &(label, fails) in cases {
        let trace = trace.clone();
        loaded
            .api
            .on::<V, ()>(
                V::EVENT_TYPE.into(),
                Rc::new(move |_, context| {
                    assert_eq!(context.cwd().unwrap(), "/work");
                    support::mark(&trace, label);
                    Box::pin(std::future::ready(if fails {
                        Err(label.into())
                    } else {
                        Ok(Some(()))
                    }))
                }),
            )
            .unwrap();
    }
}

async fn notification<V: RunnerEmitEvent<Result = ()>>(mut event: V) {
    let bus = support::LocalBus::new();
    let first = support::loaded("first", bus.controller.for_extension());
    let second = support::loaded("second", bus.controller.for_extension());
    first.owners.write().unwrap().push(second.owner.clone());
    let runner = support::runner(&first.owners);
    let trace = support::Trace::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let observed = errors.clone();
    let _remove = runner.on_error(Rc::new(move |error| {
        observed.borrow_mut().push((
            error.extension_path.clone(),
            error.event.clone(),
            error.error.clone(),
        ));
        Ok(())
    }));
    register_notifications::<V>(
        &first,
        &trace,
        &[
            ("first-error", true),
            ("a", false),
            ("middle-error", true),
            ("b", false),
        ],
    );
    register_notifications::<V>(&second, &trace, &[("c", false), ("last-error", true)]);
    for _ in 0..2 {
        let start = trace.borrow().len();
        assert!(runner.emit(&mut event).await.unwrap().is_none());
        assert_eq!(
            &trace.borrow()[start..],
            ["first-error", "a", "middle-error", "b", "c", "last-error"]
        );
    }
    assert_eq!(errors.borrow().len(), 6);
    assert_eq!(
        errors.borrow()[0],
        ("first".into(), V::EVENT_TYPE.into(), "first-error".into())
    );
    assert_eq!(
        errors.borrow()[2],
        ("second".into(), V::EVENT_TYPE.into(), "last-error".into())
    );
    bus.settle().await;
}

fn from_json<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

#[test]
fn notification_events_await_live_handlers_and_isolate_failures() {
    support::local(async {
        notification(from_json::<SessionStartEvent>(json!({"reason":"startup"}))).await;
        notification(from_json::<SessionCompactEvent>(json!({"compactionEntry":{"type":"compaction","id":"compact","parentId":null,"timestamp":"2026-10-17T00:00:00Z","summary":"summary","firstKeptEntryId":"kept","tokensBefore":2.5},"fromExtension":false}))).await;
        notification(from_json::<SessionShutdownEvent>(json!({"reason":"quit"}))).await;
        notification(from_json::<SessionTreeEvent>(
            json!({"newLeafId":"new","oldLeafId":null}),
        ))
        .await;
        notification(AfterProviderResponseEvent {
            status: 200.0,
            headers: std::collections::BTreeMap::new(),
        })
        .await;
        notification(AgentStartEvent {}).await;
        message_and_tool_notifications().await;
    });
}

#[test]
fn provider_response_handlers_finish_before_body_consumption() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("response", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let (release, waiting) = tokio::sync::oneshot::channel();
        let waiting = RefCell::new(Some(waiting));
        loaded
            .api
            .on::<AfterProviderResponseEvent, ()>(
                "after_provider_response".into(),
                Rc::new(move |_, _| {
                    let waiting = waiting.borrow_mut().take().unwrap();
                    Box::pin(async move {
                        waiting.await.unwrap();
                        Ok(None)
                    })
                }),
            )
            .unwrap();
        let consumed = std::cell::Cell::new(false);
        let mut event = AfterProviderResponseEvent {
            status: 200.0,
            headers: std::collections::BTreeMap::new(),
        };
        let operation = async {
            runner.emit(&mut event).await.unwrap();
            consumed.set(true);
        };
        let mut operation = std::pin::pin!(operation);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(operation.as_mut(), &mut context).is_pending());
        assert!(!consumed.get());
        release.send(()).unwrap();
        operation.await;
        assert!(consumed.get());
        bus.settle().await;
    });
}

#[test]
fn failing_error_listener_propagates_and_stops_later_hooks() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("failure", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let trace = support::Trace::default();
        let first = trace.clone();
        let _remove = runner.on_error(Rc::new(move |_| {
            support::mark(&first, "listener");
            Err("listener failure".into())
        }));
        let second = trace.clone();
        let _remove2 = runner.on_error(Rc::new(move |_| {
            support::mark(&second, "later listener");
            Ok(())
        }));
        loaded
            .api
            .on::<AgentStartEvent, ()>(
                "agent_start".into(),
                Rc::new(|_, _| Box::pin(async { Err("hook failure".into()) })),
            )
            .unwrap();
        let later = trace.clone();
        loaded
            .api
            .on::<AgentStartEvent, ()>(
                "agent_start".into(),
                Rc::new(move |_, _| {
                    support::mark(&later, "later hook");
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
        assert_eq!(
            runner
                .emit(&mut AgentStartEvent {})
                .await
                .unwrap_err()
                .to_string(),
            "listener failure"
        );
        assert_eq!(*trace.borrow(), ["listener"]);
        bus.settle().await;
    });
}

#[test]
fn shutdown_helper_reports_handler_presence_and_keeps_error_isolation() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("shutdown", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let mut event: SessionShutdownEvent = from_json(json!({"reason":"reload"}));
        assert!(
            !maestro_extensions::runner::emit_session_shutdown_event(&runner, &mut event)
                .await
                .unwrap()
        );
        let trace = support::Trace::default();
        loaded
            .api
            .on::<SessionShutdownEvent, ()>(
                "session_shutdown".into(),
                Rc::new(|_, _| Box::pin(async { Err("shutdown failure".into()) })),
            )
            .unwrap();
        let reached = trace.clone();
        loaded
            .api
            .on::<SessionShutdownEvent, ()>(
                "session_shutdown".into(),
                Rc::new(move |_, _| {
                    let reached = reached.clone();
                    Box::pin(async move {
                        tokio::task::yield_now().await;
                        support::mark(&reached, "finished");
                        Ok(None)
                    })
                }),
            )
            .unwrap();
        assert!(
            maestro_extensions::runner::emit_session_shutdown_event(&runner, &mut event)
                .await
                .unwrap()
        );
        assert_eq!(*trace.borrow(), ["finished"]);
        assert!(runner.has_handlers("session_shutdown"));
        bus.settle().await;
    });
}

async fn session_before<V>(mut event: V, complete: Value)
where
    V: RunnerEmitEvent,
    V::Result: serde::Serialize + serde::de::DeserializeOwned,
{
    for (answers, expected, visits) in [
        (vec![None], None, 1),
        (vec![None, Some(json!({}))], Some(json!({})), 2),
        (
            vec![
                Some(json!({"cancel":false})),
                Some(complete.clone()),
                Some(json!("error")),
            ],
            Some(complete.clone()),
            3,
        ),
        (
            vec![
                Some(complete),
                Some(json!({"cancel":true})),
                Some(json!({"cancel":false})),
            ],
            Some(json!({"cancel":true})),
            2,
        ),
    ] {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("before", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let called = Rc::new(std::cell::Cell::new(0));
        for answer in answers {
            let called = called.clone();
            loaded
                .api
                .on::<V, V::Result>(
                    V::EVENT_TYPE.into(),
                    Rc::new(move |_, _| {
                        called.set(called.get() + 1);
                        Box::pin(std::future::ready(session_answer(answer.clone())))
                    }),
                )
                .unwrap();
        }
        let result = runner
            .emit(&mut event)
            .await
            .unwrap()
            .map(|value| serde_json::to_value(value).unwrap());
        assert_eq!(result, expected, "{}", V::EVENT_TYPE);
        assert_eq!(called.get(), visits, "{}", V::EVENT_TYPE);
        bus.settle().await;
    }
}

fn compact_event() -> SessionBeforeCompactEvent {
    SessionBeforeCompactEvent {
        data: from_json(
            json!({"preparation":{"firstKeptEntryId":"kept","messagesToSummarize":[{"role":"custom","content":"message"}],"turnPrefixMessages":[{"role":"user","content":"prefix"}],"isSplitTurn":false,"tokensBefore":-1.5,"previousSummary":"previous","fileOps":{"read":["r"],"written":["w"],"edited":["e"]},"settings":{"enabled":false,"reserveTokens":2.5,"keepRecentTokens":3.5}},"branchEntries":[{"id":"branch","parentId":"parent"}],"customInstructions":"instructions"}),
        ),
        signal: maestro_models::Cancellation::new(),
    }
}
fn tree_event() -> SessionBeforeTreeEvent {
    SessionBeforeTreeEvent {
        preparation: from_json(
            json!({"targetId":"target","oldLeafId":"old","commonAncestorId":"common","entriesToSummarize":[{"id":"entry"}],"userWantsSummary":true,"customInstructions":"instructions","replaceInstructions":false,"label":"label"}),
        ),
        signal: maestro_models::Cancellation::new(),
    }
}

#[test]
fn session_before_events_keep_last_result_and_stop_at_cancel() {
    support::local(async {
        session_before(
            from_json::<SessionBeforeSwitchEvent>(
                json!({"reason":"resume","targetSessionFile":"target"}),
            ),
            json!({"cancel":false}),
        )
        .await;
        session_before(
            from_json::<SessionBeforeForkEvent>(json!({"entryId":"entry","position":"before"})),
            json!({"cancel":false,"skipConversationRestore":true}),
        )
        .await;
        session_before(compact_event(), json!({"cancel":false,"compaction":{"summary":"summary","firstKeptEntryId":"kept","tokensBefore":2.5,"details":false}})).await;
        session_before(tree_event(), json!({"cancel":false,"summary":{"summary":"summary","details":false},"customInstructions":"","replaceInstructions":false,"label":"label"})).await;
    });
}

fn message_update_event() -> MessageUpdateEvent<Value> {
    from_json::<MessageUpdateEvent<Value>>(
        json!({"message":false,"assistantMessageEvent":{"type":"start","partial":{"role":"assistant","content":[],"api":"fixture","provider":"fixture","model":"fixture","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":0}}}),
    )
}

#[path = "support/hooks.rs"]
mod hooks;
#[test]
fn hook_registration_during_await_runs_in_current_dispatch() {
    support::local(async {
        hooks::local::all_routes(hooks::Probe::Live).await;
        hooks::threaded::all_routes(hooks::Probe::Live).await;
    });
}
#[test]
fn both_callback_families_use_one_registered_handler_map() {
    support::local(async {
        hooks::local::exercise::<AgentStartEvent, ()>(
            "agent_start",
            AgentStartEvent {},
            |runner, event| Box::pin(async move { runner.emit(event).await.map(|_| ()) }),
            true,
        )
        .await;
        hooks::threaded::exercise::<AgentStartEvent, ()>(
            "agent_start",
            AgentStartEvent {},
            |runner, event| Box::pin(async move { runner.emit(event).await.map(|_| ()) }),
            true,
        )
        .await;
    });
}

#[test]
fn hook_errors_keep_source_message_and_owned_stack() {
    support::local(async {
        hooks::local::all_routes(hooks::Probe::Errors).await;
    });
}

#[test]
fn session_before_events_keep_preparation_and_original_cancellation() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("preparation", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let reached = support::Trace::default();
        register_simple_preparations(&loaded, &reached);
        let mut switch = from_json::<SessionBeforeSwitchEvent>(
            json!({"reason":"resume","targetSessionFile":"target"}),
        );
        runner.emit(&mut switch).await.unwrap();
        runner
            .emit(&mut from_json::<SessionBeforeForkEvent>(
                json!({"entryId":"entry","position":"before"}),
            ))
            .await
            .unwrap();
        let mut compact = compact_event();
        let original_compact = compact.signal.clone();
        let mut tree = tree_event();
        let original_tree = tree.signal.clone();
        register_summary_preparations(&loaded, &reached);
        assert!(runner.emit(&mut compact).await.unwrap().is_none());
        assert!(runner.emit(&mut tree).await.unwrap().is_none());
        assert!(original_compact.is_aborted());
        assert!(original_tree.is_aborted());
        assert_eq!(
            *reached.borrow(),
            [
                "switch", "switch", "fork", "fork", "compact", "compact", "tree", "tree"
            ]
        );
        bus.settle().await;
    });
}
fn bind_model(runner: &ExtensionRunner<maestro_models::Local>, name: &str) {
    let model = model_support::model(name);
    let prompt = name.to_owned();
    runner
        .bind_core(
            maestro_models::Local::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(model.clone()))),
                get_system_prompt: Rc::new(move || Ok(prompt.clone())),
                ..Default::default()
            },
            types::ProviderActions::default(),
        )
        .unwrap();
}
#[test]
fn dispatch_context_is_fresh_and_reads_current_binding() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("context", bus.controller.for_extension());
        let runner = Rc::new(support::runner(&loaded.owners));
        bind_model(&runner, "first");
        let retained = Rc::new(RefCell::new(Vec::new()));
        register_context_rebinding(&loaded, &runner, &retained);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let observed = seen.clone();
        loaded
            .api
            .on::<AgentStartEvent, ()>(
                "agent_start".into(),
                Rc::new(move |_, context| {
                    observed
                        .borrow_mut()
                        .push(context.model().unwrap().unwrap().read().unwrap().id.clone());
                    assert_eq!(context.get_system_prompt().unwrap(), "second");
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
        runner.emit(&mut AgentStartEvent {}).await.unwrap();
        runner.emit(&mut AgentStartEvent {}).await.unwrap();
        assert_eq!(*seen.borrow(), ["first", "second"]);
        assert_eq!(
            retained.borrow()[0]
                .model()
                .unwrap()
                .unwrap()
                .read()
                .unwrap()
                .id,
            "first"
        );
        assert_eq!(
            retained.borrow()[1]
                .model()
                .unwrap()
                .unwrap()
                .read()
                .unwrap()
                .id,
            "second"
        );
        assert_eq!(retained.borrow()[0].get_system_prompt().unwrap(), "second");
        runner.invalidate(Some("stale".into()));
        assert_eq!(
            retained.borrow()[0]
                .get_system_prompt()
                .unwrap_err()
                .to_string(),
            "stale"
        );
        bus.settle().await;
    });
}

async fn message_and_tool_notifications() {
    notification(AgentEndEvent::<Value> {
        messages: vec![json!({"role":"user"})],
    })
    .await;
    notification(TurnStartEvent {
        turn_index: 2.0,
        timestamp: 3.0,
    })
    .await;
    notification(from_json::<TurnEndEvent<Value>>(
        json!({"turnIndex":2,"message":false,"toolResults":[]}),
    ))
    .await;
    notification(MessageStartEvent {
        message: json!(false),
    })
    .await;
    notification(message_update_event()).await;
    notification(ToolExecutionStartEvent {
        tool_call_id: "id".into(),
        tool_name: "tool".into(),
        args: json!({}),
    })
    .await;
    notification(ToolExecutionUpdateEvent {
        tool_call_id: "id".into(),
        tool_name: "tool".into(),
        args: json!({}),
        partial_result: json!(false),
    })
    .await;
    notification(ToolExecutionEndEvent {
        tool_call_id: "id".into(),
        tool_name: "tool".into(),
        result: json!(false),
        is_error: false,
    })
    .await;
    notification(ModelSelectEvent {
        model: model_support::model("selected"),
        previous_model: Presence::Missing,
        source: ModelSelectSource::Set,
    })
    .await;
    notification(from_json::<ThinkingLevelSelectEvent>(
        json!({"level":"high","previousLevel":"off"}),
    ))
    .await;
}

fn session_answer<R: serde::de::DeserializeOwned>(
    answer: Option<Value>,
) -> Result<Option<R>, EventError> {
    match answer {
        Some(value) if value == "error" => Err("later failure".into()),
        Some(value) => Ok(Some(from_json(value))),
        None => Ok(None),
    }
}

fn register_simple_preparations(loaded: &support::Loaded, reached: &support::Trace) {
    for _ in 0..2 {
        let trace = reached.clone();
        loaded.api.on::<SessionBeforeSwitchEvent,SessionBeforeSwitchResult>("session_before_switch".into(),Rc::new(move |event,_| {
                assert_eq!(event.reason,SessionBeforeSwitchReason::Resume);
                assert!(matches!(&event.target_session_file,Presence::Present(path) if path == "target"));
                support::mark(&trace,"switch");Box::pin(std::future::ready(Ok(None)))
            })).unwrap();
        let trace = reached.clone();
        loaded
            .api
            .on::<SessionBeforeForkEvent, SessionBeforeForkResult>(
                "session_before_fork".into(),
                Rc::new(move |event, _| {
                    assert_eq!(event.entry_id, "entry");
                    assert!(matches!(event.position, ForkPosition::Before));
                    support::mark(&trace, "fork");
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
    }
}
fn register_summary_preparations(loaded: &support::Loaded, reached: &support::Trace) {
    for index in 0..2 {
        let trace = reached.clone();
        loaded.api.on::<SessionBeforeCompactEvent,SessionBeforeCompactResult>("session_before_compact".into(),Rc::new(move |event,_| {
                assert_eq!(event.preparation.first_kept_entry_id,"kept");
                assert_eq!(event.preparation.messages_to_summarize[0]["content"],"message");
                assert_eq!(event.branch_entries[0]["parentId"],"parent");
                assert!(matches!(&event.custom_instructions,Presence::Present(text) if text == "instructions"));
                assert_eq!(event.signal.is_aborted(), index == 1); event.signal.abort();
                support::mark(&trace,"compact");Box::pin(std::future::ready(Ok(None)))
            })).unwrap();
        let trace = reached.clone();
        loaded.api.on::<SessionBeforeTreeEvent,SessionBeforeTreeResult>("session_before_tree".into(),Rc::new(move |event,_| {
                assert_eq!(event.preparation.target_id,"target");assert_eq!(event.preparation.old_leaf_id.as_deref(),Some("old"));
                assert_eq!(event.preparation.common_ancestor_id.as_deref(),Some("common"));
                assert_eq!(event.preparation.entries_to_summarize[0]["id"],"entry");assert!(event.preparation.user_wants_summary);
                assert!(matches!(&event.preparation.custom_instructions,Presence::Present(text) if text == "instructions"));
                assert!(matches!(&event.preparation.replace_instructions,Presence::Present(false)));
                assert!(matches!(&event.preparation.label,Presence::Present(text) if text == "label"));
                assert_eq!(event.signal.is_aborted(),index == 1);event.signal.abort();
                support::mark(&trace,"tree");Box::pin(std::future::ready(Ok(None)))
            })).unwrap();
    }
}

fn register_context_rebinding(
    loaded: &support::Loaded,
    runner: &Rc<ExtensionRunner<maestro_models::Local>>,
    retained: &Rc<RefCell<Vec<ExtensionContext>>>,
) {
    let contexts = retained.clone();
    let weak = Rc::downgrade(runner);
    loaded
        .api
        .on::<AgentStartEvent, ()>(
            "agent_start".into(),
            Rc::new(move |_, context| {
                contexts.borrow_mut().push(context);
                bind_model(&weak.upgrade().unwrap(), "second");
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
}

#[test]
fn error_listener_removers_follow_listener_identity() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("identity", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let calls = Rc::new(std::cell::Cell::new(0));
        let seen = Rc::clone(&calls);
        let listener: ExtensionErrorListener = Rc::new(move |_| {
            seen.set(seen.get() + 1);
            Ok(())
        });
        let error = ExtensionError {
            extension_path: "identity".into(),
            event: "agent_start".into(),
            error: "failure".into(),
            stack: None,
        };
        let first = runner.on_error(Rc::clone(&listener));
        let second = runner.on_error(Rc::clone(&listener));
        second();
        runner.emit_error(&error).unwrap();
        assert_eq!(calls.get(), 0, "the later remover deletes the listener");
        let _again = runner.on_error(Rc::clone(&listener));
        first();
        runner.emit_error(&error).unwrap();
        assert_eq!(
            calls.get(),
            0,
            "an earlier remover deletes the re-registration"
        );
        bus.settle().await;
    });
}
