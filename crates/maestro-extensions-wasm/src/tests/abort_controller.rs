//! Caller cancellation through the public controller and signal interfaces.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::AbortController;

#[test]
fn abort_controller_shares_state_and_removes_listeners() -> Result<(), String> {
    let controller = AbortController::new();
    let signal = controller.signal();
    let seen = Rc::new(RefCell::new(Vec::new()));
    assert!(!signal.aborted());
    for label in ["first", "removed", "last"] {
        let seen = Rc::clone(&seen);
        let signal_in_callback = signal.clone();
        let subscription = signal.subscribe(Rc::new(move |emitter| {
            assert!(signal_in_callback.aborted());
            emitter.emit("unused", serde_json::Value::Null)?;
            seen.borrow_mut().push(label);
            Ok(())
        }))?;
        if label == "removed" {
            subscription.unsubscribe()?;
            subscription.unsubscribe()?;
        }
    }
    let controllers = [controller.clone(), controller];
    controllers[0].abort()?;
    assert!(signal.aborted());
    assert!(controllers[1].signal().aborted());
    assert_eq!(*seen.borrow(), ["first", "last"]);
    let seen_late = Rc::clone(&seen);
    signal.subscribe(Rc::new(move |_| {
        seen_late.borrow_mut().push("late");
        Ok(())
    }))?;
    controllers[1].abort()?;
    drop(controllers);
    assert!(signal.aborted());
    assert_eq!(*seen.borrow(), ["first", "last"]);
    let failed = AbortController::default();
    failed
        .signal()
        .subscribe(Rc::new(|_| Err("listener refused".into())))?;
    assert_eq!(failed.abort(), Err("listener refused".into()));
    assert!(failed.signal().aborted());
    failed.abort()?;
    let subscription = AbortController::new()
        .signal()
        .subscribe(Rc::new(|_| Ok(())))?;
    subscription.unsubscribe()?;
    Ok(())
}
