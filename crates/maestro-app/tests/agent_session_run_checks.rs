//! One agent run in a complete session: prompting, retrying and compacting happen inside it,
//! and every waiter returns only after the run settles.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, custom_note, get_assistant_texts, harness, on_task_set,
    retry_started, start_prompt, summary_provider,
};
use maestro_app::AgentSessionEvent;
use maestro_session::messages::SendMessageOptions;
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

/// A session whose context window is exhausted by a 1100-token answer, with a summary
/// extension standing in for the provider's summary.
async fn compacting(responses: Vec<Scripted>, retry_delay_ms: u32) -> Harness {
    harness(HarnessOptions {
        responses: Some(responses),
        factories: vec![summary_provider("compacted")],
        context_window: Some(1_000.0),
        settings: json!({
            "retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": retry_delay_ms},
            "compaction": {"keepRecentTokens": 1, "reserveTokens": 1},
        }),
        ..HarnessOptions::default()
    })
    .await
}

/// A custom message with `text`.
#[test]
fn prompt_resolves_after_retry_and_compaction_while_streaming_throughout() {
    on_task_set(|_| async {
        let h = compacting(
            vec![
                Scripted::text("first"),
                Scripted::failure("overloaded_error"),
                Scripted {
                    usage: Some((1_000.0, 100.0)),
                    ..Scripted::text("second")
                },
            ],
            50,
        )
        .await;
        start_prompt(&h, "one").await.unwrap().unwrap();
        let seen: Rc<RefCell<Vec<(String, bool)>>> = Rc::default();
        let record = Rc::clone(&seen);
        let session = Rc::clone(&h.session);
        drop(h.session.subscribe(Rc::new(move |event| {
            let kind = match event {
                AgentSessionEvent::AutoRetryStart { .. } => "retry_start",
                AgentSessionEvent::AutoRetryEnd { .. } => "retry_end",
                AgentSessionEvent::CompactionEnd { .. } => "compaction_end",
                _ => return,
            };
            record
                .borrow_mut()
                .push((kind.into(), session.is_streaming()));
        })));
        let started = retry_started(&h.session);
        let running = start_prompt(&h, "two");
        started.notified().await;
        assert!(
            h.session.is_streaming(),
            "the retry wait belongs to the run"
        );
        running.await.unwrap().unwrap();
        let seen = seen.borrow();
        let kinds: Vec<&str> = seen.iter().map(|(kind, _)| kind.as_str()).collect();
        assert_eq!(kinds, ["retry_start", "retry_end", "compaction_end"]);
        assert!(
            seen.iter().all(|(_, streaming)| *streaming),
            "the run flag held from the retry through the compaction: {seen:?}"
        );
        assert!(!h.session.is_streaming());
    });
}

#[test]
fn prompt_continues_through_an_overflow_recovery() {
    on_task_set(|_| async {
        let h = compacting(
            vec![
                Scripted::text("first"),
                Scripted::context_overflow(),
                Scripted::text("after recovery"),
            ],
            50,
        )
        .await;
        start_prompt(&h, "one").await.unwrap().unwrap();
        start_prompt(&h, "two").await.unwrap().unwrap();
        assert_eq!(
            h.requests.0.borrow().len(),
            3,
            "the recovery sent the turn again"
        );
        assert_eq!(
            get_assistant_texts(&h).last().map(String::as_str),
            Some("after recovery")
        );
        assert!(!h.session.is_streaming());
    });
}

#[test]
fn prompt_continues_with_a_message_queued_during_compaction() {
    on_task_set(|_| async {
        let h = compacting(
            vec![
                Scripted {
                    usage: Some((1_000.0, 100.0)),
                    ..Scripted::text("first")
                },
                Scripted::text("queued answer"),
            ],
            50,
        )
        .await;
        let session = Rc::clone(&h.session);
        drop(h.session.subscribe(Rc::new(move |event| {
            if matches!(event, AgentSessionEvent::CompactionStart { .. }) {
                session.follow_up("queued", &[]).unwrap();
            }
        })));
        start_prompt(&h, "one").await.unwrap().unwrap();
        assert_eq!(
            h.requests.0.borrow().len(),
            2,
            "the queued message started another request"
        );
        assert_eq!(
            get_assistant_texts(&h).last().map(String::as_str),
            Some("queued answer"),
            "the run delivered the queued message"
        );
        assert!(!h.session.is_streaming());
    });
}

#[test]
fn abort_returns_after_the_run_settles_its_shell_results() {
    on_task_set(|_| async {
        let h = compacting(vec![Scripted::failure("overloaded_error")], 60_000).await;
        let started = retry_started(&h.session);
        let running = start_prompt(&h, "one");
        started.notified().await;
        let finished = maestro_tools::BashResult {
            output: String::new(),
            exit_code: Some(0.0),
            cancelled: false,
            truncated: false,
            full_output_path: None,
        };
        h.session
            .record_bash_result("echo", &finished, None)
            .unwrap();
        assert!(
            h.session.has_pending_bash_messages(),
            "a retry wait belongs to the run, so a shell result waits for the settle step"
        );
        h.session.abort().await;
        assert!(!h.session.is_streaming());
        assert!(!h.session.has_pending_bash_messages());
        assert!(
            running.is_finished(),
            "the prompt settled before abort returned"
        );
    });
}

#[test]
fn custom_message_during_a_triggered_run_is_steered() {
    on_task_set(|_| async {
        let h = compacting(
            vec![
                Scripted::failure("overloaded_error"),
                Scripted::text("done"),
            ],
            60_000,
        )
        .await;
        let started = retry_started(&h.session);
        let session = Rc::clone(&h.session);
        let first = tokio::task::spawn_local(async move {
            let options = SendMessageOptions {
                trigger_turn: Some(true),
                deliver_as: None,
            };
            session
                .send_custom_message(custom_note("first"), options)
                .await
        });
        started.notified().await;
        h.session
            .send_custom_message(custom_note("second"), SendMessageOptions::default())
            .await
            .unwrap();
        assert_eq!(
            h.requests.0.borrow().len(),
            1,
            "one prompt reached the provider"
        );
        assert!(
            h.session.agent().has_queued_messages(),
            "the second is queued"
        );
        h.session.abort().await;
        drop(first.await);
    });
}

#[test]
fn dispose_stops_a_waiting_retry_so_the_run_settles() {
    on_task_set(|_| async {
        let h = compacting(vec![Scripted::failure("overloaded_error")], 60_000).await;
        let started = retry_started(&h.session);
        let running = start_prompt(&h, "one");
        started.notified().await;
        h.session.dispose();
        assert!(!h.session.is_retrying(), "disposal stopped the retry wait");
        running.await.unwrap().unwrap();
        assert!(!h.session.is_streaming());
    });
}
