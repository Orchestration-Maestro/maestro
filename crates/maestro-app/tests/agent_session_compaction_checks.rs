//! Manual compaction preserves the saved branch, live context and delivered events.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::{assert_events, assert_prompts, assert_result, messages, seeded};
use crate::support::session_harness::{
    Harness, HarnessOptions, answer, corpus, delivered_events, harness, history_view, load_history,
    message_text, of_kinds, on_task_set, recorded, script_responses, start_prompt,
};
use maestro_session::session_manager::{CompactionOptions, ReadonlySessionManager};
use maestro_session::{CompactionResult, SessionManager};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Opens the literal synthetic branch through the existing session-file reader.
fn literal_history_options(expected: &Value) -> HarnessOptions {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("history.jsonl");
    let input = include_str!("fixtures/before-compaction.jsonl");
    assert_eq!(json!(input.lines().count()), expected["input"]["rows"]);
    std::fs::write(&path, input).unwrap();
    let manager = SessionManager::open(path.to_str().unwrap(), None, None).unwrap();
    assert_history_inventory(&manager, &expected["input"]);
    assert_eq!(
        json!(manager.get_branch(None).unwrap().len()),
        expected["branch_count"]
    );
    HarnessOptions {
        session_manager: Some(manager),
        scratch: Some(scratch),
        context_window: Some(200_000.0),
        settings: json!({"compaction": {"enabled": false, "reserveTokens": 16384, "keepRecentTokens": 20000}}),
        ..HarnessOptions::default()
    }
}

/// Checks message, model and thinking records plus tool-call links.
fn assert_history_inventory(manager: &SessionManager, expected: &Value) {
    let entries = manager.get_entries();
    assert_eq!(json!(entries.len()), expected["entries"]);
    for kind in ["model_change", "thinking_level_change", "compaction"] {
        assert_eq!(
            json!(entries.iter().filter(|entry| entry["type"] == kind).count()),
            expected["types"][kind]
        );
    }
    assert_eq!(
        json!(
            entries
                .iter()
                .filter(|entry| entry["type"] == "message")
                .count()
        ),
        expected["messages"]
    );
    for (role, count) in expected["roles"].as_object().unwrap() {
        assert_eq!(
            json!(
                entries
                    .iter()
                    .filter(|entry| entry["message"]["role"] == *role)
                    .count()
            ),
            *count
        );
    }
    let calls: HashMap<_, _> = entries
        .iter()
        .filter_map(|entry| entry["message"]["content"].as_array())
        .flatten()
        .filter(|block| block["type"] == "toolCall")
        .map(|call| (call["id"].as_str().unwrap(), call["name"].as_str().unwrap()))
        .collect();
    for entry in entries
        .iter()
        .filter(|entry| entry["message"]["role"] == "toolResult")
    {
        let result = &entry["message"];
        assert_eq!(
            calls.get(result["toolCallId"].as_str().unwrap()).copied(),
            result["toolName"].as_str()
        );
    }
}

/// Rebuilds with the adjacent valid boundary to show the exact-tail check rejects it.
fn assert_changed_cut(h: &Harness, result: &CompactionResult, expected: &Value) {
    let manager = h.session.session_manager();
    let mut manager = manager.borrow_mut();
    let index = usize::try_from(expected["first_kept_index"].as_u64().unwrap()).unwrap();
    let adjacent = manager.get_entries()[index + 1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    manager
        .append_compaction(
            &result.summary,
            &adjacent,
            result.tokens_before,
            CompactionOptions::default(),
        )
        .unwrap();
    let wrong = manager.build_session_context().unwrap().messages;
    assert_eq!(json!(wrong.len()), expected["retained_count"]);
}

#[test]
fn literal_histories_keep_the_exact_retained_tail() {
    let expected = corpus("compaction_long_histories");
    let mut expected = expected
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "before" && row["changed_cut"] == false)
        .unwrap()
        .clone();
    let options = literal_history_options(&expected);
    expected["result"]["first_kept_index"] = expected["first_kept_index"].clone();
    on_task_set(|_| async {
        let h = harness(options).await;
        load_history(&h);
        let before = messages(&h);
        assert_eq!(json!(before.len()), expected["before_count"]);
        let requested = script_responses(
            &h,
            vec![answer("## Goal\nControlled summary", None, None); 2],
        );

        let result = h.session.compact(None).await.unwrap();

        assert_result(&h, &result, &expected["result"]);
        assert_prompts(&requested.lock().unwrap(), &expected);
        let after = messages(&h);
        assert_eq!(json!(after.len()), expected["after_count"]);
        assert_eq!(after[0]["role"], "compactionSummary");
        assert_eq!(after[0]["summary"], result.summary);
        let retained = usize::try_from(expected["retained_count"].as_u64().unwrap()).unwrap();
        assert_eq!(after[1..], before[before.len() - retained..]);
        assert_events(
            &h,
            &of_kinds(&h.events.borrow(), &["compaction_start", "compaction_end"]),
            &expected["events"],
        );
        assert_eq!(
            h.session
                .session_manager()
                .borrow()
                .get_entries()
                .iter()
                .filter(|entry| entry["type"] == "compaction")
                .count(),
            usize::try_from(expected["input"]["types"]["compaction"].as_u64().unwrap()).unwrap()
                + 1
        );
        assert_changed_cut(&h, &result, &expected);
    });
}

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
        let expected_messages: Vec<_> = expected["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| json!({"role": message["role"], "text": message["text"]}))
            .collect();
        assert_eq!(history_view(&h), expected_messages);
        assert_eq!(
            after[1]["stopReason"],
            expected["messages"][1]["stopReason"]
        );
        let manager = h.session.session_manager();
        let manager = manager.borrow();
        assert!(manager.get_session_file().is_none());
        let entries = manager.get_entries();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry["type"] == "compaction")
                .count(),
            usize::try_from(expected["stored_count"].as_u64().unwrap()).unwrap()
        );
        assert_eq!(entries.last().unwrap()["fromHook"], false);
        assert_events(
            &h,
            &of_kinds(&events.borrow(), &["compaction_start", "compaction_end"]),
            &expected["events"],
        );
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
