//! Fork candidates, selected text and retained conversation prefixes.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::runtime_fixtures::{RuntimeOptions, runtime_with};
use crate::support::session_harness::{
    answer, corpus, delivery_message_view, on_task_set, script_responses,
};
use maestro_app::PromptOptions;
use maestro_extensions::types::ForkOptions;
use serde_json::{Value, json};

/// Prompts each candidate and observes the branch selected before `selected`.
async fn branching_case(case: &Value, persisted: bool, selected: usize) -> Value {
    let (h, runtime, _) = runtime_with(
        "branch",
        RuntimeOptions {
            persisted,
            ..RuntimeOptions::default()
        },
    )
    .await;
    drop(script_responses(
        &h,
        vec![
            answer("one", None, None),
            answer("two", None, None),
            answer("three", None, None),
        ],
    ));
    for user in case["users"].as_array().unwrap() {
        runtime
            .session()
            .prompt(user["text"].as_str().unwrap(), PromptOptions::default())
            .await
            .unwrap();
    }
    assert!(!runtime.session().messages().read().unwrap().is_empty());
    let before_file = runtime.session().session_file().is_some();
    let users = runtime.session().get_user_messages_for_forking();
    let candidates: Vec<Value> = users
        .iter()
        .map(|user| json!({"text": user.text}))
        .collect();
    let result = runtime
        .fork(&users[selected].entry_id, ForkOptions::default())
        .await
        .unwrap();
    let session = runtime.session();
    let messages: Vec<Value> = session
        .messages()
        .read()
        .unwrap()
        .iter()
        .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
        .collect();
    let mut observed = json!({"users": candidates, "result": {"cancelled": result.cancelled, "selectedText": result.selected_text}, "messages": messages});
    if case.get("beforeFile").is_some() {
        let file = session.session_file();
        observed["beforeFile"] = json!(before_file);
        observed["afterFile"] = json!(file.is_some());
        observed["exists"] = json!(file.is_some_and(|path| std::path::Path::new(&path).exists()));
    }
    runtime.dispose().await.unwrap();
    observed
}

#[test]
fn single_user_fork_has_an_unpublished_file() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(
            branching_case(&expected["single_file"], true, 0).await,
            expected["single_file"]
        );
    });
}

#[test]
fn single_user_memory_fork_keeps_storage_in_memory() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(
            branching_case(&expected["single_memory"], false, 0).await,
            expected["single_memory"]
        );
    });
}

#[test]
fn middle_user_fork_retains_only_the_preceding_exchange() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(
            branching_case(&expected["middle"], true, 1).await,
            expected["middle"]
        );
    });
}
