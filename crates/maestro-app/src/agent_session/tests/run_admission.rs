//! A refused start leaves the session's existing run in charge of settlement.
use crate::tests::session_harness::{
    HarnessOptions, Scripted, harness, on_task_set, retry_started, start_prompt,
};
use maestro_agent::PromptInput;
use serde_json::json;

/// A retry wait leaves the low-level agent idle but still owns the session run.
#[test]
fn second_run_during_retry_is_refused_without_touching_active_run() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            responses: Some(vec![
                Scripted::failure("overloaded_error"),
                Scripted::text("unwanted"),
            ]),
            settings: json!({"retry": {"baseDelayMs": 60_000}}),
            ..HarnessOptions::default()
        })
        .await;
        let started = retry_started(&h.session);
        let first = start_prompt(&h, "first");
        started.notified().await;
        h.session.state.borrow_mut().system_prompt_override = Some("active override".into());
        let mut idle = std::pin::pin!(h.session.wait_for_idle());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(idle.as_mut(), &mut context).is_pending());

        let refused = h.session.run_agent(PromptInput::Messages(vec![])).await;
        let active = h.session.is_streaming();
        let prompt = h.session.state.borrow().system_prompt_override.clone();
        let pending = std::future::Future::poll(idle.as_mut(), &mut context).is_pending();
        h.session.abort_retry();
        first.await.unwrap().unwrap();
        idle.await;

        assert!(refused.is_err(), "second run was accepted");
        assert!(refused.unwrap_err().message.contains("already processing"));
        assert!(active);
        assert_eq!(prompt.as_deref(), Some("active override"));
        assert!(pending, "the original idle wait still belongs to its run");
        assert_eq!(h.requests.0.borrow().len(), 1, "no second agent prompt");
        assert!(h.session.is_idle());
    });
}
