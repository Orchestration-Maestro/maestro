//! An abort observer registers only once the pump that serves it is live.
use super::Pump;
use crate::{
    guest_calls::{Call, Callbacks},
    lifecycle::replies::Replies,
    local_resources::Registry,
};
use maestro_extensions::{Cancellation, Local, create_extension_runtime};
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn subscribe_without_a_live_pump_leaves_no_observer_registered() {
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    let registry = Registry {
        pump: std::rc::Weak::new(),
        ..pump.registry()
    };
    let (_owner, api) = crate::tests::api::api(create_extension_runtime::<Local>());
    let api = registry.api(api).unwrap();
    let (calls, mut released) = unbounded_channel::<Call>();
    let listener = Callbacks::new(calls.downgrade(), Replies::default()).observer(0);
    let signal = Cancellation::new();
    let error = registry.subscribe(&signal, listener, &api).err().unwrap();
    assert_eq!(error, "extension resource owner ended");
    assert!(
        matches!(released.try_recv(), Ok(Call::Release(_))),
        "the refused subscription kept its listener"
    );
}
