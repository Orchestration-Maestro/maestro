//! Threshold compaction resumes an agent-only queue through the actual provider context.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::{messages, seeded};
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, delivery_requests, history_view, of_kinds, on_task_set,
    prompt_to_end, recorded, summary_provider,
};
use maestro_agent::{AgentEvent, AgentMessage};
use maestro_app::AgentSessionEvent;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

/// Adds a custom follow-up when the large response's agent run ends, before compaction.
fn queue_after_response(h: &Harness, queued: Rc<Cell<bool>>) {
    let session = Rc::downgrade(&h.session);
    drop(h.session.subscribe(Rc::new(move |event| {
        let AgentSessionEvent::Agent(event) = event else { return; };
        let is_large = {
            let event = event.read().unwrap();
            let AgentEvent::AgentEnd { messages } = &*event else { return; };
            let Some(AgentMessage::Assistant(message)) = messages.last() else { return; };
            message.read().unwrap().usage.input.to_bits() == 199_500.0_f64.to_bits()
        };
        if !is_large { return; }
        let session = session.upgrade().unwrap();
        session.agent().follow_up(serde_json::from_value(json!({
            "role": "custom", "customType": "test", "content": [{"type": "text", "text": "queued custom"}],
            "display": false, "timestamp": 1_700_000_000_500_f64
        })).unwrap());
        assert_eq!(session.pending_message_count(), 0);
        assert!(session.agent().has_queued_messages());
        queued.set(true);
    })));
}

/// Checks the continuation's context, final message order and both empty queue mirrors.
fn assert_queue_drained(h: &Harness, expected: &Value) {
    let requests = delivery_requests(h);
    assert_eq!(
        &requests.as_array().unwrap()[2..],
        expected["requests"].as_array().unwrap()
    );
    assert_eq!(h.requests.0.borrow().len(), 4);
    let expected_messages: Vec<_> = expected["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| json!({"role": message["role"], "text": message["text"]}))
        .collect();
    assert_eq!(history_view(h), expected_messages);
    let live = messages(h);
    assert_eq!(live[1]["stopReason"], "stop");
    assert_eq!(live[3]["stopReason"], "stop");
    assert_eq!(h.session.pending_message_count(), 0);
    assert!(!h.session.agent().has_queued_messages());
    assert!(h.session.get_steering_messages().is_empty());
    assert!(h.session.get_follow_up_messages().is_empty());
}

/// Checks the threshold compaction's ordered events and selected boundary.
fn assert_queue_compaction(h: &Harness) {
    let events = of_kinds(&h.events.borrow(), &["compaction_start", "compaction_end"]);
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        json!({"type": "compaction_start", "reason": "threshold"})
    );
    let end = &events[1];
    assert_eq!(end["type"], "compaction_end");
    assert_eq!(end["reason"], "threshold");
    assert_eq!(end["aborted"], false);
    assert_eq!(end["willRetry"], false);
    assert_eq!(end["result"]["summary"], "public auto summary");
    assert_eq!(end["result"]["tokensBefore"], 199_500);
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    assert_eq!(
        end["result"]["firstKeptEntryId"],
        manager.get_entries()[5]["id"]
    );
}

#[test]
fn public_threshold_compaction_drains_the_agent_only_queue() {
    on_task_set(|_| async {
        let h = seeded(HarnessOptions {
            factories: vec![summary_provider("public auto summary")],
            responses: Some(vec![
                Scripted::text("one"),
                Scripted::text("two"),
                Scripted {
                    usage: Some((199_500.0, 0.0)),
                    ..Scripted::text("large response")
                },
                Scripted::text("queue consumed"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let queued = Rc::new(Cell::new(false));
        queue_after_response(&h, Rc::clone(&queued));

        prompt_to_end(
            &h,
            "trigger threshold",
            maestro_app::PromptOptions::default(),
        )
        .await;

        assert!(queued.get());
        assert_queue_drained(&h, &recorded("compaction_sessions", "public_queue"));
        assert_queue_compaction(&h);
    });
}
