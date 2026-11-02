//! Concrete native event handlers apply edits before independently decoding decisions.
use crate::lifecycle::replies::Replies;
use crate::read_imports::ReadEndpoint;
use crate::{
    event_registration,
    guest_calls::{
        Call, Callbacks, Handle,
        events::{Decision, Outcome},
    },
    local_resources::Pump,
};
use maestro_extensions::{ExtensionExecution, Local, ToolCallEvent, ToolCallEventResult};

#[test]
fn edited_event_failure_discards_a_valid_decision() {
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    for edit in [Err("bad edit".into()), Ok(Some("{\"toolName\":7}".into()))] {
        let mut event = tool_event();
        let outcome = Outcome {
            event: edit,
            decision: Decision::Returned(Ok(Some("{\"block\":true}".into()))),
            operations: None,
        };
        let result =
            Handle::apply_outcome::<_, ToolCallEventResult>(&mut event, outcome, pump.registry());
        assert!(result.is_err());
        assert_eq!(event.tool_name, "original");
    }
}

#[test]
fn valid_edits_survive_failed_decisions_and_valid_decisions_are_retained() {
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    for (decision, expected) in [
        (Decision::Failed("handler failed".into()), None),
        (Decision::Returned(Err("bad decision".into())), None),
        (
            Decision::Returned(Ok(Some("{\"block\":true}".into()))),
            Some(true),
        ),
    ] {
        let mut event = tool_event();
        let mut edited = serde_json::to_value(&event).unwrap();
        edited["toolName"] = "edited".into();
        let outcome = Outcome {
            event: Ok(Some(edited.to_string())),
            decision,
            operations: None,
        };
        let result =
            Handle::apply_outcome::<_, ToolCallEventResult>(&mut event, outcome, pump.registry());
        assert_eq!(event.tool_name, "edited");
        match expected {
            None => assert!(result.is_err()),
            Some(block) => assert_eq!(
                result.unwrap().unwrap().block,
                maestro_extensions::Presence::Present(block)
            ),
        }
    }
}

#[test]
fn registered_handler_uses_actual_native_types_and_original_context() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (owner, api) =
                super::api::api(maestro_extensions::create_extension_runtime::<Local>());
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let callback = callbacks.allocate().unwrap();
            let handler = callbacks.retain(callback.id, admission.clone()).unwrap();
            event_registration::register(&api, "tool_call".into(), handler, pump.registry())
                .unwrap();
            let callback = Local::recover_handler::<ToolCallEvent, ToolCallEventResult>(
                &owner.read().unwrap().handlers["tool_call"][0],
            )
            .unwrap();
            let worker = std::thread::spawn(move || {
                let Call::Event(call) = incoming.blocking_recv().unwrap() else {
                    panic!("event expected")
                };
                let data: serde_json::Value = serde_json::from_str(&call.event).unwrap();
                assert_eq!(data["type"], "tool_call");
                assert_eq!(data["toolName"], "original");
                assert_eq!(
                    call.context
                        .read(crate::context_reads::Request::Cwd)
                        .unwrap(),
                    "/original"
                );
                assert!(call.signal.is_none());
                call.reply
                    .send(Ok(Outcome {
                        event: Ok(None),
                        decision: Decision::Returned(Ok(Some("{\"block\":false}".into()))),
                        operations: None,
                    }))
                    .ok()
                    .unwrap();
            });
            let context = super::runner("/original").create_context();
            let mut event = tool_event();
            let client = async {
                let result = callback(&mut event, context).await.unwrap().unwrap();
                drop((owner, callback, api, admission, calls));
                result
            };
            let ((), result) = futures_util::future::join(pump.serve(), client).await;
            worker.join().unwrap();
            assert_eq!(result.block, maestro_extensions::Presence::Present(false));
        });
}

#[test]
fn failed_edit_releases_returned_operations_through_the_original_store_queue() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
    let (_, operations) = super::returned_bash::operations(&mut callbacks, admission);
    let outcome = Outcome {
        event: Err("bad edit".into()),
        decision: Decision::Returned(Ok(Some("{\"block\":true}".into()))),
        operations: Some(operations),
    };
    assert!(
        Handle::apply_outcome::<_, ToolCallEventResult>(
            &mut tool_event(),
            outcome,
            pump.registry()
        )
        .is_err()
    );
    let Call::Release(released) = incoming.try_recv().unwrap() else {
        panic!("operations release expected")
    };
    assert!(released.object);
    assert!(incoming.try_recv().is_err());
}

#[test]
fn resource_bearing_edits_keep_the_original_native_signal() {
    use crate::guest_calls::events::EventData;
    let signal = maestro_extensions::Cancellation::new();
    let mut event = maestro_extensions::SessionBeforeTreeEvent {
        preparation: serde_json::from_value(serde_json::json!({
            "targetId":"original", "oldLeafId":null, "commonAncestorId":null,
            "entriesToSummarize":[], "userWantsSummary":false,
        }))
        .unwrap(),
        signal: signal.clone(),
    };
    let data = event.encode("session_before_tree").unwrap();
    let mut data: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(data["type"], "session_before_tree");
    assert!(data.get("signal").is_none());
    data["preparation"]["targetId"] = "edited".into();
    event.apply(&data.to_string()).unwrap();
    signal.abort();
    assert!(event.signal.is_aborted());
    assert_eq!(event.preparation.target_id, "edited");
}

#[test]
fn application_message_edits_use_the_actual_native_message_specialization() {
    use crate::guest_calls::events::EventData;
    let mut event: maestro_extensions::MessageStartEvent = serde_json::from_value(serde_json::json!({
        "message":{"role":"custom", "customType":"literal", "content":"original", "display":true, "timestamp":1.5, "details":null},
    })).unwrap();
    let mut data: serde_json::Value =
        serde_json::from_str(&event.encode("message_start").unwrap()).unwrap();
    data["message"]["content"] = "edited".into();
    event.apply(&data.to_string()).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&event.encode("message_start").unwrap()).unwrap();
    assert_eq!(data["message"]["role"], "custom");
    assert_eq!(data["message"]["content"], "edited");
    assert!(data["message"]["details"].is_null());
}

/// Original concrete event payload, with no substitute application union.
fn tool_event() -> ToolCallEvent {
    serde_json::from_value(
        serde_json::json!({"toolName":"original","toolCallId":"id","input":{"value":"雪"}}),
    )
    .unwrap()
}
