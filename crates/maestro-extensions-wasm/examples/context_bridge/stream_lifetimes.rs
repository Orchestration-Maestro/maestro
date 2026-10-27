//! Independent stream producers, observations and retained values release separately.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    EventStream, ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionResult,
};
use std::{cell::RefCell, rc::Rc};

/// Reports actual destruction through the native application action boundary.
pub(super) struct Released {
    /// Original facade, retained independently of the initiating command context.
    api: ExtensionAPI,
    /// Distinguishable event, result or callback capture.
    label: &'static str,
}
impl Released {
    /// Retain a nonserializable local value with an observable destructor.
    pub(super) fn new(api: &ExtensionAPI, label: &'static str) -> Self {
        Self {
            api: api.clone(),
            label,
        }
    }
}
impl Drop for Released {
    fn drop(&mut self) {
        // New callback admission detects destruction under the callback-table borrow.
        let reentry = EventStream::new(|(): &()| false, |()| ());
        let label = if reentry.is_ok() {
            self.label
        } else {
            "destructor-reentry-failed"
        };
        let _ = self.api.set_session_name(label);
    }
}

/// Only test operands survive between separate compiled-guest command invocations.
#[derive(Default)]
struct Kept {
    /// Independently owned producer and observation source.
    stream: Option<EventStream<Released, Released>>,
    /// Already-polled result observations that can be dropped without cancellation.
    observers: Vec<ExtensionFuture<'static, Rc<Released>>>,
    /// Repeated observations of the same non-Clone result.
    results: Vec<Rc<Released>>,
}
thread_local! {
    /// Fixture operands only; no production stream state or buffering is recreated here.
    static KEPT: RefCell<Kept> = RefCell::default();
}

/// Clone the original stream without borrowing fixture storage across a host call.
fn stream() -> ExtensionResult<EventStream<Released, Released>> {
    KEPT.with_borrow(|kept| {
        kept.stream
            .clone()
            .ok_or_else(|| "missing retained stream".into())
    })
}

/// Start independent pending observers before returning from their initiating command.
fn create(api: &ExtensionAPI) -> ExtensionResult<()> {
    let classify = Released::new(api, "classifier");
    let extract = Released::new(api, "extractor");
    let stream = EventStream::new(
        move |event: &Released| {
            let _ = &classify;
            event.label == "terminal"
        },
        move |_| {
            let _ = &extract;
            Released::new(&extract.api, "result")
        },
    )?;
    stream.push(Released::new(api, "queued"))?;
    let mut observers = Vec::new();
    for _ in 0..3 {
        let mut result = stream.result();
        super::streams::reads::pending(result.as_mut())?;
        observers.push(result);
    }
    let replaced = KEPT.replace(Kept {
        stream: Some(stream),
        observers,
        results: Vec::new(),
    });
    drop(replaced);
    Ok(())
}

/// Observe results independently, then drain events without losing their shared owner.
async fn observe(aborted: bool) -> ExtensionResult<()> {
    let stream = stream()?;
    let first = stream.result().await?;
    let repeated = stream.result().await?;
    let expected = if aborted { "aborted-result" } else { "result" };
    if first.label != expected || !Rc::ptr_eq(&first, &repeated) {
        return Err("result owner changed".into());
    }
    let observers = KEPT.with_borrow_mut(|kept| std::mem::take(&mut kept.observers));
    for observer in observers {
        if !Rc::ptr_eq(&first, &observer.await?) {
            return Err("surviving observer lost its result owner".into());
        }
    }
    let expected_events: &[&str] = if aborted {
        &["queued"]
    } else {
        &["queued", "terminal"]
    };
    for &expected in expected_events {
        let event = stream.next().await?.ok_or("missing retained event")?;
        if event.label != expected {
            return Err("retained event order changed".into());
        }
    }
    if stream.next().await?.is_some() {
        return Err("terminal stream admitted a later event".into());
    }
    KEPT.with_borrow_mut(|kept| kept.results.extend([first, repeated]));
    Ok(())
}

/// Run an independently admitted producer under the caller's original cancellation signal.
async fn produce(api: &ExtensionAPI, context: &ExtensionCommandContext) -> ExtensionResult<()> {
    let stream = stream()?;
    let signal = context.signal()?.ok_or("missing producer signal")?;
    let (cancelled, source) = (api.clone(), stream.clone());
    let subscription = signal.subscribe(Rc::new(move |_| {
        source.end(Some(Released::new(&cancelled, "aborted-result")))?;
        cancelled.set_session_name("producer-aborted")
    }))?;
    context.wait_for_idle().await?;
    if !signal.aborted() {
        stream.push(Released::new(api, "terminal"))?;
    }
    subscription.unsubscribe()?;
    api.set_session_name("producer-complete")
}

/// Keep a producer pending while other invocations abandon their observation futures.
pub async fn command(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    action: &str,
) -> ExtensionResult<String> {
    match action {
        "create" => create(api)?,
        "produce" => produce(api, context).await?,
        "drop-one" => {
            let observer = KEPT.with_borrow_mut(|kept| kept.observers.pop());
            drop(observer.ok_or("missing first observer")?);
        }
        "drop-all" => {
            let observers = KEPT.with_borrow_mut(|kept| std::mem::take(&mut kept.observers));
            if observers.len() != 2 {
                return Err("missing remaining observers".into());
            }
            drop(observers);
        }
        "observe" => observe(false).await?,
        "observe-aborted" => observe(true).await?,
        "barrier" => {}
        "release-stream" => {
            let stream = KEPT.with_borrow_mut(|kept| kept.stream.take());
            drop(stream.ok_or("missing final stream alias")?);
        }
        "release-result" => {
            let result = KEPT.with_borrow_mut(|kept| kept.results.pop());
            let result = result.ok_or("missing result alias")?;
            if !matches!(result.label, "result" | "aborted-result") {
                return Err("result did not survive its stream".into());
            }
            drop(result);
        }
        _ => return Err("unknown stream lifetime observation".into()),
    }
    Ok(action.to_owned())
}
