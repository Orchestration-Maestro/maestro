//! Background work handed to the host's event-tail executor: it runs until it ends, and
//! dropping its task stops it.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use serde_json::json;
use tokio::sync::oneshot;

use super::controlled::Controlled;
use super::observed::Observed;
use crate::component_adapter::{Capabilities, Exports};
use crate::{
    Background, BackgroundTask, Extension, ExtensionAPI, ExtensionEventResult, ExtensionFuture,
};

/// The task the witness keeps while its job runs.
type Slot = Rc<RefCell<Option<BackgroundTask>>>;

/// Starts a job that waits on the host and then emits, keeping its task in `slot`.
fn start_job(
    api: ExtensionAPI,
    background: Background,
    slot: Slot,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let events = api.events();
        let work = Box::pin(async move {
            api.native_io()
                .sleep(Duration::from_millis(5), None)
                .await?;
            events.emit("witness.done", json!("ran")).await
        });
        slot.replace(Some(background.spawn(work).await?));
        Ok(None)
    })
}

/// Starts the job on `agent_start` and drops its task on `agent_end`.
struct Witness;
impl Extension for Witness {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move {
            let background = Background::new(api.events())?;
            let slot = Slot::default();
            let (owner, started) = (api.clone(), Rc::clone(&slot));
            api.on(
                "agent_start",
                Rc::new(move |_, _| {
                    start_job(owner.clone(), background.clone(), Rc::clone(&started))
                }),
            )?;
            api.on(
                "agent_end",
                Rc::new(move |_, _| {
                    *slot.borrow_mut() = None;
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
        })
    }
}

/// The controlled host and the adapter that delivers its callbacks.
struct Driver {
    /// Host observations and native wait gates.
    host: Controlled,
    /// The public adapter that delivers callbacks.
    exports: Exports<Controlled>,
}

impl Driver {
    /// Registers the witness.
    fn new() -> Result<Self, String> {
        let (host, exports) = super::block_on(super::example_support::start::<Witness>())?;
        Ok(Self { host, exports })
    }

    /// Delivers one lifecycle event and returns the handler's failure, if any.
    fn event(&self, name: &str) -> Result<(), String> {
        let key = self.host.observed().callback(&format!("event {name}"))?;
        let event = super::documents::document(name).to_string();
        let outcome = super::block_on(self.exports.invoke_event(
            key,
            event,
            Capabilities {
                ctx: self.host.context("/work"),
                signal: None,
            },
        ))?;
        let answer = super::documents::Answer::of(super::guest_family::delivery(outcome))?;
        match answer.ended {
            super::documents::Ended::Returned(None) => Ok(()),
            super::documents::Ended::Failed(error) => Err(error),
            other @ super::documents::Ended::Returned(_) => {
                Err(format!("unexpected event answer: {other:?}"))
            }
        }
    }

    /// Starts the job and waits until its native wait holds.
    fn start(&self) -> Result<(ExtensionFuture<'static, ()>, oneshot::Sender<()>), String> {
        let (gate, entered, open) = super::observed::gate();
        self.host.observe().hold_next_idle(gate);
        self.event("agent_start")?;
        let key = *self.host.observed().bus.tails.last().ok_or("no tail")?;
        let mut tail: ExtensionFuture<'static, ()> =
            Box::pin(Exports::<Controlled>::invoke_tail(key));
        assert!(poll(&mut tail).is_pending());
        super::block_on(entered).unwrap();
        Ok((tail, open))
    }

    /// The values emitted on the job's own channel.
    fn done(&self) -> Vec<serde_json::Value> {
        done(&self.host.observed())
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let listeners = self.host.observed().bus.listeners.clone();
        let mut names: Vec<_> = listeners
            .iter()
            .map(|(_, id)| format!("event bus {id}"))
            .collect();
        names.extend(["event agent_start".to_owned(), "event agent_end".to_owned()]);
        let tails = self.host.observed().bus.tails.len();
        names.extend(std::iter::repeat_n("bus tail".to_owned(), tails));
        super::example_support::release_all(&self.host, &names);
    }
}

/// Polls once without a watchdog or a wall-clock dependency.
fn poll(tail: &mut ExtensionFuture<'static, ()>) -> Poll<Result<(), String>> {
    tail.as_mut().poll(&mut Context::from_waker(Waker::noop()))
}

/// The values emitted on the job's own channel.
fn done(observed: &Observed) -> Vec<serde_json::Value> {
    observed
        .bus
        .emissions
        .iter()
        .filter(|(channel, _)| channel == "witness.done")
        .map(|(_, value)| value.clone())
        .collect()
}

/// The host polls the job on its own tail after the starting callback returned.
#[test]
fn background_task_runs_on_the_host_tail() -> Result<(), String> {
    let driver = Driver::new()?;
    let (mut tail, open) = driver.start()?;
    assert!(driver.done().is_empty(), "the job waits on its own");
    open.send(()).unwrap();
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    assert_eq!(driver.done(), [json!("ran")]);
    Ok(())
}

/// Dropping the task drops the job and its pending wait, and settles its tail.
#[test]
fn background_task_stops_when_dropped() -> Result<(), String> {
    let driver = Driver::new()?;
    let (mut tail, open) = driver.start()?;
    driver.event("agent_end")?;
    assert!(open.is_closed(), "the pending wait was dropped");
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    assert!(driver.done().is_empty());
    Ok(())
}

/// A refused announcement fails the start and leaves no job waiting for a later one.
#[test]
fn background_task_reports_a_refused_announcement() -> Result<(), String> {
    let driver = Driver::new()?;
    driver.host.observe().bus.emission_error = Some("announcement refused".into());
    assert_eq!(
        driver.event("agent_start"),
        Err("announcement refused".into())
    );
    driver.host.observe().bus.emission_error = None;
    let (mut tail, open) = driver.start()?;
    open.send(()).unwrap();
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    assert_eq!(driver.done(), [json!("ran")]);
    assert_eq!(driver.host.observed().bus.tails.len(), 1);
    Ok(())
}
