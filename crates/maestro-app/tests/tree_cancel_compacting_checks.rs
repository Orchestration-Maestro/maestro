//! A cancelled tree navigation releasing the branch summary state, compared with the
//! recorded regression case.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, assistant, harness, on_task_set, recorded, user,
};
use maestro_extensions::{ExtensionFactory, SessionBeforeTreeEvent, SessionBeforeTreeResult};
use maestro_models::Local;
use maestro_session::session_manager::NavigateTreeOptions;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::json;
use std::rc::Rc;

/// An extension cancelling every tree navigation.
fn vetoing() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(|_, _| {
                let result: SessionBeforeTreeResult =
                    serde_json::from_value(json!({"cancel": true})).unwrap();
                Box::pin(std::future::ready(Ok(Some(result))))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn tree_veto_releases_summary_state_without_moving_leaf() {
    let expected = recorded(
        "model_navigation",
        "tree_veto_releases_summary_state_without_moving_leaf",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            factories: vec![vetoing()],
            ..HarnessOptions::default()
        })
        .await;
        let (target, leaf) = {
            let mut manager = h.session.session_manager().borrow_mut();
            let target = manager.append_message(user(&json!("first"), 1)).unwrap();
            manager
                .append_message(assistant("reply", 2, &json!({})))
                .unwrap();
            let leaf = manager.append_message(user(&json!("second"), 3)).unwrap();
            (target, leaf)
        };
        let before = h
            .session
            .session_manager()
            .borrow()
            .get_leaf_id()
            .map(str::to_owned);
        let result = h
            .session
            .navigate_tree(&target, NavigateTreeOptions::default())
            .await
            .unwrap();
        let manager = h.session.session_manager().borrow();
        let after = manager.get_leaf_id().map(str::to_owned);
        let observed = json!({
            "result": {"cancelled": result.cancelled},
            "compacting": h.session.is_compacting(),
            "same_leaf": json!(before == after && after.as_deref() == Some(leaf.as_str())),
        });
        assert_eq!(observed, expected);
    });
}
