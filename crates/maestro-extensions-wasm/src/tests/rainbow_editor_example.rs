//! The shipped rainbow editor driven through its session hook, the editor the host shows and
//! native wait gates.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::task::{Context, Poll, Waker};

use serde::Deserialize;
use serde_json::json;
use tokio::sync::oneshot;
use wasmtime::component::{Accessor, Resource};

use super::controlled::Controlled;
use super::observed::Observed;
use super::renderer_host::{Call, Reply};
use crate::component_adapter::{Capabilities, Exports, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/rainbow_editor.rs"]
mod rainbow_editor;

/// The same factory the component exports.
struct Rainbow;
impl Extension for Rainbow {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        rainbow_editor::factory(api)
    }
}

/// Rows and keyword answers recorded from the reference example.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// The editor's border row, then the two typed lines, before highlighting.
    lines: Vec<String>,
    /// The highlighted rows of frames 0 through 20, one full shine cycle and its wraparound.
    frames: Vec<Vec<String>>,
    /// Editor texts and whether each one starts the animation.
    keyword: Vec<(String, bool)>,
}

/// The recorded reference rows.
fn expected() -> Expected {
    serde_json::from_str(include_str!("rainbow_editor_expected.json")).unwrap()
}

/// The typed text whose rows the recorded frames highlight.
fn typed(expected: &Expected) -> String {
    format!("{}\n{}", expected.lines[1], expected.lines[2])
}

/// The controlled host and the adapter that delivers its callbacks.
struct Driver {
    /// Host observations, the shown editor and native wait gates.
    host: Controlled,
    /// The public adapter that delivers callbacks.
    exports: Exports<Controlled>,
}

impl Driver {
    /// Registers the example and installs its editor through one session start.
    fn new() -> Result<Self, String> {
        let host = Controlled::default();
        let builder = crate::component_adapter::Builder(host.clone());
        crate::component_adapter::install(std::rc::Rc::new(builder));
        let exports = Exports::new(host.clone());
        super::block_on(exports.start::<Rainbow>())?;
        assert_eq!(host.observed().bus.listeners.len(), 1);
        let driver = Self { host, exports };
        driver.session_start()?;
        Ok(driver)
    }

    /// Delivers `session_start` and checks the handler returned no result.
    fn session_start(&self) -> Result<(), String> {
        let key = self.host.observed().callback("event session_start")?;
        let event = super::documents::document("session_start").to_string();
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

    /// Delivers terminal input to the editor the host shows.
    fn input(&self, data: &str) -> Result<(), String> {
        match self.host.editor_component(Call::Input(data))? {
            Reply::Done => Ok(()),
            other => Err(format!("unexpected input answer: {other:?}")),
        }
    }

    /// The rows the shown editor renders 40 columns wide.
    fn render(&self) -> Result<Vec<String>, String> {
        match self.host.editor_component(Call::Render(40))? {
            Reply::Lines(rows) => Ok(rows),
            other => Err(format!("unexpected render answer: {other:?}")),
        }
    }

    /// The tails the example announced so far.
    fn tails(&self) -> usize {
        self.host.observed().bus.tails.len()
    }

    /// The most recently announced host-owned animation.
    fn tail(&self) -> ExtensionFuture<'static, ()> {
        let key = *self.host.observed().bus.tails.last().expect("announced");
        Box::pin(Exports::<Controlled>::invoke_tail(key))
    }

    /// The shown editor's factory, component and operation handles.
    fn shown(&self) -> (u32, u32, u32) {
        let observed = self.host.observed();
        let slot = observed.ui.editor.as_ref().expect("shown editor");
        (slot.factory, slot.component, slot.editor)
    }

    /// Releases a replaced editor and its factory, as a host does when it drops them.
    fn release_replaced(&self, (factory, component, editor): (u32, u32, u32)) {
        for name in [
            format!("editor component {component}"),
            format!("editor {editor}"),
        ] {
            let key = self.host.observe().take_registration(&name);
            release(key.expect("replaced editor registration"));
        }
        release(factory);
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
        let mut names = vec![
            format!("event bus {}", self.host.observed().bus.listeners[0].1),
            "event session_start".to_owned(),
        ];
        names.extend(std::iter::repeat_n("bus tail".to_owned(), self.tails()));
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

/// Each 60 ms frame moves the shine over ten characters, pauses ten frames, then wraps.
#[test]
fn rainbow_editor_shine_follows_reference_frames() -> Result<(), String> {
    let driver = Driver::new()?;
    let expected = expected();
    let (entered, mut open) = hold(&mut driver.host.observe());
    driver.input(&typed(&expected))?;
    let mut tail = driver.tail();
    assert!(poll(&mut tail).is_pending());
    super::block_on(entered).unwrap();
    for (frame, rows) in expected.frames.iter().enumerate() {
        assert_eq!(driver.render()?, *rows, "frame {frame}");
        let (entered, next) = hold(&mut driver.host.observe());
        open.send(()).unwrap();
        assert!(poll(&mut tail).is_pending());
        super::block_on(entered).unwrap();
        open = next;
    }
    let observed = driver.host.observed();
    assert_eq!(
        lines(&observed, "native-wait"),
        vec!["native-wait 0 60000000"; 22]
    );
    assert_eq!(
        lines(&observed, "writer requestRender"),
        vec!["writer requestRender false"; 21]
    );
    drop(observed);
    driver.input("enter")?;
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    assert!(
        open.is_closed(),
        "losing the keyword drops the pending wait"
    );
    assert_eq!(driver.tails(), 1);
    Ok(())
}

/// Only input checks the text, matching the keyword in any ASCII case.
#[test]
fn rainbow_editor_input_starts_animation_on_keyword() -> Result<(), String> {
    for (text, animates) in expected().keyword {
        let driver = Driver::new()?;
        driver
            .host
            .authored_editor(json!({ "op": "setText", "text": text }))?;
        assert_eq!(driver.tails(), 0, "{text:?} set without input");
        driver.input(" ")?;
        driver.input(" ")?;
        assert_eq!(driver.tails(), usize::from(animates), "{text:?}");
    }
    Ok(())
}

/// Losing the keyword stops the frames; typing it again resumes the cycle where it stopped.
#[test]
fn rainbow_editor_restarted_animation_keeps_its_frame() -> Result<(), String> {
    let driver = Driver::new()?;
    let expected = expected();
    let (entered, mut open) = hold(&mut driver.host.observe());
    driver.input(&typed(&expected))?;
    let mut tail = driver.tail();
    assert!(poll(&mut tail).is_pending());
    super::block_on(entered).unwrap();
    for _ in 0..3 {
        let (entered, next) = hold(&mut driver.host.observe());
        open.send(()).unwrap();
        assert!(poll(&mut tail).is_pending());
        super::block_on(entered).unwrap();
        open = next;
    }
    driver.input("enter")?;
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    assert!(open.is_closed());
    assert_eq!(driver.render()?, ["<borderMuted>edit|40", ""]);
    driver.input(&typed(&expected))?;
    assert_eq!(driver.render()?, expected.frames[3]);
    assert_eq!(driver.tails(), 2);
    let mut restarted = driver.tail();
    let (entered, open) = hold(&mut driver.host.observe());
    assert!(poll(&mut restarted).is_pending());
    super::block_on(entered).unwrap();
    driver.input("enter")?;
    assert_eq!(poll(&mut restarted), Poll::Ready(Ok(())));
    assert!(open.is_closed());
    Ok(())
}

/// Every editor operation and the custom editor's actions reach the editor the example extends.
#[test]
fn rainbow_editor_keeps_custom_editor_operations() -> Result<(), String> {
    let driver = Driver::new()?;
    assert_eq!(
        lines(&driver.host.observed(), "editor built"),
        ["editor built custom=[]"]
    );
    let mut answers = Vec::new();
    for request in [
        json!({ "op": "setOnChange", "callback": "change" }),
        json!({ "op": "setOnSubmit", "callback": "submit" }),
        json!({ "op": "setText", "text": "a" }),
        json!({ "op": "getExpandedText" }),
        json!({ "op": "borderColor" }),
        json!({ "op": "setBorderColor", "role": "borderAccent" }),
        json!({ "op": "setPaddingX", "padding": 2.0 }),
        json!({ "op": "addToHistory", "text": "past" }),
        json!({ "op": "insertTextAtCursor", "text": "!" }),
        json!({ "op": "setAutocompleteMaxVisible", "max": 4.0 }),
        json!({ "op": "setAutocompleteProvider" }),
        json!({ "op": "onChange" }),
        json!({ "op": "getText" }),
    ] {
        answers.push(driver.host.authored_editor(request)?);
    }
    assert_eq!(
        answers,
        [
            json!("done"),
            json!("done"),
            json!("done"),
            json!({ "text": "a" }),
            json!({ "style": "<borderMuted>border" }),
            json!("done"),
            json!("done"),
            json!("done"),
            json!("done"),
            json!("done"),
            json!("done"),
            json!({ "handler": true }),
            json!({ "text": "a!" }),
        ]
    );
    driver.input("enter")?;
    assert_eq!(driver.render()?, ["<borderAccent>edit|40", ""]);
    let observed = driver.host.observed();
    assert_eq!(
        lines(&observed, "text "),
        [
            r#"text change "a""#,
            r#"text change "a!""#,
            r#"text change "probe""#,
            r#"text submit "a!""#,
        ]
    );
    let custom = observed.ui.editor.as_ref().and_then(|slot| slot.custom);
    let provided = lines(
        &observed,
        &format!("widget {} setAutocompleteProvider ", custom.unwrap()),
    );
    drop(observed);
    release_lent(&driver, custom.unwrap(), &provided)
}

/// Releases the callbacks and provider the custom editor kept, as a host does when it drops
/// that editor.
fn release_lent(driver: &Driver, custom: u32, provided: &[String]) -> Result<(), String> {
    let handlers = {
        let mut observed = driver.host.observe();
        let editor = super::widget_editors::editor(&mut observed, custom)?;
        editor
            .handlers
            .drain()
            .map(|(_, key)| key)
            .collect::<Vec<_>>()
    };
    for key in handlers {
        release(key);
    }
    for line in provided {
        let (_, keys) = line.rsplit_once(' ').ok_or("provider line")?;
        let keys: Vec<u32> = serde_json::from_str(keys).map_err(|e| e.to_string())?;
        keys.into_iter().for_each(release);
    }
    Ok(())
}

/// Replacing the editor and releasing the old one cancels its pending frame.
#[test]
fn rainbow_editor_release_cancels_the_animation() -> Result<(), String> {
    let driver = Driver::new()?;
    let (entered, open) = hold(&mut driver.host.observe());
    driver.input(&typed(&expected()))?;
    let mut tail = driver.tail();
    assert!(poll(&mut tail).is_pending());
    super::block_on(entered).unwrap();
    let replaced = driver.shown();
    driver.session_start()?;
    assert!(
        !open.is_closed(),
        "the host still holds the replaced editor"
    );
    driver.release_replaced(replaced);
    assert_eq!(poll(&mut tail), Poll::Ready(Ok(())));
    assert!(
        open.is_closed(),
        "releasing the editor drops its pending wait"
    );
    assert_eq!(driver.render()?, ["<borderMuted>edit|40", ""]);
    assert_eq!(driver.tails(), 1);
    Ok(())
}

/// One guest component request of the built example under a fresh emitter.
async fn component(
    accessor: &Accessor<super::host::State>,
    exports: &super::host_family::guest::Guest,
    rep: u32,
    call: Call<'_>,
) -> wasmtime::Result<Reply> {
    let emitter = accessor.with(|mut a| a.get().table.push(super::bus_host::Emitter { key: 0 }))?;
    let borrowed = || Resource::new_borrow(emitter.rep());
    let reply = match call {
        Call::Render(width) => exports
            .call_component_render(accessor, super::host::borrow(rep), width, borrowed())
            .await?
            .map(Reply::Lines),
        Call::Input(data) => exports
            .call_component_handle_input(
                accessor,
                super::host::borrow(rep),
                data.to_owned(),
                borrowed(),
            )
            .await?
            .map(|()| Reply::Done),
        Call::Release | Call::Invalidate => unreachable!("not used"),
    };
    accessor.with(|mut a| a.get().table.delete(emitter))?;
    reply.map_err(wasmtime::Error::msg)
}

/// Delivers `session_start` to the built component and checks its no-result answer.
fn component_session_start(harness: &mut super::host::Harness) -> wasmtime::Result<()> {
    let key = harness.callback("event session_start")?;
    let context = harness.ordinary("/work")?;
    let exports = harness.exports.clone();
    let outcome = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                super::documents::document("session_start").to_string(),
                super::host_family::guest::Capabilities {
                    ctx: context,
                    signal: None,
                },
            )
            .await
    }))??
    .map_err(wasmtime::Error::msg)?;
    let answer = super::documents::Answer::of(super::host_family::delivery(outcome))
        .map_err(wasmtime::Error::msg)?;
    assert_eq!(answer.ended, super::documents::Ended::Returned(None));
    Ok(())
}

/// Types the keyword, advances one frame, then releases the editor while its next wait pends.
fn component_frames(
    harness: &mut super::host::Harness,
    expected: &Expected,
) -> wasmtime::Result<()> {
    let (shown, operations) = {
        let slot = harness.observed().ui.editor.take().expect("shown editor");
        (slot.component, slot.editor)
    };
    let exports = harness.exports.clone();
    let (entered, open) = hold(harness.observed());
    let (next, pending) = hold(harness.observed());
    super::block_on(harness.store.run_concurrent(async |accessor| {
        component(accessor, &exports, shown, Call::Input(&typed(expected))).await?;
        let key = accessor.with(|mut a| *a.get().observed.bus.tails.last().unwrap());
        let tail = exports.call_invoke_tail(accessor, super::host::borrow(key));
        let control = async {
            entered.await.unwrap();
            let rows = component(accessor, &exports, shown, Call::Render(40)).await;
            assert_eq!(rows.unwrap(), Reply::Lines(expected.frames[0].clone()));
            open.send(()).unwrap();
            next.await.unwrap();
            let rows = component(accessor, &exports, shown, Call::Render(40)).await;
            assert_eq!(rows.unwrap(), Reply::Lines(expected.frames[1].clone()));
            let release = accessor
                .with(|access| {
                    exports
                        .func_release()
                        .func()
                        .typed::<(Resource<super::host::Identity>,), ()>(access)
                })
                .unwrap();
            for rep in [shown, operations] {
                let borrowed = (super::host::borrow(rep),);
                release.call_concurrent(accessor, borrowed).await.unwrap();
            }
        };
        super::join::alongside(tail, control)
            .await?
            .map_err(wasmtime::Error::msg)
    }))??;
    assert!(pending.is_closed(), "releasing the editor dropped its wait");
    Ok(())
}

/// The built component installs the editor, redraws after each frame and stops when released.
#[test]
fn rainbow_editor_component_animates_until_released() -> Result<(), String> {
    let mut harness = super::host::Harness::example("rainbow_editor")?;
    let listener = harness.observed().bus.listeners[0].1;
    component_session_start(&mut harness).map_err(|error| error.to_string())?;
    component_frames(&mut harness, &expected()).map_err(|error| error.to_string())?;
    assert_eq!(
        lines(harness.observed(), "writer requestRender"),
        ["writer requestRender false"]
    );
    let mut keys = vec![
        listener,
        harness
            .callback("event session_start")
            .map_err(|error| error.to_string())?,
    ];
    keys.extend(harness.observed().bus.tails.iter().copied());
    for key in keys {
        super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    }
    Ok(())
}
