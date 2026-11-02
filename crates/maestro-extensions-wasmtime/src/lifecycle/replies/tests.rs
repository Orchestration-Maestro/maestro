//! Cached faults settle each admitted native waiter once without per-reply rescans.
use super::Replies;
use std::sync::{Arc, Mutex};

#[test]
fn fault_settles_all_started_waiters_once_and_rejects_later_admission() {
    for count in [32, 64] {
        let owner = Replies::default();
        let received = Arc::new(Mutex::new(Vec::new()));
        let completed = Arc::new(Mutex::new(Vec::new()));
        let mut tokens = Vec::new();
        for id in 0..count {
            let received = received.clone();
            let token = owner
                .register::<u32>(move |result| received.lock().unwrap().push((id, result)))
                .unwrap();
            let completed = completed.clone();
            token.before(move || completed.lock().unwrap().push(id));
            tokens.push(token);
        }
        drop(tokens);
        assert_eq!(owner.state().pending.len(), count);
        assert_eq!(owner.fault("attributed trap".into()), "attributed trap");
        assert!(owner.state().pending.is_empty());
        assert_eq!(completed.lock().unwrap().len(), count);
        assert_eq!(received.lock().unwrap().len(), count);
        for (id, result) in received.lock().unwrap().iter() {
            assert!(*id < count);
            assert_eq!(result.as_ref().unwrap_err(), "attributed trap");
        }
        assert_eq!(owner.fault("second trap".into()), "attributed trap");
        assert_eq!(received.lock().unwrap().len(), count);
        let later = Arc::new(Mutex::new(None));
        let observed = later.clone();
        assert!(
            owner
                .register::<u32>(move |result| *observed.lock().unwrap() = Some(result))
                .is_err()
        );
        assert_eq!(
            later
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap_err(),
            "attributed trap"
        );
    }
}

#[test]
fn task_completion_precedes_native_settlement_without_holding_the_owner_lock() {
    let owner = Replies::default();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let received = trace.clone();
    let reader = owner.clone();
    let reply = owner
        .register(move |result: Result<u32, String>| {
            assert_eq!(result.unwrap(), 7);
            assert!(reader.state().pending.is_empty());
            received.lock().unwrap().push("reply");
        })
        .unwrap();
    let completed = trace.clone();
    reply.before(move || completed.lock().unwrap().push("finished"));
    reply.send(Ok(7)).unwrap();
    assert_eq!(*trace.lock().unwrap(), ["finished", "reply"]);
    assert!(owner.state().pending.is_empty());
}

#[test]
fn cancelled_unstarted_call_removes_its_pending_sender() {
    let owner = Replies::default();
    let (sender, receiver) = std::sync::mpsc::channel();
    let token = owner
        .register::<()>(move |result| sender.send(result).unwrap())
        .unwrap();
    drop(token);
    assert!(owner.state().pending.is_empty());
    assert_eq!(
        receiver.recv().unwrap().unwrap_err(),
        "extension guest reply ended"
    );
}
