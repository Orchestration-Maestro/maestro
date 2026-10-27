//! Native callback waiters receive the one lifecycle's cached failure.
use crate::lifecycle::replies::Replies;
use crate::{
    guest_calls::{Call, Callbacks, RegisteredCall},
    local_resources::Pump,
};

#[test]
fn overlapping_native_callbacks_share_fault_and_later_calls_never_enqueue() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            let replies = Replies::default();
            let mut callbacks = Callbacks::new(calls.downgrade(), replies.clone());
            let callback = callbacks.allocate().unwrap();
            let handler = callbacks.retain(callback.id, admission.clone()).unwrap();
            let command = handler.command(pump.registry());
            let runner = super::runner("/fault");
            let mut first = command("one".into(), runner.create_command_context());
            let mut second = command("two".into(), runner.create_command_context());
            let mut context = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(std::future::Future::poll(first.as_mut(), &mut context).is_pending());
            assert!(std::future::Future::poll(second.as_mut(), &mut context).is_pending());
            for _ in 0..2 {
                let Call::Registered(RegisteredCall::Command(_, _, _, reply)) =
                    incoming.try_recv().unwrap()
                else {
                    panic!("command expected")
                };
                reply.before(|| {});
            }
            let fault = "component fault /fixture: trapped";
            assert_eq!(replies.fault(fault.into()), fault);
            assert_eq!(first.await.unwrap_err().to_string(), fault);
            assert_eq!(second.await.unwrap_err().to_string(), fault);
            assert_eq!(
                command("later".into(), runner.create_command_context())
                    .await
                    .unwrap_err()
                    .to_string(),
                fault
            );
            assert!(incoming.try_recv().is_err());
            drop((handler, command, runner, admission, calls));
            pump.serve().await;
        });
}
