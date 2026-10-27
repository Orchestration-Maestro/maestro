//! Stored session names and their published events.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::session_harness::{HarnessOptions, corpus, harness, on_task_set};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::json;

#[test]
fn name_events_report_the_stored_trimmed_name() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        for case in expected["names"].as_array().unwrap() {
            h.events.borrow_mut().clear();
            h.session
                .set_session_name(case["input"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                json!(
                    h.session
                        .session_manager()
                        .borrow()
                        .get_session_name()
                        .unwrap()
                ),
                case["name"]
            );
            assert_eq!(*h.events.borrow(), [case["event"].clone()]);
        }
    });
}
