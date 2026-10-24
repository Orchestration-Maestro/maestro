//! Active-branch forks and destination state restored through the runtime.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::runtime_fixtures::{RuntimeOptions, runtime_for, runtime_with};
use crate::support::session_harness::{
    Harness, answer, corpus, delivery_message_view, on_task_set, script_responses,
    with_placeholders,
};
use maestro_app::PromptOptions;
use maestro_extensions::{
    ForkPosition,
    types::{ForkOptions, SwitchSessionOptions},
};
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use serde_json::{Value, json};

/// Forks at the complete branch's leaf and records storage and ordered messages.
async fn leaf_fork(persisted: bool) -> Value {
    let (h, runtime, _) = runtime_with(
        "leaf",
        RuntimeOptions {
            persisted,
            bootstrap_model: true,
            ..RuntimeOptions::default()
        },
    )
    .await;
    assert_eq!(runtime.session().model().read().unwrap().id, "faux-1");
    drop(script_responses(
        &h,
        vec![answer("one", None, None), answer("two", None, None)],
    ));
    for text in ["hello", "again"] {
        runtime
            .session()
            .prompt(text, PromptOptions::default())
            .await
            .unwrap();
    }
    let previous = runtime.session().session_file();
    let messages = serde_json::to_value(&*runtime.session().messages().read().unwrap()).unwrap();
    let leaf = runtime
        .session()
        .session_manager()
        .borrow()
        .get_leaf_id()
        .unwrap()
        .to_owned();
    let result = runtime
        .fork(
            &leaf,
            ForkOptions {
                position: Some(ForkPosition::At),
                with_session: None,
            },
        )
        .await
        .unwrap();
    let current = runtime.session();
    assert_eq!(
        serde_json::to_value(&*current.messages().read().unwrap()).unwrap(),
        messages
    );
    assert!(result.selected_text.is_none());
    let observed = json!({
        "result": {"cancelled": result.cancelled},
        "changedFile": previous != current.session_file(),
        "beforeFile": previous.is_some(), "afterFile": current.session_file().is_some(),
        "messages": messages.as_array().unwrap().iter().map(delivery_message_view).collect::<Vec<_>>()
    });
    runtime.dispose().await.unwrap();
    observed
}

#[test]
fn leaf_fork_preserves_the_file_backed_active_branch() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(leaf_fork(true).await, expected["at_file"]);
    });
}

#[test]
fn leaf_fork_preserves_the_in_memory_active_branch() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(leaf_fork(false).await, expected["at_memory"]);
    });
}

/// A destination whose assistant and recorded selection both identify the second model.
fn destination(h: &Harness) -> SessionManager {
    let root = h.scratch.path().display().to_string();
    let cwd = format!("{root}/case-11/destination");
    std::fs::create_dir_all(&cwd).unwrap();
    let mut manager =
        SessionManager::create(&cwd, Some(&format!("{root}/destination-sessions")), &root).unwrap();
    manager
        .append_message(json!({"role": "user", "content": "target", "timestamp": 0}))
        .unwrap();
    let mut response = answer("target answer", None, None);
    response.model = "faux-2".into();
    manager
        .append_message(serde_json::to_value(response).unwrap())
        .unwrap();
    manager.append_model_change("faux", "faux-2").unwrap();
    manager.append_thinking_level_change("off").unwrap();
    manager
}

#[test]
fn switch_reads_the_destination_working_directory() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_for("original", Vec::new()).await;
        let original = runtime.cwd();
        let target = destination(&h);
        assert_ne!(original, target.get_cwd());
        runtime
            .switch_session(
                target.get_session_file().unwrap(),
                None,
                SwitchSessionOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            with_placeholders(json!(runtime.cwd()), &h),
            expected["destination"]["cwd"]
        );
        assert_eq!(
            with_placeholders(
                json!(runtime.session().session_manager().borrow().get_cwd()),
                &h
            ),
            expected["destination"]["managerCwd"]
        );
        let messages: Vec<Value> = runtime
            .session()
            .messages()
            .read()
            .unwrap()
            .iter()
            .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
            .collect();
        assert_eq!(json!(messages), expected["destination"]["messages"]);
        runtime.dispose().await.unwrap();
    });
}

#[test]
fn switch_restores_the_destination_model_and_thinking() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_with(
            "restore",
            RuntimeOptions {
                bootstrap_model: false,
                persisted: true,
                models: vec![("faux-1", true), ("faux-2", false)],
                ..RuntimeOptions::default()
            },
        )
        .await;
        let target = destination(&h);
        runtime
            .switch_session(
                target.get_session_file().unwrap(),
                None,
                SwitchSessionOptions::default(),
            )
            .await
            .unwrap();
        assert!(runtime.session().session_file().is_some());
        assert_eq!(
            json!(runtime.session().model().read().unwrap().id),
            expected["destination"]["model"]
        );
        assert_eq!(
            json!(runtime.session().thinking_level()),
            expected["destination"]["thinking"]
        );
        runtime.dispose().await.unwrap();
    });
}
