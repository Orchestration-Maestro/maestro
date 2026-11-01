//! Manual compaction preserves the saved branch, live context and delivered events.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::{assert_prompts, assert_result, messages, seeded};
use crate::support::session_harness::{
    HarnessOptions, answer, delivered_events, message_text, of_kinds, on_task_set, recorded,
    script_responses, start_prompt,
};
use maestro_session::SessionManager;
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

#[test]
fn compacted_session_accepts_the_next_prompt() {
    on_task_set(|_| async {
        let h = seeded(HarnessOptions::default()).await;
        drop(script_responses(
            &h,
            vec![answer("## Goal\nControlled summary", None, None); 2],
        ));
        let result = h.session.compact(None).await.unwrap();
        let events = delivered_events(&h.session);

        start_prompt(&h, "third").await.unwrap().unwrap();

        let requests = h.requests.0.borrow();
        assert_eq!(requests.len(), 3);
        let request = serde_json::to_value(&requests[2].messages).unwrap();
        let texts: Vec<_> = request
            .as_array()
            .unwrap()
            .iter()
            .map(message_text)
            .collect();
        assert_eq!(
            texts,
            [
                json!(format!(
                    "The conversation history before this point was compacted into the following summary:\n\n<summary>\n{}\n</summary>",
                    result.summary
                )),
                json!("two"),
                json!("third"),
            ]
        );
        let after = messages(&h);
        assert_eq!(
            after.iter().map(message_text).collect::<Vec<_>>(),
            [
                json!(result.summary),
                json!("two"),
                json!("third"),
                json!("one"),
            ]
        );
        let manager = h.session.session_manager();
        let manager = manager.borrow();
        let stored = manager.get_entries();
        assert_eq!(stored.last().unwrap()["message"]["role"], "assistant");
        assert_eq!(message_text(&stored.last().unwrap()["message"]), "one");
        assert_eq!(of_kinds(&events.borrow(), &["message_end"]).len(), 2);
        assert_eq!(of_kinds(&events.borrow(), &["agent_end"]).len(), 1);
    });
}

#[test]
fn compaction_survives_reopening_the_history_file() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    let manager = SessionManager::create(&root, Some(&format!("{root}/sessions")), &root).unwrap();
    on_task_set(|_| async {
        let h = seeded(HarnessOptions {
            session_manager: Some(manager),
            scratch: Some(scratch),
            ..HarnessOptions::default()
        })
        .await;
        drop(script_responses(
            &h,
            vec![answer("## Goal\nControlled summary", None, None); 2],
        ));

        let result = h.session.compact(None).await.unwrap();
        let path = h
            .session
            .session_manager()
            .borrow()
            .get_session_file()
            .unwrap()
            .to_owned();
        let reopened = SessionManager::open(&path, None, None).unwrap();

        let entries = reopened.get_entries();
        let compacted: Vec<_> = entries
            .iter()
            .filter(|entry| entry["type"] == "compaction")
            .collect();
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0]["summary"], result.summary);
        assert_eq!(compacted[0]["firstKeptEntryId"], result.first_kept_entry_id);
        assert_eq!(
            compacted[0]["tokensBefore"].as_f64().unwrap().to_bits(),
            result.tokens_before.to_bits()
        );
        assert_eq!(
            compacted[0]["details"],
            json!({"readFiles": [], "modifiedFiles": []})
        );
        assert_eq!(compacted[0]["fromHook"], false);
        let context = reopened.build_session_context().unwrap().messages;
        assert_eq!(context[0]["summary"], result.summary);
        let restored: Vec<maestro_extensions::NativeAgentMessage> =
            serde_json::from_value(json!(context)).unwrap();
        let restored = serde_json::to_value(restored).unwrap();
        assert_eq!(restored.as_array().unwrap()[1..], messages(&h)[1..]);
    });
}
