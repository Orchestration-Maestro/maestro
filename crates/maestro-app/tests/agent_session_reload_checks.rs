//! A reload applies the saved preferences that the live agent also holds.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{HarnessOptions, harness, on_task_set};
use maestro_agent::QueueMode;
use maestro_settings::MessageDeliveryMode;

#[test]
fn reload_applies_saved_queue_modes_to_the_live_agent() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        assert_eq!(h.session.steering_mode(), QueueMode::OneAtATime);
        assert_eq!(h.session.follow_up_mode(), QueueMode::OneAtATime);
        {
            let mut settings = h.session.settings_manager().borrow_mut();
            settings.set_steering_mode(MessageDeliveryMode::All);
            settings.set_follow_up_mode(MessageDeliveryMode::All);
        }
        h.session.reload().await.unwrap();
        assert_eq!(h.session.steering_mode(), QueueMode::All);
        assert_eq!(h.session.follow_up_mode(), QueueMode::All);
    });
}
