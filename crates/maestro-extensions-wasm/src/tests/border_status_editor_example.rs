//! The shipped border status editor's spinner, driven through its lifecycle hooks, the editor
//! the host shows and native wait gates.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use serde_json::json;
use tokio::sync::oneshot;

use super::controlled::Controlled;
use super::observed::Observed;
use super::renderer_host::{Call, Reply};
use super::theme_host::{Palette, Reply as Answer};
use crate::component_adapter::{Capabilities, Exports, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/border_status_editor.rs"]
mod border_status_editor;

/// The same factory the component exports.
struct BorderStatus;
impl Extension for BorderStatus {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        border_status_editor::factory(api)
    }
}

/// The ten frames in display order.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The theme answers the editor reads while it renders: each spinner frame and the status.
fn palette() -> Palette {
    let fg = |color: &str, text: &str, answer: String| Answer {
        method: "fg".to_owned(),
        operands: json!([color, text]),
        answer: json!({ "ok": answer }),
    };
    let mut replies: Vec<Answer> = FRAMES
        .iter()
        .map(|frame| fg("accent", &format!(" {frame} "), format!("[{frame}]")))
        .collect();
    replies.push(fg("muted", " no model · high ", "[left]".to_owned()));
    replies.push(fg("muted", " ctx ? · /work ", "[right]".to_owned()));
    Palette {
        name: None,
        source_path: None,
        source_info: None,
        mode: "truecolor".to_owned(),
        replies,
    }
}

/// The controlled host and the adapter that delivers its callbacks.
struct Driver {
    /// Host observations, the shown editor and native wait gates.
    host: Controlled,
    /// The public adapter that delivers callbacks.
    exports: Exports<Controlled>,
}

impl Driver {
    /// Registers the example, starts the session and finishes its branch read.
    fn new() -> Result<Self, String> {
        let host = Controlled::default();
        let builder = crate::component_adapter::Builder(host.clone());
        crate::component_adapter::install(Rc::new(builder));
        let exports = Exports::new(host.clone());
        super::block_on(exports.start::<BorderStatus>())?;
        host.observe().actions.value = json!({
            "visible_width": 0,
            "paths.shorten_path": "/work",
            "getThinkingLevel": "high",
        });
        host.observe().themes.publish("dark", palette());
        let driver = Self { host, exports };
        driver.event("session_start")?;
        assert_eq!(poll(&mut driver.tail()), Poll::Ready(Ok(())));
        Ok(driver)
    }

    /// Delivers one lifecycle event and checks the handler returned no result.
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

    /// The most recently announced host-owned work.
    fn tail(&self) -> ExtensionFuture<'static, ()> {
        let key = *self.host.observed().bus.tails.last().expect("announced");
        Box::pin(Exports::<Controlled>::invoke_tail(key))
    }

    /// The top border the shown editor renders 40 columns wide.
    fn top(&self) -> Result<String, String> {
        match self.host.editor_component(Call::Render(40))? {
            Reply::Lines(rows) => Ok(rows[0].clone()),
            other => Err(format!("unexpected render answer: {other:?}")),
        }
    }

    /// Checks that the top border shows `frame`, or no spinner for `None`.
    fn shows(&self, frame: Option<&str>) -> Result<(), String> {
        let top = self.top()?;
        let shown: Vec<_> = FRAMES
            .iter()
            .filter(|candidate| top.contains(&format!("[{candidate}]")))
            .copied()
            .collect();
        assert_eq!(shown, Vec::from_iter(frame), "{top:?}");
        Ok(())
    }

    /// Starts working and waits until the first 80 ms wait holds.
    fn start(&self) -> Result<(ExtensionFuture<'static, ()>, oneshot::Sender<()>), String> {
        let (entered, open) = hold(&mut self.host.observe());
        self.event("agent_start")?;
        let mut tail = self.tail();
        assert!(poll(&mut tail).is_pending());
        super::block_on(entered).unwrap();
        Ok((tail, open))
    }

    /// Ends the held wait, then waits until the next one holds.
    fn step(
        &self,
        tail: &mut ExtensionFuture<'static, ()>,
        open: oneshot::Sender<()>,
    ) -> oneshot::Sender<()> {
        let (entered, next) = hold(&mut self.host.observe());
        open.send(()).unwrap();
        assert!(poll(tail).is_pending());
        super::block_on(entered).unwrap();
        next
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let shown = self.host.observe().ui.editor.take();
        if let Some(slot) = shown {
            release(slot.component);
            release(slot.editor);
            release(slot.factory);
        }
        self.host.release_footer();
        let mut names = vec![
            format!("event bus {}", self.host.observed().bus.listeners[0].1),
            "event agent_start".to_owned(),
            "event agent_end".to_owned(),
            "event session_shutdown".to_owned(),
            "event session_start".to_owned(),
        ];
        let tails = self.host.observed().bus.tails.len();
        names.extend(std::iter::repeat_n("bus tail".to_owned(), tails));
        super::example_support::release_all(&self.host, &names);
    }
}

/// Holds exactly the next native wait and exposes its entry and release.
fn hold(observed: &mut Observed) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (gate, entered, open) = super::observed::gate();
    observed.hold_next_idle(gate);
    (entered, open)
}

/// Polls once without a watchdog or a wall-clock dependency.
fn poll(tail: &mut ExtensionFuture<'static, ()>) -> Poll<Result<(), String>> {
    tail.as_mut().poll(&mut Context::from_waker(Waker::noop()))
}

/// The transcript lines starting with `prefix`.
fn lines(observed: &Observed, prefix: &str) -> Vec<String> {
    observed
        .transcript
        .iter()
        .filter(|line| line.starts_with(prefix))
        .cloned()
        .collect()
}

/// Working shows the ten frames in order, one per 80 ms wait with a redraw each, wraps after
/// the last, and the end of work drops the pending wait and clears the spinner.
#[test]
fn border_status_editor_spinner_follows_each_wait() -> Result<(), String> {
    let driver = Driver::new()?;
    driver.shows(None)?;
    let redraws = lines(&driver.host.observed(), "writer requestRender").len();
    let (mut tail, mut open) = driver.start()?;
    driver.shows(Some("⠋"))?;
    for frame in FRAMES.iter().skip(1).chain(&FRAMES[..2]) {
        open = driver.step(&mut tail, open);
        driver.shows(Some(frame))?;
    }
    let observed = driver.host.observed();
    assert_eq!(
        lines(&observed, "native-wait"),
        vec!["native-wait 0 80000000"; 12]
    );
    let requested = lines(&observed, "writer requestRender");
    assert_eq!(requested[redraws..], vec!["writer requestRender false"; 12]);
    drop(observed);
    driver.event("agent_end")?;
    assert!(open.is_closed(), "ending drops the pending wait");
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    driver.shows(None)?;
    Ok(())
}

/// A second start drops the running wait and continues from the frame shown.
#[test]
fn border_status_editor_restart_keeps_the_frame() -> Result<(), String> {
    let driver = Driver::new()?;
    let (mut tail, mut open) = driver.start()?;
    for _ in 0..3 {
        open = driver.step(&mut tail, open);
    }
    driver.shows(Some("⠸"))?;
    let (mut restarted, open_again) = driver.start()?;
    assert!(open.is_closed(), "restarting drops the pending wait");
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    driver.shows(Some("⠸"))?;
    let open_again = driver.step(&mut restarted, open_again);
    driver.shows(Some("⠼"))?;
    driver.event("agent_end")?;
    assert!(open_again.is_closed());
    assert_eq!(poll(&mut restarted), Poll::Ready(Ok(())));
    Ok(())
}

/// Shutdown drops the pending wait and requests no redraw, even when the session keeps
/// receiving agent events.
#[test]
fn border_status_editor_shutdown_stops_the_spinner() -> Result<(), String> {
    let driver = Driver::new()?;
    let (mut tail, open) = driver.start()?;
    let open = driver.step(&mut tail, open);
    driver.event("session_shutdown")?;
    assert!(open.is_closed(), "shutdown drops the pending wait");
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    let redraws = lines(&driver.host.observed(), "writer requestRender").len();
    driver.event("agent_end")?;
    assert_eq!(
        lines(&driver.host.observed(), "writer requestRender").len(),
        redraws,
        "no terminal after shutdown"
    );
    Ok(())
}
