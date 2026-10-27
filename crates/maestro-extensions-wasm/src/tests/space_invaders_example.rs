//! The shipped game through retained components, registered commands and owned bus tails.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{controlled::Controlled, host::Harness, observed::Observed};
use crate::component_adapter::Exports;
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[path = "../../../../examples/extensions/space_invaders.rs"]
mod space_invaders;
use space_invaders::{SpaceInvadersComponent, game::GameState};

/// The factory exported by the shipped example.
struct Invaders;
impl Extension for Invaders {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        space_invaders::factory(api)
    }
}
/// Frames and saved records captured by running the original game with controlled ticks.
fn corpus() -> Value {
    serde_json::from_str(include_str!("space_invaders_frames.json")).unwrap()
}
/// Supplies literal key-decoder answers for plain and enhanced keyboard inputs.
fn helpers(observed: &mut Observed) {
    observed.actions.value =
        json!({"keys.is_key_release":false,"keys.matches_key":false,"visible_width":63});
    for (data, key, release) in [
        ("\x1b", "escape", false),
        ("\x1b[D", "left", false),
        ("\x1b[C", "right", false),
        ("\x1b[97;1:3u", "a", true),
        ("\x1b[97;1:2u", "a", false),
        ("\x1b[100;1:3u", "d", true),
        ("\x1b[102;1:3u", "f", true),
        (" ", "space", false),
    ] {
        observed.actions.helpers.insert(
            ("keys.is_key_release".into(), json!([data]).to_string()),
            json!(release),
        );
        observed.actions.helpers.insert(
            ("keys.matches_key".into(), json!([data, key]).to_string()),
            json!(true),
        );
    }
}
/// Supplies measured widths for the literal oracle frame, including its boxed content.
fn frame_helpers(observed: &mut Observed, frame: &Value) {
    let ansi = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
    for row in frame.as_array().unwrap() {
        let row = row.as_str().unwrap().trim_end();
        let mut texts = vec![row];
        if let Some(content) = row
            .strip_prefix("\x1b[2m │\x1b[22m")
            .and_then(|s| s.strip_suffix("\x1b[2m│\x1b[22m"))
        {
            texts.push(content.trim_end());
        }
        for text in texts {
            observed.actions.helpers.insert(
                ("visible_width".into(), json!([text]).to_string()),
                json!(ansi.replace_all(text, "").chars().count()),
            );
        }
    }
}
/// Expected frame at the supplied width, preserving all authored ANSI sequences.
fn frame(value: &Value, width: usize) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let row = row.as_str().unwrap().trim_end();
            format!("{row}{}", " ".repeat(width.saturating_sub(63)))
        })
        .collect()
}
/// A retained view with save and close effects observed outside the component.
struct View {
    /// Existing controlled adapter.
    host: Controlled,
    /// The same retained component its factory returned.
    component: Rc<SpaceInvadersComponent>,
    /// Ordered saves, including explicit null discards.
    saves: Rc<RefCell<Vec<Value>>>,
    /// Accepted close calls.
    closed: Rc<std::cell::Cell<usize>>,
}
impl View {
    /// Supplies the existing adapter's terminal and observes saves without inspecting state.
    fn new(state: Option<GameState>) -> Result<Self, String> {
        let host = Controlled::default();
        let api = super::bus_callbacks::captured_api(&host)?;
        let saves = Rc::new(RefCell::new(Vec::new()));
        let saving = saves.clone();
        let closed = Rc::new(std::cell::Cell::new(0));
        let closing = closed.clone();
        let tui = crate::component_adapter::factories::test_tui(
            &host,
            super::controlled::Tui::lent(&host.observed),
        );
        let component = Rc::new(SpaceInvadersComponent::new(
            api,
            tui,
            Rc::new(move |()| {
                closing.set(closing.get() + 1);
                Ok(())
            }),
            Rc::new(move |state| {
                saving
                    .borrow_mut()
                    .push(serde_json::to_value(state).unwrap());
                Ok(())
            }),
            state,
        ));
        helpers(&mut host.observe());
        Ok(Self {
            host,
            component,
            saves,
            closed,
        })
    }
    /// Delivers input through the component interface and borrowed-emitter adapter.
    fn input(&self, data: &str) -> Result<(), String> {
        crate::component_adapter::renderers::with_emitter(
            &self.host,
            &super::bus_host::Emitter { key: 0 },
            |emitter| self.component.handle_input(data, emitter),
        )
    }
    /// Renders at a supplied width through the same interface the host invokes.
    fn render(&self, width: u32) -> Result<Vec<String>, String> {
        crate::component_adapter::renderers::with_emitter(
            &self.host,
            &super::bus_host::Emitter { key: 0 },
            |emitter| self.component.render(width, emitter),
        )
    }
}
use crate::ComponentView;

#[test]
fn space_invaders_ticks_match_reference_movement_fire_and_collisions() -> Result<(), String> {
    let corpus = corpus();
    for case in corpus["cases"].as_array().unwrap() {
        let view = View::new(Some(serde_json::from_value(case["start"].clone()).unwrap()))?;
        view.input("x")?;
        for key in case["inputs"].as_array().unwrap() {
            view.input(key.as_str().unwrap())?;
        }
        for _ in 0..case["ticks"].as_u64().unwrap() {
            let column = case["column"].as_u64().unwrap_or(0);
            view.component.tick(|_| usize::try_from(column).unwrap())?;
        }
        frame_helpers(&mut view.host.observe(), &case["frame"]);
        assert_eq!(
            view.render(80)?,
            frame(&case["frame"], 80),
            "{}",
            case["name"]
        );
        view.input("\x1b")?;
        assert_eq!(
            *view.saves.borrow(),
            [case["expected"].clone()],
            "{}",
            case["name"]
        );
        assert_eq!(view.closed.get(), 1);
        view.finish();
    }
    Ok(())
}

#[test]
fn space_invaders_releases_keys_restarts_and_discards_saves() -> Result<(), String> {
    let view = View::new(None)?;
    view.input("a")?;
    view.input("\x1b[97;1:2u")?;
    for _ in 0..2 {
        view.component.tick(|_| 0)?;
    }
    view.input("\x1b[97;1:3u")?;
    view.input("f")?;
    view.input("\x1b[102;1:3u")?;
    for _ in 0..2 {
        view.component.tick(|_| 0)?;
    }
    view.input("\x1b")?;
    assert_eq!(view.saves.borrow()[0]["player"]["x"], 29);
    assert_eq!(view.saves.borrow()[0]["bullets"], json!([]));
    view.component.tick(|_| 0)?;
    view.input("q")?;
    assert_eq!(view.closed.get(), 1, "a removed view accepts no more input");
    view.finish();
    for case in corpus()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| matches!(c["name"].as_str(), Some("victory" | "loss")))
    {
        let view = View::new(Some(serde_json::from_value(case["start"].clone()).unwrap()))?;
        view.input("x")?;
        view.component.tick(|_| 0)?;
        view.input(" ")?;
        assert_eq!(*view.saves.borrow(), [Value::Null]);
        for _ in 0..2 {
            view.component.tick(|_| 0)?;
        }
        view.input("\x1b")?;
        let saves = view.saves.borrow();
        let state = &saves[1];
        assert_eq!(
            state["level"],
            if case["name"] == "victory" {
                json!(3)
            } else {
                json!(1)
            }
        );
        assert_eq!(state["highScore"], case["expected"]["highScore"]);
        assert_eq!(state["player"], json!({"x":30,"lives":3}));
        assert_eq!(state["bullets"], json!([]), "restart clears held fire");
        drop(saves);
        view.finish();
    }
    Ok(())
}

impl View {
    /// Drops the retained view before the shared registration teardown checks its leases.
    fn finish(self) {
        drop(self.component);
        super::example_support::release_all(&self.host, &bus_registrations(&self.host));
    }
}
/// Expected listener registrations and independent tails, in the shared teardown order.
fn bus_registrations(host: &Controlled) -> Vec<String> {
    let observed = host.observed();
    let mut expected: Vec<_> = observed
        .bus
        .removals
        .iter()
        .map(|id| format!("event bus {id}"))
        .collect();
    expected.extend(std::iter::repeat_n(
        "bus tail".to_owned(),
        observed.bus.tails.len(),
    ));
    expected
}

#[test]
fn space_invaders_tick_tail_cancels_pending_host_sleep() -> Result<(), String> {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let view = View::new(None)?;
    let api = super::bus_callbacks::captured_api(&view.host)?;
    let dispose = crate::component_adapter::renderers::with_emitter(
        &view.host,
        &super::bus_host::Emitter { key: 0 },
        |emitter| space_invaders::ticking(&api, view.component.clone(), emitter),
    )?;
    super::block_on(api.events().emit("space-invaders:run", Value::Null))?;
    assert_eq!(view.host.observed().bus.tails.len(), 1);
    let tail = view.host.observed().bus.tails[0];
    let listener = view.host.observed().bus.listeners[0].1;
    view.input("a")?;
    let mut ticking = Box::pin(Exports::<Controlled>::invoke_tail(tail));
    let mut cx = Context::from_waker(Waker::noop());
    let mut pending: Option<tokio::sync::oneshot::Sender<()>> = None;
    for _ in 0..3 {
        let (hold, mut entered, open) = super::observed::gate();
        view.host.observe().hold_next_idle(hold);
        if let Some(open) = pending.take() {
            open.send(()).unwrap();
        }
        assert!(matches!(ticking.as_mut().poll(&mut cx), Poll::Pending));
        assert_eq!(entered.try_recv(), Ok(()));
        pending = Some(open);
    }
    view.input("\x1b")?;
    assert_eq!(view.saves.borrow()[0]["player"]["x"], 29);
    assert!(
        view.host
            .observed()
            .transcript
            .iter()
            .any(|s| s == "native-wait 0 50000000")
    );
    crate::component_adapter::renderers::with_emitter(
        &view.host,
        &super::bus_host::Emitter { key: 0 },
        |emitter| dispose(emitter),
    )?;
    assert!(
        pending.unwrap().send(()).is_err(),
        "disposal drops the pending native sleep"
    );
    assert_eq!(ticking.as_mut().poll(&mut cx), Poll::Ready(Ok(())));
    assert_eq!(view.host.observed().bus.removals, [listener]);
    drop(dispose);
    drop(ticking);
    view.finish();
    Ok(())
}

/// Both adapters exercise the same shipped factory and custom-view script.
enum Host {
    /// In-process guest adapter.
    Controlled(Controlled, Exports<Controlled>),
    /// Built component loaded by the existing test host.
    Component(Box<Harness>),
}
impl Host {
    /// Loads the selected adapter without a provider or application installation.
    fn new(component: bool) -> Result<Self, String> {
        if component {
            let path = super::build::example_component("space_invaders")?;
            Ok(Self::Component(Box::new(
                super::block_on(Harness::start(&path)).map_err(|e| e.to_string())?,
            )))
        } else {
            let (host, exports) = super::block_on(super::example_support::start::<Invaders>())?;
            Ok(Self::Controlled(host, exports))
        }
    }
    /// Reads or supplies observations without borrowing across guest callbacks.
    fn observe<T>(&mut self, f: impl FnOnce(&mut Observed) -> T) -> T {
        match self {
            Self::Controlled(host, _) => f(&mut host.observe()),
            Self::Component(host) => f(host.observed()),
        }
    }
    /// Invokes the actual registered command.
    fn command(&mut self) -> Result<(), String> {
        let key = self.observe(|o| o.callback("command invaders"))?;
        match self {
            Self::Controlled(host, exports) => super::block_on(exports.invoke_command(
                key,
                "ignored".into(),
                host.command_context("/game"),
            )),
            Self::Component(host) => {
                super::example_support::invoke_command_on_component(host, key, "ignored", false)
            }
        }
    }
    /// Supplies all-session entries and an exact custom-view input/frame script.
    fn script(&mut self, entries: &Value, expected: &Value, key: &str) {
        self.observe(|o| {
            helpers(o);
            frame_helpers(o, expected);
            o.catalog.data.has_ui = true;
            o.readers.control(
                &json!({"bind":{"replies":[{"method":"getEntries","args":[],"value":entries}]}})
                    .to_string(),
            );
            o.ui.custom_script = Some(super::custom_host::Script {
                width: None,
                inputs: vec![key.into()],
                frames: vec![frame(expected, 20); 2],
            });
        });
    }
    /// Saved entries in request order.
    fn saves(&mut self) -> Vec<Value> {
        self.observe(|o| {
            o.transcript
                .iter()
                .filter_map(|s| s.strip_prefix("entry space-invaders-save "))
                .map(|s| serde_json::from_str(s).unwrap())
                .collect()
        })
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let Self::Controlled(host, _) = self else {
            return;
        };
        let mut expected = vec!["command invaders".to_owned()];
        expected.extend(bus_registrations(host));
        super::example_support::release_all(host, &expected);
    }
}
/// A custom entry in the entire session history.
fn save(id: &str, value: &Value) -> Value {
    json!({"type":"custom","id":id,"parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"space-invaders-save","data":value})
}

#[test]
fn space_invaders_component_runs_registered_command_and_restores_latest_save() -> Result<(), String>
{
    let corpus = corpus();
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.command()?;
        host.observe(|o| {
            assert_eq!(
                super::example_support::observations(&o.transcript),
                [super::example_support::Observation::Notify {
                    message: "Space Invaders requires interactive mode".into(),
                    kind: Some("error".into())
                }]
            );
        });
        let mut paused = corpus["cases"][0]["start"].clone();
        paused["highScore"] = json!(70);
        paused["level"] = json!(2);
        paused["alienMoveDelay"] = json!(16);
        host.script(
            &json!([save("old", &Value::Null), save("new", &paused)]),
            &corpus["frames"]["paused"],
            "q",
        );
        host.command()?;
        assert!(
            host.saves().is_empty(),
            "closing the paused view keeps its save"
        );
        host.script(
            &json!([save("old", &paused), save("discard", &Value::Null)]),
            &corpus["frames"]["initial"],
            "\x1b",
        );
        host.command()?;
        assert_eq!(host.saves(), [corpus["cases"][0]["start"].clone()]);
        host.script(&json!([]), &corpus["frames"]["initial"], "q");
        host.command()?;
        assert_eq!(host.saves().last(), Some(&Value::Null));
        host.observe(|o| {
            assert_eq!(
                o.registrations.commands,
                [(
                    "invaders".into(),
                    Some("Play Space Invaders!".into()),
                    false
                )]
            );
        });
    }
    Ok(())
}

#[test]
fn space_invaders_resume_redraws_current_controls_without_losing_save() -> Result<(), String> {
    let corpus = corpus();
    let mut state = corpus["cases"][0]["start"].clone();
    state["highScore"] = json!(70);
    state["level"] = json!(2);
    state["alienMoveDelay"] = json!(16);
    let view = View::new(Some(serde_json::from_value(state.clone()).unwrap()))?;
    frame_helpers(&mut view.host.observe(), &corpus["frames"]["paused"]);
    frame_helpers(&mut view.host.observe(), &corpus["frames"]["initial"]);
    let paused = view.render(80)?;
    assert_eq!(paused, frame(&corpus["frames"]["paused"], 80));
    view.input("\x1b[97;1:3u")?;
    view.component.tick(|_| 0)?;
    assert_eq!(
        view.render(80)?,
        paused,
        "a release never resumes a paused game"
    );
    view.input("f")?;
    assert_eq!(
        view.render(80)?[28],
        frame(&corpus["frames"]["initial"], 80)[28]
    );
    view.input("\x1b")?;
    assert_eq!(
        *view.saves.borrow(),
        [state],
        "the resume key does not also fire"
    );
    view.finish();
    Ok(())
}

#[test]
fn space_invaders_simultaneous_hits_keep_lives_nonnegative() -> Result<(), String> {
    let mut state = corpus()["cases"][2]["start"].clone();
    let bullet = state["bullets"][0].clone();
    state["bullets"].as_array_mut().unwrap().push(bullet);
    let view = View::new(Some(serde_json::from_value(state).unwrap()))?;
    view.input("x")?;
    view.component.tick(|_| 0)?;
    view.input("\x1b")?;
    assert_eq!(view.saves.borrow()[0]["player"]["lives"], 0);
    assert_eq!(view.saves.borrow()[0]["gameOver"], true);
    assert!(
        view.render(80)?
            .iter()
            .any(|line| line.contains("GAME OVER!"))
    );
    view.finish();
    Ok(())
}
