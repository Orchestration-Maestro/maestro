//! Prompt-built navigation, summary placement and cancellation against recorded outcomes.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::session_harness::{
    Harness, HarnessOptions, answer, harness, hold_first_answer, on_task_set, prompt_to_end,
    recorded, script_responses,
};
use maestro_app::agent_session::{NavigateTreeResult, PromptOptions};
use maestro_session::session_manager::{NavigateTreeOptions, ReadonlySessionManager};
use serde_json::{Value, json};

/// A session whose message entries were produced by completed prompt turns.
async fn conversation(turns: &[&str]) -> (Harness, Vec<String>) {
    let h = harness(HarnessOptions {
        settings: json!({"compaction": {"keepRecentTokens": 1}}),
        ..HarnessOptions::default()
    })
    .await;
    for text in turns {
        h.faux
            .set_responses(vec![maestro_models::FauxResponseStep::Message(Box::new(
                answer("Answer", None, None),
            ))]);
        prompt_to_end(&h, text, PromptOptions::default()).await;
    }
    let ids = h
        .session
        .session_manager()
        .borrow()
        .get_entries()
        .iter()
        .filter(|entry| entry["type"] == "message")
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect();
    (h, ids)
}

/// The navigation result's complete recorded projection.
fn result_view(result: &NavigateTreeResult) -> Value {
    json!({
        "cancelled": result.cancelled,
        "aborted": result.aborted,
        "editor_text": result.editor_text,
        "summary": result.summary_entry.as_ref().map(|entry| &entry.summary),
    })
}

/// Whether the active leaf is `id`.
fn is_leaf(h: &Harness, id: Option<&str>) -> bool {
    h.session.session_manager().borrow().get_leaf_id() == id
}

/// Navigates with a generated summary and observes its stored parent and leaf.
async fn summarized(h: &Harness, target: &str, parent: Option<&str>) -> Value {
    script_responses(h, vec![answer("Branch summary", None, None)]);
    let result = h
        .session
        .navigate_tree(
            target,
            NavigateTreeOptions {
                summarize: Some(true),
                ..NavigateTreeOptions::default()
            },
        )
        .await
        .unwrap();
    let summary = result.summary_entry.as_ref().expect("a summary");
    let entry = serde_json::to_value(summary).unwrap();
    json!({
        "result": result_view(&result),
        "type": entry["type"],
        "parent_matches": entry["parentId"].as_str() == parent,
        "leaf_is_summary": is_leaf(h, entry["id"].as_str()),
    })
}

#[test]
fn root_user_navigation_restores_editor_and_empty_leaf() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["First message", "Second message"]).await;
        let result = h
            .session
            .navigate_tree(&ids[0], NavigateTreeOptions::default())
            .await
            .unwrap();
        assert_eq!(
            json!({"result": result_view(&result), "leaf_absent": is_leaf(&h, None)}),
            recorded(
                "model_navigation",
                "root_user_navigation_restores_editor_and_empty_leaf"
            )
        );
    });
}

#[test]
fn assistant_navigation_selects_the_message_without_editor_text() {
    on_task_set(|_| async {
        // The leaf is the second answer, so selecting the first answer is a real move.
        let (h, ids) = conversation(&["Hello", "Again"]).await;
        assert!(is_leaf(&h, Some(&ids[3])));
        let result = h
            .session
            .navigate_tree(&ids[1], NavigateTreeOptions::default())
            .await
            .unwrap();
        assert_eq!(
            json!({"result": result_view(&result), "leaf_is_assistant": is_leaf(&h, Some(&ids[1]))}),
            recorded(
                "model_navigation",
                "assistant_navigation_selects_the_message_without_editor_text"
            )
        );
    });
}

#[test]
fn root_summary_becomes_a_new_root_and_leaf() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["What is 2+2?", "What is 3+3?"]).await;
        assert_eq!(
            summarized(&h, &ids[0], None).await,
            recorded(
                "model_navigation",
                "root_summary_becomes_a_new_root_and_leaf"
            )
        );
    });
}

#[test]
fn nested_user_summary_is_a_sibling_of_the_selected_user() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["Message one", "Message two", "Message three"]).await;
        let mut observed = summarized(&h, &ids[2], Some(&ids[1])).await;
        let manager = h.session.session_manager().borrow();
        let children = manager.get_children(&ids[1]);
        assert!(children.iter().any(|entry| entry["id"] == ids[2]));
        let mut types: Vec<_> = children.iter().map(|entry| entry["type"].clone()).collect();
        types.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        observed["child_types"] = json!(types);
        assert_eq!(
            observed,
            recorded(
                "model_navigation",
                "nested_user_summary_is_a_sibling_of_the_selected_user"
            )
        );
    });
}

#[test]
fn assistant_summary_is_attached_to_the_selected_message() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["First message", "Second message"]).await;
        assert_eq!(
            summarized(&h, &ids[1], Some(&ids[1])).await,
            recorded(
                "model_navigation",
                "assistant_summary_is_attached_to_the_selected_message"
            )
        );
    });
}

#[test]
fn aborting_a_started_summary_preserves_tree_and_clears_state() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["First message", "Second message"]).await;
        let before = json!(h.session.session_manager().borrow().get_entries());
        let (requested, release) = hold_first_answer(&h, "Branch summary", &[]);
        let navigation = h.session.navigate_tree(
            &ids[0],
            NavigateTreeOptions {
                summarize: Some(true),
                ..NavigateTreeOptions::default()
            },
        );
        let cancel = async {
            requested.notified().await;
            let during = h.session.is_compacting();
            h.session.abort_branch_summary();
            release.notify_one();
            during
        };
        let (result, during) = tokio::join!(navigation, cancel);
        let result = result.unwrap();
        let observed = json!({
            "during": during, "result": result_view(&result),
            "same_entries": before == json!(h.session.session_manager().borrow().get_entries()),
            "same_leaf": is_leaf(&h, ids.last().map(String::as_str)),
            "after": h.session.is_compacting(),
        });
        assert_eq!(
            observed,
            recorded(
                "model_navigation",
                "aborting_a_started_summary_preserves_tree_and_clears_state"
            )
        );
    });
}

#[test]
fn plain_navigation_does_not_append_a_summary() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["First message", "Second message"]).await;
        let before = json!(h.session.session_manager().borrow().get_entries());
        h.session
            .navigate_tree(
                &ids[0],
                NavigateTreeOptions {
                    summarize: Some(false),
                    ..NavigateTreeOptions::default()
                },
            )
            .await
            .unwrap();
        let after = json!(h.session.session_manager().borrow().get_entries());
        assert_eq!(
            json!({"same_entries": before == after,
            "summaries": after.as_array().unwrap().iter().filter(|entry| entry["type"] == "branch_summary").count()}),
            recorded(
                "model_navigation",
                "plain_navigation_does_not_append_a_summary"
            )
        );
    });
}

#[test]
fn same_leaf_navigation_is_an_observable_noop() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["Hello"]).await;
        let before = json!(h.session.session_manager().borrow().get_entries());
        let result = h
            .session
            .navigate_tree(&ids[1], NavigateTreeOptions::default())
            .await
            .unwrap();
        assert_eq!(
            json!({"result": result_view(&result),
            "same_leaf": is_leaf(&h, Some(&ids[1])),
            "same_entries": before == json!(h.session.session_manager().borrow().get_entries())}),
            recorded(
                "model_navigation",
                "same_leaf_navigation_is_an_observable_noop"
            )
        );
    });
}

#[test]
fn custom_summary_focus_reaches_the_summary_request() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["What is TypeScript?", "Tell me more"]).await;
        let prompts = script_responses(
            &h,
            vec![answer("Branch summary MONKEY MONKEY MONKEY", None, None)],
        );
        let instructions = "After the summary, you MUST end with exactly: MONKEY MONKEY MONKEY. This is of utmost importance.";
        let result = h
            .session
            .navigate_tree(
                &ids[0],
                NavigateTreeOptions {
                    summarize: Some(true),
                    custom_instructions: Some(instructions.into()),
                    ..NavigateTreeOptions::default()
                },
            )
            .await
            .unwrap();
        let prompts = prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            json!({"result": result_view(&result),
            "request_contains_instructions": prompts[0].contains(instructions),
            "request_uses_additional_focus": prompts[0].contains("Additional focus")}),
            recorded(
                "model_navigation",
                "custom_summary_focus_reaches_the_summary_request"
            )
        );
    });
}

#[test]
fn cross_branch_summary_captures_the_branch_being_left() {
    on_task_set(|_| async {
        let (h, ids) = conversation(&["Main branch start", "Main branch continue"]).await;
        h.session
            .navigate_tree(&ids[1], NavigateTreeOptions::default())
            .await
            .unwrap();
        h.faux
            .set_responses(vec![maestro_models::FauxResponseStep::Message(Box::new(
                answer("Alternate answer", None, None),
            ))]);
        prompt_to_end(&h, "Branch path", PromptOptions::default()).await;
        let prompts = script_responses(&h, vec![answer("Branch summary", None, None)]);
        let result = h
            .session
            .navigate_tree(
                &ids[2],
                NavigateTreeOptions {
                    summarize: Some(true),
                    ..NavigateTreeOptions::default()
                },
            )
            .await
            .unwrap();
        let prompts = prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            json!({"result": result_view(&result),
            "request_contains_abandoned_branch": prompts[0].contains("Branch path")}),
            recorded(
                "model_navigation",
                "cross_branch_summary_captures_the_branch_being_left"
            )
        );
    });
}
