//! Statistics, context usage, the last answer's text, fork points, names, listeners and
//! the session's view of its supplied owners.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, TS, assistant, case, corpus, harness, model_view, on_task_set,
    session_event_view, usage, user,
};
use maestro_app::{AgentSessionEventListener, SessionStats};
use maestro_extensions::NativeAgentMessage;
use maestro_json::compact_json;
use maestro_session::{
    SessionManager,
    session_manager::{CompactionOptions, ReadonlySessionManager},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// A tool call block.
fn tool_call(id: &str) -> Value {
    json!({"type": "toolCall", "id": id, "name": "read", "arguments": {"path": "a.txt"}})
}

/// A tool result record.
fn tool_result(id: &str, text: &str, offset: u64) -> Value {
    json!({"role": "toolResult", "toolCallId": id, "toolName": "read",
        "content": [{"type": "text", "text": text}], "isError": false, "timestamp": TS + offset})
}

/// The recorded mixed history.
fn stat_messages() -> Vec<Value> {
    let calling = json!([{"type": "text", "text": "calling"}, tool_call("t1"), tool_call("t2")]);
    vec![
        user(&json!("question"), 1),
        assistant(
            "calling",
            2,
            &json!({"content": calling,
            "usage": usage([100, 20, 5, 3], 0.25, Some(128)), "stopReason": "toolUse"}),
        ),
        tool_result("t1", "data", 3),
        tool_result("t2", "more", 4),
        json!({"role": "bashExecution", "command": "ls", "output": "a.txt", "exitCode": 0,
            "cancelled": false, "truncated": false, "timestamp": TS + 5}),
        assistant(
            "  done  ",
            6,
            &json!({"usage": usage([200, 40, 10, 6], 0.5, Some(256))}),
        ),
        user(&json!([{"type": "text", "text": "next"}]), 7),
    ]
}

/// Writes JSON numbers as JavaScript does, so `300.0` reads back as `300`.
fn js_numbers(value: &Value) -> Value {
    serde_json::from_str(&compact_json(value).unwrap()).unwrap()
}

/// Session entries added after the history is stored, given the stored identities.
type Compaction = fn(&mut SessionManager, &[String]);

/// Stores `messages`, applies `compaction`, gives the agent `messages` and compares the
/// statistics, context usage and last answer text with the recorded case.
fn stats_case(name: &str, messages: &[Value], window: Option<f64>, compaction: Option<Compaction>) {
    let expected = case("statistics", name);
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            context_window: window,
            ..HarnessOptions::default()
        })
        .await;
        {
            let mut manager = h.session.session_manager().borrow_mut();
            let ids: Vec<String> = messages
                .iter()
                .map(|message| manager.append_message(message.clone()).unwrap())
                .collect();
            if let Some(compaction) = compaction {
                compaction(&mut manager, &ids);
            }
        }
        let history: Vec<NativeAgentMessage> = serde_json::from_value(json!(messages)).unwrap();
        h.session
            .agent()
            .state()
            .write()
            .unwrap()
            .set_messages(history);
        let stats: SessionStats = h.session.get_session_stats().unwrap();
        assert_eq!(stats.session_id, h.session.session_id());
        assert_eq!(stats.session_file, None);
        let usage = |usage: Option<maestro_extensions::ContextUsage>| {
            usage.map(|usage| {
                json!({"tokens": usage.tokens, "contextWindow": usage.context_window, "percent": usage.percent})
            })
        };
        let tokens = &stats.tokens;
        let observed = json!({
            "name": name,
            "contextWindow": window,
            "stats": {
                "sessionId": "$ID",
                "userMessages": stats.user_messages,
                "assistantMessages": stats.assistant_messages,
                "toolCalls": stats.tool_calls,
                "toolResults": stats.tool_results,
                "totalMessages": stats.total_messages,
                "tokens": {"input": tokens.input, "output": tokens.output, "cacheRead": tokens.cache_read,
                    "cacheWrite": tokens.cache_write, "total": tokens.total},
                "cost": stats.cost,
                "contextUsage": usage(stats.context_usage).unwrap_or(json!("missing")),
            },
            "contextUsage": usage(h.session.get_context_usage().unwrap()).unwrap_or(json!({"undefined": true})),
            "lastAssistantText": h.session.get_last_assistant_text(),
        });
        assert_eq!(js_numbers(&observed), expected);
    });
}

/// Branches back to the answer at `index` and records a compaction kept from it.
fn compact_after(manager: &mut SessionManager, ids: &[String], index: usize) {
    manager.branch(&ids[index]).unwrap();
    manager
        .append_compaction("Summary", &ids[index], 500.0, CompactionOptions::default())
        .unwrap();
}

#[test]
fn stats_count_roles_tool_calls_tokens_and_cost() {
    stats_case(
        "counts roles, tool calls, tokens and cost",
        &stat_messages(),
        None,
        None,
    );
}

#[test]
fn stats_without_context_window_have_no_usage() {
    stats_case(
        "no context window means no usage",
        &stat_messages(),
        Some(0.0),
        None,
    );
}

#[test]
fn usage_after_compaction_is_unknown_until_answer() {
    let compaction: Compaction = |manager, ids| compact_after(manager, ids, 1);
    stats_case(
        "usage after the latest compaction is unknown until an answer",
        &stat_messages()[..2],
        None,
        Some(compaction),
    );
}

#[test]
fn answer_after_compaction_gives_usage() {
    let fresh = || {
        assistant(
            "fresh",
            9,
            &json!({"usage": usage([30, 5, 0, 0], 0.1, Some(35))}),
        )
    };
    let mut messages = stat_messages()[..2].to_vec();
    messages.extend([user(&json!("again"), 8), fresh()]);
    let compaction: Compaction = |manager, ids| {
        compact_after(manager, ids, 1);
        manager.append_message(user(&json!("again"), 8)).unwrap();
        let fresh = assistant(
            "fresh",
            9,
            &json!({"usage": usage([30, 5, 0, 0], 0.1, Some(35))}),
        );
        manager.append_message(fresh).unwrap();
    };
    stats_case(
        "an answer after the compaction gives usage",
        &messages,
        None,
        Some(compaction),
    );
}

#[test]
fn aborted_and_failed_answers_after_compaction_are_skipped() {
    let compaction: Compaction = |manager, ids| {
        compact_after(manager, ids, 1);
        manager
            .append_message(assistant("partial", 8, &json!({"stopReason": "aborted"})))
            .unwrap();
        let failed = assistant("", 9, &json!({"stopReason": "error", "errorMessage": "x"}));
        manager.append_message(failed).unwrap();
    };
    stats_case(
        "aborted and failed answers after a compaction are skipped",
        &stat_messages()[..2],
        None,
        Some(compaction),
    );
}

#[test]
fn zero_usage_newest_answer_after_compaction_uses_the_answer_before_it() {
    let compaction: Compaction = |manager, ids| {
        compact_after(manager, ids, 1);
        let old = assistant(
            "old",
            8,
            &json!({"usage": usage([30, 5, 0, 0], 0.1, Some(35))}),
        );
        manager.append_message(old).unwrap();
        let zero = assistant("zero", 9, &json!({"usage": usage([0, 0, 0, 0], 0.0, None)}));
        manager.append_message(zero).unwrap();
    };
    stats_case(
        "a zero-usage newest answer after a compaction uses the answer before it",
        &stat_messages()[..2],
        None,
        Some(compaction),
    );
}

#[test]
fn no_assistant_message_has_no_last_text() {
    stats_case(
        "no assistant message has no last text",
        &[user(&json!("only"), 1)],
        None,
        None,
    );
}

#[test]
fn empty_aborted_answer_is_skipped_for_last_text() {
    let aborted = assistant("x", 2, &json!({"content": [], "stopReason": "aborted"}));
    let messages = vec![assistant("kept", 1, &json!({})), aborted];
    stats_case(
        "an empty aborted answer is skipped for last text",
        &messages,
        None,
        None,
    );
}

#[test]
fn aborted_answer_with_content_gives_last_text() {
    let messages = vec![
        assistant("older", 1, &json!({})),
        assistant("partial", 2, &json!({"stopReason": "aborted"})),
    ];
    stats_case(
        "an aborted answer with content gives last text",
        &messages,
        None,
        None,
    );
}

#[test]
fn blank_answer_has_no_last_text() {
    let messages = vec![assistant("  \n ", 1, &json!({}))];
    stats_case("blank answer text has no last text", &messages, None, None);
}

#[test]
fn only_text_blocks_join_into_last_text() {
    let content = json!([{"type": "text", "text": " one"}, {"type": "thinking", "thinking": "hidden"},
        tool_call("t"), {"type": "text", "text": "two "}]);
    let messages = vec![assistant("a", 1, &json!({"content": content}))];
    stats_case(
        "only text blocks join into the last text",
        &messages,
        None,
        None,
    );
}

/// The recorded history of user messages, an answer and a custom message.
fn fork_history(manager: &mut SessionManager) {
    let image = json!({"type": "image", "data": "AAAA", "mimeType": "image/png"});
    let with_image =
        json!([{"type": "text", "text": "with"}, image, {"type": "text", "text": " image"}]);
    for message in [
        user(&json!("first"), 1),
        assistant("answer", 2, &json!({})),
        user(&with_image, 3),
        user(&json!(""), 4),
        user(&json!([image]), 5),
    ] {
        manager.append_message(message).unwrap();
    }
    let content = serde_json::from_value(json!("custom text")).unwrap();
    manager
        .append_custom_message_entry("note", content, true, maestro_session::Presence::Missing)
        .unwrap();
}

#[test]
fn fork_points_list_user_messages_with_text() {
    let expected = corpus("history");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        fork_history(&mut h.session.session_manager().borrow_mut());
        let forks = h.session.get_user_messages_for_forking();
        let texts: Vec<&str> = forks.iter().map(|fork| fork.text.as_str()).collect();
        assert_eq!(json!(texts), expected["forkTexts"]);
        let manager = h.session.session_manager().borrow();
        let entries = manager.get_entries();
        assert_eq!(forks[0].entry_id, entries[0]["id"].as_str().unwrap());
        assert_eq!(forks[1].entry_id, entries[2]["id"].as_str().unwrap());
    });
}

#[test]
fn session_names_are_trimmed_and_announced() {
    let expected = corpus("history");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut names = Vec::new();
        for name in ["  Release notes  ", "", "Second"] {
            h.session.set_session_name(name).unwrap();
            let manager = h.session.session_manager().borrow();
            let entries = manager.get_entries();
            let entry = entries.last().unwrap();
            names.push(json!({
                "input": name,
                "sessionName": h.session.session_name().unwrap(),
                "entry": {"type": entry["type"], "name": entry["name"]},
            }));
        }
        assert_eq!(json!(names), expected["names"]);
        assert_eq!(json!(*h.events.borrow()), expected["sessionEvents"]);
    });
}

#[test]
fn listeners_receive_events_until_removed() {
    let expected = corpus("listeners");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let listener = |label: &'static str| -> AgentSessionEventListener {
            let seen = Rc::clone(&seen);
            Rc::new(move |event| {
                let view = session_event_view(event).unwrap();
                let name = view["name"].as_str().unwrap_or_default().to_owned();
                seen.borrow_mut()
                    .push(format!("{label}:{}:{name}", view["type"].as_str().unwrap()));
            })
        };
        let first = h.session.subscribe(listener("first"));
        let _second = h.session.subscribe(listener("second"));
        h.session.set_session_name("one").unwrap();
        first();
        first();
        h.session.set_session_name("two").unwrap();
        assert_eq!(json!(*seen.borrow()), expected["seen"]);
    });
}

/// The owner-backed observations of a session.
fn owner_view(h: &Harness) -> Value {
    let history = h.session.messages();
    let live_history = Arc::ptr_eq(&history, h.session.state().read().unwrap().messages());
    json!({
        "model": model_view(&h.session.model()),
        "thinkingLevel": h.session.thinking_level(),
        "streaming": h.session.is_streaming(),
        // The prompt spells the working directory with `/` separators on every host.
        "systemPrompt": h.session.system_prompt()
            .ends_with(&format!("directory: {}", h.cwd.replace('\\', "/"))),
        "liveHistory": live_history,
        "sessionId": h.session.session_id() == h.session.session_manager().borrow().get_session_id(),
        "templates": h.session.prompt_templates().prompts.len(),
        "scoped": h.session.scoped_models().len(),
    })
}

#[test]
fn session_getters_read_the_supplied_owners() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let memory = harness(HarnessOptions::default()).await;
        let view = json!({"model": "faux/plain", "thinkingLevel": "off", "streaming": false,
            "systemPrompt": true, "liveHistory": true, "sessionId": true, "templates": 0, "scoped": 0});
        assert_eq!(owner_view(&memory), view);
        assert_eq!(memory.session.session_file(), None);
        assert!(Arc::ptr_eq(
            memory.session.model_registry(),
            &memory.registry
        ));
        memory
            .session
            .set_scoped_models(vec![Harness::scoped(memory.model("think"), Some("high"))]);
        let scoped = memory.session.scoped_models();
        assert_eq!(model_view(&scoped[0].model), "faux/think");
        let sessions = format!("{root}/sessions");
        let file =
            SessionManager::create(&format!("{root}/project"), Some(&sessions), &root).unwrap();
        let path = file.get_session_file().map(str::to_owned);
        let file = harness(HarnessOptions {
            session_manager: Some(file),
            ..HarnessOptions::default()
        })
        .await;
        assert_eq!(owner_view(&file), view);
        assert_eq!(file.session.session_file(), path);
        assert!(path.is_some());
    });
}
