//! Retained task records cannot retain admission after once-only completion.
use super::*;
use crate::lifecycle::replies::Replies;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Tracks operand release before either bookkeeping or native settlement.
struct Released(Arc<AtomicUsize>);
impl Drop for Released {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn completion_releases_operands_and_sender_even_when_task_record_survives() {
    let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let weak = calls.downgrade();
    let completion = Completion::new(calls);
    let released = Arc::new(AtomicUsize::new(0));
    completion.retain(Released(released.clone()));
    completion.finish();
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(matches!(incoming.try_recv().unwrap(), Call::Finished));
    assert!(weak.upgrade().is_none());
    completion.finish();
    drop(completion);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(incoming.try_recv().is_err());
}

#[test]
fn cancelled_started_native_caller_releases_once_before_fault_settlement() {
    let replies = Replies::default();
    let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let completion = Completion::new(calls);
    let released = Arc::new(AtomicUsize::new(0));
    completion.retain(Released(released.clone()));
    let settled = Arc::new(AtomicUsize::new(0));
    let observed = released.clone();
    let count = settled.clone();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let reply = replies
        .register(move |result: Result<(), String>| {
            assert_eq!(observed.load(Ordering::SeqCst), 1);
            count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(result.unwrap_err(), "trap");
            assert!(sender.send(()).is_err());
        })
        .unwrap();
    Call::Start(reply).before(&completion);
    drop(receiver);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert_eq!(settled.load(Ordering::SeqCst), 0);
    replies.fault("trap".into());
    assert_eq!(settled.load(Ordering::SeqCst), 1);
    assert!(matches!(incoming.try_recv().unwrap(), Call::Finished));
    completion.finish();
    replies.fault("later fault".into());
    drop(completion);
    assert_eq!(settled.load(Ordering::SeqCst), 1);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(incoming.try_recv().is_err());
}

#[test]
fn fault_releases_trapped_release_admission_while_task_operands_survive() {
    let (_pump, admission) = crate::local_resources::Pump::new(crate::tests::support::widgets());
    let replies = Replies::default();
    let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let weak = calls.downgrade();
    let mut callbacks = crate::guest_calls::Callbacks::new(weak.clone(), replies.clone());
    let callback = callbacks.allocate().unwrap();
    let handle = callbacks.retain(callback.id, admission).unwrap();
    drop(handle);
    let call = incoming.try_recv().unwrap();
    let completion = Completion::new(calls);
    call.before(&completion);
    let task_operands = call.admitted(&completion);
    replies.fault("release trap".into());
    assert!(
        weak.upgrade().is_none(),
        "trapped release retained Store admission"
    );
    assert!(matches!(incoming.try_recv().unwrap(), Call::Finished));
    drop((task_operands, completion));
    assert!(incoming.try_recv().is_err());
}

#[test]
fn healthy_release_and_stale_skip_each_complete_once() {
    for stale in [false, true] {
        let (_pump, admission) =
            crate::local_resources::Pump::new(crate::tests::support::widgets());
        let replies = Replies::default();
        let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        let mut callbacks = crate::guest_calls::Callbacks::new(calls.downgrade(), replies);
        let callback = callbacks.allocate().unwrap();
        let handle = callbacks.retain(callback.id, admission.clone()).unwrap();
        drop(handle);
        let call = incoming.try_recv().unwrap();
        let live = stale.then(|| callbacks.retain(callback.id, admission).unwrap());
        let completion = Completion::new(calls);
        call.before(&completion);
        let Call::Release(mut release) = call.admitted(&completion) else {
            panic!("missing release");
        };
        assert_eq!(callbacks.release(&release), (!stale).then_some(callback.id));
        release.completion.take().unwrap().send(Ok(())).unwrap();
        assert!(matches!(incoming.try_recv().unwrap(), Call::Finished));
        completion.finish();
        drop((release, completion));
        assert!(incoming.try_recv().is_err());
        drop(live);
    }
}
