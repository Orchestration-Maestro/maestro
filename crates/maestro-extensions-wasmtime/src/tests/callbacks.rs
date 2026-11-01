//! Callback aliases retain one native final-release owner and one pump admission.
use crate::lifecycle::replies::Replies;
use crate::{
    guest_calls::{Call, Callbacks},
    local_resources::Pump,
};
use tokio::sync::mpsc;

#[test]
fn callback_aliases_release_once_without_a_store_sender_cycle() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (calls, mut incoming) = mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let first = callbacks.allocate().unwrap();
            let second = callbacks.allocate().unwrap();
            assert_ne!(first.id, second.id);
            let retained = callbacks.retain(first.id, admission.clone()).unwrap();
            let alias = callbacks.retain(first.id, admission.clone()).unwrap();
            let copy = retained.clone();
            drop((calls, admission, retained, alias));
            assert!(matches!(
                incoming.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));
            drop(copy);
            let Some(Call::Release(release)) = incoming.recv().await else {
                panic!("missing release")
            };
            assert_eq!(callbacks.release(&release), Some(first.id));
            assert!(
                callbacks
                    .retain(second.id, Pump::new(crate::tests::support::widgets()).1)
                    .is_ok()
            );
            drop(release);
            let Some(Call::Release(admitted)) = incoming.recv().await else {
                panic!("missing newly admitted release")
            };
            assert_eq!(callbacks.release(&admitted), Some(second.id));
            drop(admitted);
            assert!(incoming.recv().await.is_none());
            assert!(
                callbacks
                    .retain(second.id, Pump::new(crate::tests::support::widgets()).1)
                    .is_err()
            );
            pump.serve().await;
        });
}

#[test]
fn queued_release_cannot_retire_a_callback_retained_again_before_dispatch() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (calls, mut incoming) = mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let callback = callbacks.allocate().unwrap();
            let old = callbacks.retain(callback.id, admission.clone()).unwrap();
            drop(old);
            let Some(Call::Release(old)) = incoming.recv().await else {
                panic!("missing old release")
            };
            let live = callbacks.retain(callback.id, admission.clone()).unwrap();
            assert!(callbacks.release(&old).is_none());
            drop(live);
            let Some(Call::Release(final_release)) = incoming.recv().await else {
                panic!("missing final release")
            };
            assert_eq!(callbacks.release(&final_release), Some(callback.id));
            assert!(callbacks.release(&final_release).is_none());
            drop((calls, admission, old, final_release));
            assert!(incoming.recv().await.is_none());
            pump.serve().await;
        });
}
