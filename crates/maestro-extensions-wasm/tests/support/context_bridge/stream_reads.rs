//! Competing-reader and terminal-result observations through the authored facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventStream, ExtensionResult};
use serde_json::{Value, json};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Waker},
};

/// Require an observation to remain pending after one real poll.
pub fn pending<F: Future + ?Sized>(future: Pin<&mut F>) -> ExtensionResult<()> {
    if future
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending()
    {
        Ok(())
    } else {
        Err("stream observation settled before publication".into())
    }
}

/// Queued duplicate events and pending readers retain admission order through EOF.
pub async fn readers() -> ExtensionResult<Value> {
    let stream = EventStream::new(|_: &(String, i32)| false, |event| event.1)?;
    for n in [1, 2] {
        stream.push(("duplicate".into(), n))?;
    }
    stream.end(None)?;
    let mut queued = Vec::new();
    while let Some(event) = stream.next().await? {
        queued.push(event);
    }
    let stream = EventStream::new(|_: &i32| false, |n| *n)?;
    let mut first = stream.next();
    let mut second = stream.next();
    let mut third = stream.next();
    pending(first.as_mut())?;
    pending(second.as_mut())?;
    pending(third.as_mut())?;
    stream.push(7)?;
    stream.push(8)?;
    stream.end(None)?;
    Ok(
        json!({"queued":queued,"pending":[first.await?,second.await?,third.await?],"eof":stream.next().await?.is_none()}),
    )
}

/// A terminal push alone completes every other already-pending next operation.
pub async fn terminal_readers() -> ExtensionResult<Value> {
    let stream = EventStream::new(|n: &i32| *n < 0, |n| *n)?;
    let mut first = stream.next();
    let mut second = stream.next();
    let mut third = stream.next();
    pending(first.as_mut())?;
    pending(second.as_mut())?;
    pending(third.as_mut())?;
    stream.push(-1)?;
    Ok(json!([
        first.await?,
        second.await?,
        third.await?,
        stream.next().await?
    ]))
}

/// Missing results stay pending; every supplied JSON root remains an explicit result.
pub async fn optional_results() -> ExtensionResult<Value> {
    let mut observed = Vec::new();
    for selected in [
        Value::Null,
        json!(false),
        json!(0),
        json!(""),
        json!({}),
        json!([]),
    ] {
        let stream = EventStream::new(|_: &Value| false, Clone::clone)?;
        let mut result = stream.result();
        pending(result.as_mut())?;
        stream.end(None)?;
        if stream.next().await?.is_some() {
            return Err("end admitted an event".into());
        }
        pending(result.as_mut())?;
        stream.end(Some(selected))?;
        let first = result.await?;
        stream.end(Some(json!("replacement")))?;
        let repeated = stream.result().await?;
        observed.push(json!({"value": first, "sameOwner":std::rc::Rc::ptr_eq(&first,&repeated)}));
    }
    Ok(json!(observed))
}

/// The first terminal event wins before and after draining its queued events.
pub async fn first_terminal() -> ExtensionResult<Value> {
    use std::{cell::Cell, rc::Rc};
    let classified = Rc::new(Cell::new(0));
    let extracted = Rc::new(Cell::new(0));
    let classify = classified.clone();
    let extract = extracted.clone();
    let stream = EventStream::new(
        move |n: &i32| {
            classify.set(classify.get() + 1);
            *n < 0
        },
        move |n| {
            extract.set(extract.get() + 1);
            *n
        },
    )?;
    for n in [1, -1, -2, 3] {
        stream.push(n)?;
    }
    stream.end(Some(99))?;
    let before = stream.result().await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await? {
        events.push(event);
    }
    let after = stream.result().await?;
    Ok(
        json!({"events":events,"before":before,"after":after,"sameOwner":Rc::ptr_eq(&before,&after),"classified":classified.get(),"extracted":extracted.get()}),
    )
}
