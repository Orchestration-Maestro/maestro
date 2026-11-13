//! Decisions before session selection effects, refused over the protocol.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::rpc_connection::connect;
use maestro_app::test_support::{RuntimeOptions, assistant, on_task_set, runtime_with};
use maestro_settings::{DecisionRequest, SettingsDecider};
use maestro_test_directory::Scratch;
use serde_json::json;
use std::{rc::Rc, sync::Arc};

/// Refuses every explicit operation.
struct Refuse;
impl SettingsDecider for Refuse {
    fn decide(&self, _: &DecisionRequest<'_>) -> Result<(), String> {
        Err("configuration release refuses selection".into())
    }
}

/// Protocol model and thinking failures retain the caller's correlation and session bytes.
#[test]
fn rpc_selection_refusals_preserve_history_and_return_the_message() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_with(
            Scratch::new().unwrap(),
            "decisions",
            RuntimeOptions {
                models: vec![("faux-1", true), ("faux-2", true)],
                ..RuntimeOptions::default()
            },
        )
        .await
        .unwrap();
        let runtime = Rc::new(runtime);
        let session = runtime.session();
        session
            .settings_manager()
            .borrow_mut()
            .register_decider(Arc::new(Refuse))
            .unwrap();
        session
            .session_manager()
            .borrow_mut()
            .append_message(assistant("seed", 0, &json!({})))
            .unwrap();
        let path = session.session_file().unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut client = connect(Rc::clone(&runtime));
        for command in [
            json!({"type":"set_model","provider":"faux","modelId":"faux-2"}),
            json!({"type":"cycle_model"}),
            json!({"type":"set_thinking_level","level":"high"}),
            json!({"type":"cycle_thinking_level"}),
            json!({"type":"set_steering_mode", "mode":"all"}),
            json!({"type":"set_follow_up_mode", "mode":"all"}),
            json!({"type":"set_auto_compaction", "enabled": false}),
            json!({"type":"set_auto_retry", "enabled": false}),
        ] {
            let reply = client.ask(&command.to_string()).await;
            assert_eq!(reply["success"], false);
            assert_eq!(reply["error"], "configuration release refuses selection");
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        client.end(None);
        client.outcome().await.unwrap();
    })
    .unwrap();
}
