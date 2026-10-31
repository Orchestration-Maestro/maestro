//! Context usage and token totals on an application session, compared with the recorded
//! statistics cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, assistant, harness, load_history, on_task_set, recorded, recorded_number,
    usage, user,
};
use maestro_app::SessionStats;
use maestro_extensions::ContextUsage;
use maestro_session::session_manager::CompactionOptions;
use serde_json::{Value, json};

/// The window every case replaces the scripted model's with.
const WINDOW: f64 = 200_000.0;

/// An answer whose input usage is `tokens`.
fn answer(tokens: u64, offset: u64) -> Value {
    assistant(
        "hi",
        offset,
        &json!({"usage": usage([tokens, 0, 0, 0], 0.0, Some(tokens))}),
    )
}

/// A question `text`.
fn question(text: &str, offset: u64) -> Value {
    user(&json!(text), offset)
}

/// The recorded form of a context usage.
fn usage_view(usage: Option<ContextUsage>) -> Value {
    usage.map_or(Value::Null, |usage| {
        json!({
            "tokens": usage.tokens.map_or(Value::Null, recorded_number),
            "contextWindow": recorded_number(usage.context_window),
            "percent": usage.percent.map_or(Value::Null, recorded_number),
        })
    })
}

/// The stored statistics' token input count.
fn input_tokens(stats: &SessionStats) -> Value {
    recorded_number(stats.tokens.input)
}

#[test]
fn current_usage_matches_statistics_and_context_window() {
    let expected = recorded(
        "model_navigation",
        "current_usage_matches_statistics_and_context_window",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            context_window: Some(WINDOW),
            ..HarnessOptions::default()
        })
        .await;
        {
            let mut manager = h.session.session_manager().borrow_mut();
            manager.append_message(question("hello", 1)).unwrap();
            manager.append_message(answer(200, 2)).unwrap();
        }
        load_history(&h);
        let stats = h.session.get_session_stats().unwrap();
        assert_eq!(stats.context_usage, h.session.get_context_usage().unwrap());
        let observed = json!({
            "input": input_tokens(&stats),
            "usage": usage_view(stats.context_usage),
        });
        assert_eq!(observed, expected);
    });
}

/// The conversation before a compaction: two answers and the message its cut keeps.
fn before_compaction(manager: &mut maestro_session::SessionManager) -> String {
    manager.append_message(question("first", 1)).unwrap();
    manager.append_message(answer(180_000, 2)).unwrap();
    let kept = manager.append_message(question("second", 3)).unwrap();
    manager.append_message(answer(195_000, 4)).unwrap();
    manager
        .append_compaction("summary", &kept, 195_000.0, CompactionOptions::default())
        .unwrap();
    kept
}

#[test]
fn compaction_hides_stale_kept_usage() {
    let expected = recorded("model_navigation", "compaction_hides_stale_kept_usage");
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            context_window: Some(WINDOW),
            ..HarnessOptions::default()
        })
        .await;
        {
            let mut manager = h.session.session_manager().borrow_mut();
            before_compaction(&mut manager);
            manager.append_message(question("third", 5)).unwrap();
        }
        load_history(&h);
        let stats = h.session.get_session_stats().unwrap();
        let observed = json!({
            "input": input_tokens(&stats),
            "usage": usage_view(stats.context_usage),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn fresh_usage_after_compaction_replaces_kept_usage() {
    let expected = recorded(
        "model_navigation",
        "fresh_usage_after_compaction_replaces_kept_usage",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            context_window: Some(WINDOW),
            ..HarnessOptions::default()
        })
        .await;
        {
            let mut manager = h.session.session_manager().borrow_mut();
            before_compaction(&mut manager);
            manager.append_message(question("third", 5)).unwrap();
            manager.append_message(answer(25_000, 6)).unwrap();
        }
        load_history(&h);
        let stats = h.session.get_session_stats().unwrap();
        assert_eq!(stats.context_usage, h.session.get_context_usage().unwrap());
        let observed = json!({
            "input": input_tokens(&stats),
            "usage": usage_view(stats.context_usage),
        });
        assert_eq!(observed, expected);
    });
}
