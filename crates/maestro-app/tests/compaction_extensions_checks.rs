//! Extension compaction decisions preserve preparation, stored results and notification order.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::{assert_prompts, assert_result, messages, seeded};
use crate::support::session_harness::{
    Harness, HarnessOptions, answer, on_task_set, recorded, script_responses,
};
use maestro_extensions::{
    ExtensionFactory, SessionBeforeCompactEvent, SessionBeforeCompactEventData,
    SessionBeforeCompactResult, SessionCompactEvent,
};
use maestro_models::{Cancellation, Local};
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_session::{CompactionResult, Presence};
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

/// Supplies a summary with an overridden token estimate and caller-authored details.
fn supplied(details: Presence<Value>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let details = details.clone();
        drop(
            api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
                "session_before_compact".into(),
                Rc::new(move |event, _| {
                    Box::pin(std::future::ready(Ok(Some(SessionBeforeCompactResult {
                        cancel: Presence::Missing,
                        compaction: Presence::Present(CompactionResult {
                            summary: "summary from extension".into(),
                            first_kept_entry_id: event.data.preparation.first_kept_entry_id.clone(),
                            tokens_before: 999.0,
                            details: details.clone(),
                        }),
                    }))))
                }),
            ),
        );
        Box::pin(std::future::ready(Ok(())))
    })
}

/// Refuses its before event with an attributed extension error.
fn throwing() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        drop(
            api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
                "session_before_compact".into(),
                Rc::new(|_, _| {
                    Box::pin(std::future::ready(Err(
                        "Extension intentionally throws".into()
                    )))
                }),
            ),
        );
        Box::pin(std::future::ready(Ok(())))
    })
}

/// Records one extension's callbacks without changing compaction.
fn ordered(label: &'static str, order: Rc<RefCell<Vec<String>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let before = Rc::clone(&order);
        drop(
            api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
                "session_before_compact".into(),
                Rc::new(move |_, _| {
                    before.borrow_mut().push(format!("{label}-before"));
                    Box::pin(std::future::ready(Ok(None)))
                }),
            ),
        );
        let after = Rc::clone(&order);
        drop(api.on::<SessionCompactEvent, ()>(
            "session_compact".into(),
            Rc::new(move |_, _| {
                after.borrow_mut().push(format!("{label}-after"));
                Box::pin(std::future::ready(Ok(None)))
            }),
        ));
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn compaction_handlers_keep_registration_order() {
    on_task_set(|_| async {
        let order = Rc::new(RefCell::new(Vec::new()));
        let h = seeded(HarnessOptions {
            factories: vec![
                ordered("1", Rc::clone(&order)),
                ordered("2", Rc::clone(&order)),
            ],
            ..HarnessOptions::default()
        })
        .await;
        drop(script_responses(
            &h,
            vec![answer("## Goal\nControlled summary", None, None); 2],
        ));

        drop(h.session.compact(None).await.unwrap());

        assert_eq!(
            *order.borrow(),
            ["1-before", "2-before", "1-after", "2-after"]
        );
    });
}

#[test]
fn throwing_compaction_hook_falls_back_to_the_summarizer() {
    on_task_set(|_| async {
        let observed = Rc::new(RefCell::new(Observations::default()));
        let h = seeded(HarnessOptions {
            factories: vec![observing(false, Rc::clone(&observed))],
            labelled_factories: vec![("throwing-extension", throwing())],
            ..HarnessOptions::default()
        })
        .await;
        let errors = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&errors);
        let _unsubscribe = h.runner().on_error(Rc::new(move |error| {
            captured.borrow_mut().push(error.clone());
            Ok(())
        }));
        let requested = script_responses(
            &h,
            vec![answer("## Goal\nControlled summary", None, None); 2],
        );

        let result = h.session.compact(None).await.unwrap();

        let expected = recorded("compaction_sessions", "manual");
        assert_result(&h, &result, &expected["result"]);
        assert_prompts(&requested.lock().unwrap(), &expected);
        assert_eq!(observed.borrow().order, ["before", "after"]);
        assert_eq!(observed.borrow().after.len(), 1);
        assert!(!observed.borrow().after[0].from_extension);
        let errors = errors.borrow();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].event, "session_before_compact");
        assert_eq!(errors[0].error, "Extension intentionally throws");
        assert_eq!(errors[0].extension_path, "throwing-extension");
    });
}

/// Checks saved extension values without collapsing absent details into null.
fn assert_saved_result(h: &Harness, result: &CompactionResult, expected: Option<&Value>) {
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    let entries = manager.get_entries();
    let compacted: Vec<_> = entries
        .iter()
        .filter(|entry| entry["type"] == "compaction")
        .collect();
    assert_eq!(compacted.len(), 1);
    assert_eq!(compacted[0]["summary"], result.summary);
    assert_eq!(compacted[0]["firstKeptEntryId"], result.first_kept_entry_id);
    assert_eq!(
        compacted[0]["tokensBefore"].as_f64().unwrap().to_bits(),
        999.0_f64.to_bits()
    );
    assert_eq!(compacted[0].get("details"), expected);
}

#[test]
fn extension_compaction_keeps_returned_values() {
    on_task_set(|_| async {
        for expected in [
            None,
            Some(Value::Null),
            Some(json!("text")),
            Some(json!(7)),
            Some(json!(false)),
            Some(json!({"source": "extension"})),
            Some(json!(["value"])),
        ] {
            let details = match expected.clone() {
                None => Presence::Missing,
                Some(Value::Null) => Presence::Null,
                Some(value) => Presence::Present(value),
            };
            let observed = Rc::new(RefCell::new(Observations::default()));
            let h = seeded(HarnessOptions {
                factories: vec![observing(false, Rc::clone(&observed)), supplied(details)],
                ..HarnessOptions::default()
            })
            .await;
            let before = messages(&h);
            let requested = script_responses(&h, vec![answer("must not be requested", None, None)]);

            let result = h.session.compact(None).await.unwrap();

            assert_eq!(result.summary, "summary from extension");
            assert_eq!(result.tokens_before.to_bits(), 999.0_f64.to_bits());
            assert_eq!(
                result.first_kept_entry_id,
                observed.borrow().before[0]
                    .0
                    .preparation
                    .first_kept_entry_id
            );
            assert!(requested.lock().unwrap().is_empty());
            assert_eq!(
                serde_json::to_value(&result).unwrap().get("details"),
                expected.as_ref()
            );
            assert_saved_result(&h, &result, expected.as_ref());
            let after = messages(&h);
            assert_eq!(after[0]["summary"], "summary from extension");
            assert_eq!(after[1..], before[3..]);
            assert_eq!(observed.borrow().after.len(), 1);
            assert!(observed.borrow().after[0].from_extension);
        }
    });
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
