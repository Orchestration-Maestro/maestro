//! Synchronous callbacks may reenter their original stream outside identity-table borrows.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventStream, ExtensionResult};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// Non-admitting reference to the originally created facade.
type Target = Weak<RefCell<Option<EventStream<i32>>>>;
/// Push from inside a callback, retaining a recoverable failure for its caller.
fn push(target: &Target, value: i32, error: &RefCell<Option<String>>) {
    let result = target
        .upgrade()
        .ok_or_else(|| "stream target ended".into())
        .and_then(|target| {
            target
                .borrow()
                .as_ref()
                .ok_or_else(|| "stream target absent".into())
                .and_then(|stream| stream.push(value))
        });
    if let Err(message) = result {
        *error.borrow_mut() = Some(message);
    }
}

/// Compare synchronous reentry against an ordinary non-reentrant control.
pub async fn observe(reenter: bool) -> ExtensionResult<Value> {
    let target = Rc::new(RefCell::new(None::<EventStream<i32>>));
    let classify_target = Rc::downgrade(&target);
    let extract_target = Rc::downgrade(&target);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let classify_calls = calls.clone();
    let extract_calls = calls.clone();
    let error = Rc::new(RefCell::new(None));
    let classify_error = error.clone();
    let extract_error = error.clone();
    let stream = EventStream::new(
        move |n: &i32| {
            classify_calls.borrow_mut().push(format!("classify:{n}"));
            if reenter && *n < 0 {
                push(&classify_target, 2, &classify_error);
            }
            *n < 0
        },
        move |n| {
            extract_calls.borrow_mut().push(format!("extract:{n}"));
            if reenter {
                push(&extract_target, 99, &extract_error);
            }
            *n
        },
    )?;
    *target.borrow_mut() = Some(stream.clone());
    stream.push(-1)?;
    let synchronous_calls = calls.borrow().clone();
    if let Some(error) = error.borrow_mut().take() {
        return Err(error);
    }
    let result = stream.result().await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await? {
        events.push(event);
    }
    Ok(json!({"calls":synchronous_calls,"events":events,"result":result}))
}
