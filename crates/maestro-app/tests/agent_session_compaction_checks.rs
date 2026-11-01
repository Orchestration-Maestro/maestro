//! Manual compaction preserves the saved branch, live context and delivered events.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::{assert_prompts, assert_result, messages, seeded};
use crate::support::session_harness::{
    HarnessOptions, answer, delivered_events, of_kinds, on_task_set, recorded, script_responses,
};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::json;

#[test]
fn manual_compaction_records_exact_summary_and_events() {
    on_task_set(|_| async {
        let h = seeded(HarnessOptions::default()).await;
        let before = messages(&h);
        let events = delivered_events(&h.session);
        let requested = script_responses(
            &h,
            vec![answer("## Goal\nControlled summary", None, None); 2],
        );

        let result = h.session.compact(None).await.unwrap();

        let expected = recorded("compaction_sessions", "manual");
        assert_result(&h, &result, &expected["result"]);
        assert_prompts(&requested.lock().unwrap(), &expected);
        let after = messages(&h);
        assert_eq!(after[0]["role"], "compactionSummary");
        assert_eq!(after[0]["summary"], result.summary);
        assert_eq!(after[1..], before[3..]);
        let manager = h.session.session_manager();
        let manager = manager.borrow();
        assert!(manager.get_session_file().is_none());
        let entries = manager.get_entries();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry["type"] == "compaction")
                .count(),
            1
        );
        assert_eq!(entries.last().unwrap()["fromHook"], false);
        assert_eq!(
            of_kinds(&events.borrow(), &["compaction_start", "compaction_end"]),
            [
                json!({"type": "compaction_start", "reason": "manual"}),
                json!({"type": "compaction_end", "reason": "manual", "aborted": false,
                "willRetry": false, "result": {"summary": result.summary,
                    "firstKeptEntryId": result.first_kept_entry_id,
                    "tokensBefore": 150, "details": {"readFiles": [], "modifiedFiles": []}}}),
            ]
        );
        assert_eq!(of_kinds(&h.events.borrow(), &["compaction_start"]).len(), 1);
    });
}
