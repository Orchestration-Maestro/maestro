//! Extension compaction decisions preserve preparation, stored results and notification order.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::{messages, seeded};
use crate::support::session_harness::{HarnessOptions, answer, on_task_set, script_responses};
use maestro_extensions::{
    ExtensionFactory, SessionBeforeCompactEvent, SessionBeforeCompactEventData,
    SessionBeforeCompactResult, SessionCompactEvent,
};
use maestro_models::{Cancellation, Local};
use maestro_session::Presence;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Preparation, retained signals and saved notifications observed by one extension.
#[derive(Default)]
struct Observations {
    /// Before events with the live cancellation signal retained independently.
    before: Vec<(SessionBeforeCompactEventData, Cancellation)>,
    /// Compact events whose entries were already readable during notification.
    after: Vec<SessionCompactEvent>,
    /// Delivered extension phases in arrival order.
    order: Vec<&'static str>,
}

/// Records preparation and verifies the announced entry is already saved.
fn observing(cancel: bool, observed: Rc<RefCell<Observations>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let before = Rc::clone(&observed);
        drop(
            api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
                "session_before_compact".into(),
                Rc::new(move |event, ctx| {
                    assert!(ctx.model_registry().is_ok());
                    assert_eq!(ctx.model().unwrap().unwrap().read().unwrap().id, "plain");
                    let manager = ctx.session_manager().unwrap();
                    let manager = manager.borrow();
                    assert_eq!(
                        event.data.branch_entries,
                        manager
                            .get_entries()
                            .into_iter()
                            .cloned()
                            .collect::<Vec<_>>()
                    );
                    let mut observed = before.borrow_mut();
                    observed.order.push("before");
                    observed
                        .before
                        .push((event.data.clone(), event.signal.clone()));
                    Box::pin(std::future::ready(Ok(Some(SessionBeforeCompactResult {
                        cancel: Presence::Present(cancel),
                        compaction: Presence::Missing,
                    }))))
                }),
            ),
        );
        let after = Rc::clone(&observed);
        drop(api.on::<SessionCompactEvent, ()>(
            "session_compact".into(),
            Rc::new(move |event, ctx| {
                let manager = ctx.session_manager().unwrap();
                let manager = manager.borrow();
                let saved = manager.get_entry(&event.compaction_entry.base.id).unwrap();
                assert_eq!(
                    *saved,
                    serde_json::to_value(&event.compaction_entry).unwrap()
                );
                let mut observed = after.borrow_mut();
                observed.order.push("after");
                observed.after.push(event.clone());
                Box::pin(std::future::ready(Ok(None)))
            }),
        ));
        Box::pin(std::future::ready(Ok(())))
    })
}

/// Checks the prepared split against the completed source turns.
fn assert_preparation(observed: &Observations, before: &[Value]) {
    assert_eq!(observed.before.len(), 1);
    let (data, signal) = &observed.before[0];
    assert!(!signal.is_aborted());
    assert!(data.custom_instructions.is_missing());
    let preparation = &data.preparation;
    assert_eq!(preparation.messages_to_summarize, before[..2]);
    assert_eq!(preparation.turn_prefix_messages, before[2..3]);
    assert!(preparation.is_split_turn);
    assert_eq!(preparation.tokens_before.to_bits(), 150.0_f64.to_bits());
    assert_eq!(
        preparation.settings.keep_recent_tokens.to_bits(),
        1.0_f64.to_bits()
    );
    assert_eq!(
        preparation.settings.reserve_tokens.to_bits(),
        16_384.0_f64.to_bits()
    );
    assert!(preparation.settings.enabled);
    assert_eq!(
        preparation.first_kept_entry_id,
        data.branch_entries[3]["id"]
    );
}

#[test]
fn compaction_hooks_observe_preparation_then_saved_entry() {
    on_task_set(|_| async {
        for cancel in [false, true] {
            let observed = Rc::new(RefCell::new(Observations::default()));
            let h = seeded(HarnessOptions {
                factories: vec![observing(cancel, Rc::clone(&observed))],
                ..HarnessOptions::default()
            })
            .await;
            let before = messages(&h);
            let requested = script_responses(
                &h,
                vec![answer("## Goal\nControlled summary", None, None); 2],
            );

            let result = h.session.compact(None).await;

            let observed = observed.borrow();
            assert_preparation(&observed, &before);
            if cancel {
                assert_eq!(result.unwrap_err().message, "Compaction cancelled");
                assert!(requested.lock().unwrap().is_empty());
                assert_eq!(messages(&h), before);
                assert_eq!(observed.order, ["before"]);
                assert!(observed.after.is_empty());
                assert_eq!(h.session.session_manager().borrow().get_entries().len(), 4);
            } else {
                let result = result.unwrap();
                assert_eq!(requested.lock().unwrap().len(), 2);
                assert_eq!(observed.order, ["before", "after"]);
                assert_eq!(observed.after.len(), 1);
                assert!(!observed.after[0].from_extension);
                assert_eq!(observed.after[0].compaction_entry.summary, result.summary);
                assert_eq!(
                    json!(observed.after[0].compaction_entry.details),
                    json!({"readFiles": [], "modifiedFiles": []})
                );
            }
        }
    });
}
