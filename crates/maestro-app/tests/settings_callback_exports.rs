//! The queue vocabulary the interactive settings consumer needs, imported at the root.
//!
//! `delivery_mode` and `queue_mode` belong to the native session, so the browser build skips it.
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::{MessageDeliveryMode, QueueMode, delivery_mode, queue_mode};

#[test]
fn settings_queue_policy_exports_keep_distinct_types() {
    assert_eq!(
        queue_mode(&MessageDeliveryMode::All),
        QueueMode::All,
        "the panel's all value reaches the agent's all queue mode"
    );
    assert_eq!(
        queue_mode(&MessageDeliveryMode::OneAtATime),
        QueueMode::OneAtATime,
        "the panel's single value reaches the agent's one-at-a-time queue mode"
    );
    assert_eq!(
        queue_mode(&MessageDeliveryMode::Unknown("queued".to_owned())),
        QueueMode::OneAtATime,
        "an unknown delivery mode keeps the existing one-at-a-time fallback"
    );
    assert_eq!(delivery_mode(QueueMode::All), MessageDeliveryMode::All);
    assert_eq!(
        delivery_mode(QueueMode::OneAtATime),
        MessageDeliveryMode::OneAtATime
    );
}
