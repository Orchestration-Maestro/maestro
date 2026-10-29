//! The shipped title animation driven through lifecycle registrations and native wait gates.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::task::{Context, Poll, Waker};

use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::controlled::Controlled;
use super::example_support::start;
use super::observed::Observed;
use super::ui_host::receipts;
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[expect(
    clippy::duplicate_mod,
    reason = "Each shipped example is its own crate; only this harness loads several examples."
)]
#[path = "../../../../examples/extensions/titlebar_spinner.rs"]
mod titlebar_spinner;

/// The same factory the component exports.
struct TitlebarSpinner;
impl Extension for TitlebarSpinner {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        titlebar_spinner::factory(api)
    }
}

/// The controlled host and its registered lifecycle callbacks.
struct Driver {
    /// Host observations and native wait gates.
    host: Controlled,
    /// The public adapter that delivers callbacks.
    exports: Exports<Controlled>,
    /// The bus listener kept for release even after shutdown unsubscribes it.
    listener: u32,
}

impl Driver {
    /// Registers the example without running an event.
    fn new() -> Result<Self, String> {
        let (host, exports) = super::block_on(start::<TitlebarSpinner>())?;
        host.observe().actions.raw = Some(json!("project").to_string());
        assert_eq!(host.observed().bus.listeners.len(), 1);
        assert!(titles(&host.observed()).is_empty());
        let listener = host.observed().bus.listeners[0].1;
        Ok(Self {
            host,
            exports,
            listener,
        })
    }

    /// Delivers one lifecycle event with a supplied working directory.
    fn event(&self, name: &str, cwd: &str) -> Result<(), String> {
        let key = self.host.observed().callback(&format!("event {name}"))?;
        let event = super::documents::document(name).to_string();
        let outcome = super::block_on(self.exports.invoke_event(
            key,
            event,
            Capabilities {
                ctx: self.host.context(cwd),
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

    /// The most recently announced host-owned animation task.
    fn tail(&self) -> ExtensionFuture<'static, ()> {
        let key = *self
            .host
            .observed()
            .bus
            .tails
            .last()
            .expect("announced animation");
        Box::pin(Exports::<Controlled>::invoke_tail(key))
    }

    /// Supplies the live session name without changing a running animation.
    fn name(&self, name: Option<&str>) {
        self.host.observe().actions.value = name.map_or(Value::Null, |name| json!(name));
    }
}

/// The literal title operands the host received.
fn titles(observed: &Observed) -> Vec<String> {
    receipts(&observed.transcript)
        .iter()
        .map(|receipt| {
            assert_eq!(receipt.method, "setTitle");
            receipt.operands[0].as_str().unwrap().to_owned()
        })
        .collect()
}

/// Holds exactly the next native wait and exposes its entry and cancellation.
fn hold(observed: &mut Observed) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (gate, entered, open) = super::observed::gate();
    observed.hold_next_idle(gate);
    (entered, open)
}

/// Polls once without a watchdog or a wall-clock dependency.
fn poll(tail: &mut ExtensionFuture<'static, ()>) -> Poll<Result<(), String>> {
    tail.as_mut().poll(&mut Context::from_waker(Waker::noop()))
}

/// All ten authored frames, wraparound, live renaming and the delayed first frame.
#[test]
fn titlebar_spinner_cadence_and_live_title() -> Result<(), String> {
    let driver = Driver::new()?;
    driver.name(Some("named 雪"));
    let (entered, mut open) = hold(&mut driver.host.observe());
    driver.event("agent_start", "/work/project/")?;
    assert_eq!(
        titles(&driver.host.observed()),
        ["Maestro - named 雪 - project"]
    );
    let mut tail = driver.tail();
    assert!(poll(&mut tail).is_pending());
    super::block_on(entered).unwrap();
    for (index, expected) in ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "⠋"]
        .iter()
        .enumerate()
    {
        if index == 1 {
            driver.name(Some("renamed"));
        }
        if index == 2 {
            driver.name(Some(""));
        }
        let (entered, next) = hold(&mut driver.host.observe());
        open.send(()).unwrap();
        assert!(poll(&mut tail).is_pending());
        super::block_on(entered).unwrap();
        let suffix = match index {
            0 => "named 雪 - project",
            1 => "renamed - project",
            _ => "project",
        };
        assert_eq!(
            titles(&driver.host.observed()).last().unwrap(),
            &format!("{expected} Maestro - {suffix}")
        );
        open = next;
    }
    let waits: Vec<_> = driver
        .host
        .observed()
        .transcript
        .iter()
        .filter(|line| line.starts_with("native-wait"))
        .cloned()
        .collect();
    assert_eq!(waits, vec!["native-wait 0 80000000"; 12]);
    driver.event("agent_end", "/work/project")?;
    assert!(
        open.is_closed(),
        "ending must drop the native wait immediately"
    );
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    Ok(())
}

/// Restart, end and shutdown cancel the pending timer and restore the current base title.
#[test]
fn titlebar_spinner_lifecycle_cancels_pending_waits() -> Result<(), String> {
    let driver = Driver::new()?;
    for ending in ["agent_start", "agent_end", "session_shutdown"] {
        let (entered, open) = hold(&mut driver.host.observe());
        driver.event("agent_start", r"C:\work\project\")?;
        let mut tail = driver.tail();
        assert!(poll(&mut tail).is_pending());
        super::block_on(entered).unwrap();
        driver.name(Some("after"));
        driver.event(ending, r"C:\work\project\")?;
        assert!(open.is_closed(), "{ending} must drop its timer");
        assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
        assert_eq!(
            titles(&driver.host.observed()).last().unwrap(),
            "Maestro - after - project"
        );
        if ending == "agent_start" {
            let (entered, open) = hold(&mut driver.host.observe());
            let mut restarted = driver.tail();
            assert!(poll(&mut restarted).is_pending());
            super::block_on(entered).unwrap();
            let (entered, pending) = hold(&mut driver.host.observe());
            open.send(()).unwrap();
            assert!(poll(&mut restarted).is_pending());
            super::block_on(entered).unwrap();
            assert_eq!(
                titles(&driver.host.observed()).last().unwrap(),
                "⠋ Maestro - after - project"
            );
            driver.event("agent_end", "/work/project")?;
            assert!(pending.is_closed());
            assert_eq!(poll(&mut restarted), Poll::Ready(Ok(())));
        }
    }
    assert!(driver.host.observed().bus.listeners.is_empty());
    driver.host.observe().actions.raw = Some(json!("").to_string());
    driver.event("session_shutdown", "/")?;
    assert_eq!(
        titles(&driver.host.observed()).last().unwrap(),
        "Maestro - after - "
    );
    Ok(())
}

/// The title uses the host's basename, preserving a literal POSIX backslash.
#[test]
fn titlebar_spinner_uses_host_basename() -> Result<(), String> {
    let driver = Driver::new()?;
    driver.host.observe().actions.raw = Some(json!(r"a\b").to_string());
    driver.event("agent_end", r"/work/a\b")?;
    assert_eq!(titles(&driver.host.observed()), [r"Maestro - a\b"]);
    Ok(())
}

/// Root, empty and trailing directory spellings restore without inventing a directory name.
#[test]
fn titlebar_spinner_base_titles_and_rejections() -> Result<(), String> {
    let driver = Driver::new()?;
    for (cwd, expected) in [
        ("/", "Maestro - "),
        (r"C:\", "Maestro - "),
        ("", "Maestro - "),
        ("/a/b///", "Maestro - b"),
        (r"C:\a\b\", "Maestro - b"),
    ] {
        driver.host.observe().actions.raw =
            Some(json!(expected.trim_start_matches("Maestro - ")).to_string());
        driver.event("agent_end", cwd)?;
        assert_eq!(titles(&driver.host.observed()).last().unwrap(), expected);
    }
    driver
        .host
        .observe()
        .ui
        .control(&json!({"error": "title refused"}).to_string());
    assert_eq!(
        driver.event("agent_start", "/a/b"),
        Err("title refused".into())
    );
    assert!(driver.host.observed().bus.tails.is_empty());
    driver
        .host
        .observe()
        .ui
        .control(&json!({"error": null}).to_string());
    driver.host.observe().actions.error = Some("name refused".into());
    assert_eq!(
        driver.event("agent_start", "/a/b"),
        Err("name refused".into())
    );
    driver.host.observe().actions.error = None;
    driver.host.observe().bus.emission_error = Some("tail refused".into());
    assert_eq!(
        driver.event("agent_start", "/a/b"),
        Err("tail refused".into())
    );
    driver.host.observe().bus.emission_error = None;
    driver.event("session_shutdown", "/a/b")?;
    Ok(())
}

impl Drop for Driver {
    fn drop(&mut self) {
        let mut names = vec![
            format!("event bus {}", self.listener),
            "event agent_start".to_owned(),
            "event agent_end".to_owned(),
            "event session_shutdown".to_owned(),
        ];
        names.extend(std::iter::repeat_n(
            "bus tail".to_owned(),
            self.host.observed().bus.tails.len(),
        ));
        super::example_support::release_all(&self.host, &names);
    }
}

/// Delivers a lifecycle callback to the built component and checks its no-result answer.
async fn component_event(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    exports: &super::host_family::guest::Guest,
    key: u32,
    event: &str,
    context: wasmtime::component::Resource<super::host::Ordinary>,
) -> wasmtime::Result<()> {
    let outcome = exports
        .call_invoke_event(
            accessor,
            super::host::borrow(key),
            super::documents::document(event).to_string(),
            super::host_family::guest::Capabilities {
                ctx: context,
                signal: None,
            },
        )
        .await?
        .map_err(wasmtime::Error::msg)?;
    let answer = super::documents::Answer::of(super::host_family::delivery(outcome))
        .map_err(wasmtime::Error::msg)?;
    assert_eq!(answer.ended, super::documents::Ended::Returned(None));
    Ok(())
}

/// Runs the component's animation concurrently with the lifecycle event that cancels it.
fn component_lifecycle(harness: &mut super::host::Harness, ending: &str) -> wasmtime::Result<()> {
    harness.store.data_mut().observed.actions.raw = Some(json!("project").to_string());
    let start = harness.callback("event agent_start")?;
    let end = harness.callback(&format!("event {ending}"))?;
    let context = harness.ordinary("/controlled/project/")?;
    let stopped_context = harness.ordinary("/controlled/project/")?;
    let exports = harness.exports.clone();
    super::block_on(harness.store.run_concurrent(async |accessor| {
        component_event(accessor, &exports, start, "agent_start", context).await?;
        let (entered, open) = accessor.with(|mut a| hold(&mut a.get().observed));
        accessor.with(|mut a| assert_eq!(titles(&a.get().observed), ["Maestro - project"]));
        let key = accessor.with(|mut a| *a.get().observed.bus.tails.last().unwrap());
        let tail = exports.call_invoke_tail(accessor, super::host::borrow(key));
        let control = async {
            entered.await.unwrap();
            let (entered, pending) = accessor.with(|mut a| hold(&mut a.get().observed));
            open.send(()).unwrap();
            entered.await.unwrap();
            accessor.with(|mut a| {
                assert_eq!(
                    titles(&a.get().observed).last().unwrap(),
                    "⠋ Maestro - project"
                );
                a.get().observed.actions.value = json!("renamed");
            });
            component_event(accessor, &exports, end, ending, stopped_context)
                .await
                .unwrap();
            assert!(pending.is_closed(), "{ending} dropped its pending wait");
        };
        super::join::alongside(tail, control)
            .await?
            .map_err(wasmtime::Error::msg)?;
        Ok::<_, wasmtime::Error>(())
    }))??;
    assert_eq!(
        titles(&harness.store.data().observed).last().unwrap(),
        "Maestro - renamed - project"
    );
    Ok(())
}

/// The built component emits a delayed frame and cancels native waits at every stop seam.
#[test]
fn titlebar_spinner_component_runs_lifecycle() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "titlebar_spinner")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    for ending in ["agent_start", "agent_end", "session_shutdown"] {
        let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
            .map_err(|error| error.to_string())?;
        let listener = harness.observed().bus.listeners[0].1;
        component_lifecycle(&mut harness, ending).map_err(|error| error.to_string())?;
        let shutdown = harness
            .callback("event session_shutdown")
            .map_err(|error| error.to_string())?;
        let context = harness
            .ordinary("/controlled/project")
            .map_err(|error| error.to_string())?;
        let exports = harness.exports.clone();
        super::block_on(harness.store.run_concurrent(async |accessor| {
            component_event(accessor, &exports, shutdown, "session_shutdown", context).await
        }))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        let mut keys = vec![listener];
        for event in ["agent_start", "agent_end", "session_shutdown"] {
            keys.push(
                harness
                    .callback(&format!("event {event}"))
                    .map_err(|error| error.to_string())?,
            );
        }
        keys.extend(harness.observed().bus.tails.iter().copied());
        for key in keys {
            super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
        }
        assert_eq!(
            harness.observed().live,
            0,
            "{ending} released its resources"
        );
    }
    Ok(())
}
