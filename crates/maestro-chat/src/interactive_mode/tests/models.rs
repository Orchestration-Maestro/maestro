//! Model selection, model scopes and the subscription notice through the mode's installed
//! callbacks, its registered keys, the real panels and an in-memory catalog.
use super::{World, isolated, started};
use crate::ModelSelectorComponent;
use crate::components::session_selector::list::tests::session_picker::decode_cases;
use maestro_app::{
    Model, ModelCost, ModelInput, ProviderConfigInput, ProviderModelInput, SharedHandle,
};
use maestro_tui::{Component, Input, Text, tui::InputHandler};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    any::Any,
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Every recorded model-selection case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_models.json");

/// The width transcript rows are rendered at.
const WIDTH: usize = 200;

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    decode_cases(FIXTURES, test)
}

/// Registers `provider` with `ids` as its models, authenticated by a configured key, and
/// returns the catalog's descriptors in order.
fn register(world: &World, provider: &str, ids: &[&str]) -> Vec<Arc<RwLock<Model>>> {
    let session = world.host.session();
    let models = ids
        .iter()
        .map(|id| ProviderModelInput {
            id: (*id).to_owned(),
            name: (*id).to_owned(),
            api: None,
            base_url: None,
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 1000.0,
            max_tokens: 100.0,
            headers: None,
            compat: None,
        })
        .collect();
    session
        .model_registry()
        .register_provider(
            provider,
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                base_url: Some("https://example.invalid".into()),
                api_key: Some("fixture".into()),
                api: Some("openai-completions".into()),
                models: Some(models),
                ..ProviderConfigInput::default()
            })),
        )
        .unwrap();
    ids.iter()
        .map(|id| session.model_registry().find(provider, id).unwrap())
        .collect()
}

/// The current session model's `provider/id`.
fn current(world: &World) -> String {
    let model = world.host.session().model();
    let model = model.read().unwrap();
    format!("{}/{}", model.provider, model.id)
}

/// Whether the transcript shows `text`.
fn shows(world: &World, text: &str) -> bool {
    world
        .mode
        .chat_container
        .render(WIDTH)
        .concat()
        .contains(text)
}

/// The components the editor container holds.
fn shown(world: &World) -> Vec<Rc<dyn Component>> {
    world.mode.editor_container.children().borrow().clone()
}

/// Whether the editor container holds only `component`.
fn shows_only(world: &World, component: &Rc<dyn Component>) -> bool {
    let shown = shown(world);
    shown.len() == 1 && std::ptr::addr_eq(Rc::as_ptr(&shown[0]), Rc::as_ptr(component))
}

/// The editor as a shown component.
fn editor(world: &World) -> Rc<dyn Component> {
    world.mode.editor.clone()
}

/// Whether typed text reaches the editor, which then holds exactly that text.
fn editor_focused(world: &World) -> bool {
    world.mode.editor.set_text("");
    world.terminal.send_input("x");
    let focused = world.mode.editor.get_text() == "x";
    world.mode.editor.set_text("");
    focused
}

/// The model panel the editor container shows, if it shows one.
fn model_panel(world: &World) -> Option<Rc<ModelSelectorComponent>> {
    let shown = shown(world);
    let [panel] = shown.as_slice() else {
        return None;
    };
    (Rc::clone(panel) as Rc<dyn Any>)
        .downcast::<ModelSelectorComponent>()
        .ok()
}

/// One recorded effect, its name first.
type Event = Vec<String>;

/// The final state recorded effects leave behind: the editor container's children, the
/// focused component, whether a frame was requested and whether the factory ran.
#[derive(Debug, Default, PartialEq)]
struct Effects {
    /// The editor container's children, by recorded name.
    children: Vec<String>,
    /// The focused component's recorded name.
    focus: Option<String>,
    /// Whether a frame was requested.
    render: bool,
    /// Whether the panel factory ran.
    factory: bool,
}

/// Folds recorded effects, starting from the editor shown and focused. Frames run on the
/// terminal runtime after the callback returns, so only the state each effect leaves is
/// observable, not the order of effects on separate owners.
fn effects(events: &[Event]) -> Effects {
    let mut state = Effects {
        children: vec!["editor".into()],
        focus: Some("editor".into()),
        ..Effects::default()
    };
    for event in events {
        match event.iter().map(String::as_str).collect::<Vec<_>>()[..] {
            ["factory"] => state.factory = true,
            ["clear"] => state.children.clear(),
            ["add", name] => state.children.push(name.into()),
            ["focus", name] => state.focus = Some(name.into()),
            ["render"] => state.render = true,
            _ => {}
        }
    }
    state
}

/// The helper closed `closes` times after opening.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperClose {
    /// Always `selector-helper-close`.
    op: String,
    /// How often the completion runs.
    closes: usize,
}

/// The effects of opening and of every completion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperEffects {
    /// Opening's effects.
    opened: Vec<Event>,
    /// The completions' effects.
    closed: Vec<Event>,
}

/// The state of the mode's editor container, focus and frames as recorded names.
fn observed(world: &World, panel: &Rc<dyn Component>, focus: &Rc<Input>) -> Effects {
    let render = world.runtime.pending() > 0;
    let names = [(editor(world), "editor"), (Rc::clone(panel), "panel")];
    let children = shown(world)
        .iter()
        .map(|child| {
            let name = names
                .iter()
                .find(|(known, _)| std::ptr::addr_eq(Rc::as_ptr(known), Rc::as_ptr(child)));
            name.map_or("other", |(_, name)| name).to_owned()
        })
        .collect();
    let target = if editor_focused(world) {
        "editor"
    } else {
        world.terminal.send_input("y");
        assert!(focus.get_value().ends_with('y'));
        "focus-child"
    };
    Effects {
        children,
        focus: Some(target.to_owned()),
        render,
        factory: false,
    }
}

#[test]
fn selector_completion_restores_editor_and_focus() {
    isolated(
        "selector_completion_restores_editor_and_focus",
        |directory| {
            for (input, expected) in
                cases::<HelperClose, HelperEffects>("selector_completion_restores_editor_and_focus")
            {
                assert_eq!(input.op, "selector-helper-close");
                let world = started(directory);
                let panel: Rc<dyn Component> = Rc::new(Text::new("panel".into(), 0, 0, None));
                let focus = Rc::new(Input::new());
                let done = RefCell::new(None);
                let mut factory = false;
                world
                    .mode
                    .show_selector(|close| {
                        factory = shows_only(&world, &editor(&world));
                        *done.borrow_mut() = Some(close);
                        Ok((Rc::clone(&panel), focus.clone() as Rc<dyn Component>))
                    })
                    .unwrap();
                let opened = effects(&expected.opened);
                assert_eq!(factory, opened.factory);
                assert_eq!(
                    Effects {
                        factory,
                        ..observed(&world, &panel, &focus)
                    },
                    opened
                );
                world.runtime.settle().unwrap();
                let done = done.borrow_mut().take().unwrap();
                for _ in 0..input.closes {
                    done();
                }
                let closed = Effects {
                    render: effects(&expected.closed).render,
                    ..effects(&[expected.opened, expected.closed].concat())
                };
                assert_eq!(
                    Effects {
                        factory: true,
                        ..observed(&world, &panel, &focus)
                    },
                    closed
                );
            }
        },
    );
}

/// The helper whose factory fails.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperFailure {
    /// Always `selector-helper-failure`.
    op: String,
}

/// The factory's error and the effects that followed it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureEffects {
    /// The factory's error.
    error: String,
    /// The effects after the failure.
    events: Vec<Event>,
}

#[test]
fn selector_factory_failure_keeps_the_editor() {
    isolated("selector_factory_failure_keeps_the_editor", |directory| {
        let (input, expected) =
            cases::<HelperFailure, FailureEffects>("selector_factory_failure_keeps_the_editor")
                .swap_remove(0);
        assert_eq!(input.op, "selector-helper-failure");
        let world = started(directory);
        let message = expected.error.clone();
        let error = world
            .mode
            .show_selector(move |_| Err(message.into()))
            .unwrap_err();
        assert_eq!(error.to_string(), expected.error);
        let panel: Rc<dyn Component> = Rc::new(Text::new(String::new(), 0, 0, None));
        assert_eq!(
            observed(&world, &panel, &Rc::new(Input::new())),
            effects(&expected.events)
        );
    });
}

/// Checks the recorded final notice, a status or an error, against the transcript and the
/// border, and returns whether the selection succeeded.
fn final_notice(world: &World, notice: &Event) -> bool {
    let border = world.mode.editor.render(80).swap_remove(0);
    let [kind, text] = notice.as_slice() else {
        panic!("a final notice");
    };
    if kind == "status" {
        assert!(shows(world, text));
        assert_ne!(border, "─".repeat(80));
        return true;
    }
    assert_eq!(kind, "error");
    assert!(shows(world, "Error: No API key for a/alpha"));
    assert_eq!(border, "─".repeat(80));
    false
}

/// A model panel selection that the session accepts or refuses.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelSelect {
    /// Always `selector-select`.
    op: String,
    /// Whether the session refuses the model.
    failure: bool,
}

/// The saved default and the selection's effects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelSelected {
    /// The saved default provider and model.
    defaults: [String; 2],
    /// The selection's effects, the final notice last.
    events: Vec<Event>,
    /// The session scope afterwards.
    scope: Vec<String>,
    /// The available provider count, unchanged.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

/// The model-select key.
const SELECT_KEY: &str = "\x0c";

/// Opens the model panel through its key and waits for it to show.
fn open_panel(world: &World, key: &str) -> Rc<ModelSelectorComponent> {
    world.terminal.send_input(key);
    model_panel(world).expect("the model panel is shown")
}

#[test]
fn model_selector_selection_restores_editor_before_its_notice() {
    isolated(
        "model_selector_selection_restores_editor_before_its_notice",
        |directory| {
            for (input, expected) in cases::<PanelSelect, PanelSelected>(
                "model_selector_selection_restores_editor_before_its_notice",
            ) {
                assert_eq!(input.op, "selector-select");
                let world = started(directory);
                register(&world, "a", &["alpha"]);
                let previous = current(&world);
                world.mode.editor.set_border_color(Rc::new(str::to_owned));
                open_panel(&world, SELECT_KEY);
                if input.failure {
                    let session = world.host.session();
                    session.model_registry().unregister_provider("a").unwrap();
                }
                world.terminal.send_input("\r");
                assert!(world.runtime.drain_futures().is_empty());
                let defaults = {
                    let session = world.host.session();
                    let settings = session.settings_manager().borrow();
                    [
                        settings.get_default_provider(),
                        settings.get_default_model(),
                    ]
                };
                assert_eq!(defaults, expected.defaults.clone().map(Some));
                let final_state = effects(&expected.events);
                assert!(shows_only(&world, &editor(&world)));
                assert_eq!(final_state.children, ["editor"]);
                assert!(editor_focused(&world));
                let session = world.host.session();
                let selected = if final_notice(&world, expected.events.last().unwrap()) {
                    expected.defaults.join("/")
                } else {
                    previous
                };
                assert_eq!(current(&world), selected);
                assert!(expected.scope.is_empty() && session.scoped_models().is_empty());
                assert_eq!(expected.count, None);
                assert!(!expected.shown);
            }
        },
    );
}

/// A cancelled model panel.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelCancel {
    /// Always `selector-cancel`.
    op: String,
}

/// The cancellation's effects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelCancelled {
    /// The effects of cancelling.
    events: Vec<Event>,
    /// The session scope afterwards.
    scope: Vec<String>,
    /// The available provider count, unchanged.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

#[test]
fn model_selector_cancel_restores_editor_and_requests_a_frame() {
    isolated(
        "model_selector_cancel_restores_editor_and_requests_a_frame",
        |directory| {
            let (input, expected) = cases::<PanelCancel, PanelCancelled>(
                "model_selector_cancel_restores_editor_and_requests_a_frame",
            )
            .swap_remove(0);
            assert_eq!(input.op, "selector-cancel");
            let world = started(directory);
            register(&world, "a", &["alpha"]);
            let previous = world.host.session().model();
            open_panel(&world, SELECT_KEY);
            world.runtime.settle().unwrap();
            world.terminal.send_input("\x1b");
            let cancelled = effects(&expected.events);
            assert!(shows_only(&world, &editor(&world)));
            assert_eq!(cancelled.children, ["editor"]);
            assert_eq!(world.runtime.pending() > 0, cancelled.render);
            assert!(editor_focused(&world));
            let session = world.host.session();
            assert!(Arc::ptr_eq(&session.model(), &previous));
            assert_eq!(
                session.settings_manager().borrow().get_default_model(),
                None
            );
            assert!(expected.scope.is_empty() && session.scoped_models().is_empty());
            assert_eq!(expected.count, None);
            assert!(!expected.shown);
        },
    );
}

/// The model-select key pressed in the editor.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectAction {
    /// Always `model-select-action`.
    op: String,
}

/// The bound action and what it opened.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectActionEffects {
    /// The action the key is bound to.
    action: String,
    /// The effects: the panel opened.
    events: Vec<Event>,
    /// The session scope afterwards.
    scope: Vec<String>,
    /// The available provider count, unchanged.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

#[test]
fn model_select_action_uses_the_registered_binding() {
    isolated(
        "model_select_action_uses_the_registered_binding",
        |directory| {
            let (input, expected) = cases::<SelectAction, SelectActionEffects>(
                "model_select_action_uses_the_registered_binding",
            )
            .swap_remove(0);
            assert_eq!(input.op, "model-select-action");
            assert_eq!(expected.events, [["opened"]]);
            assert!(expected.scope.is_empty() && expected.count.is_none() && !expected.shown);
            let world = started(directory);
            open_panel(&world, SELECT_KEY);
            std::fs::write(
                directory.join("keybindings.json"),
                format!(r#"{{"{}": "ctrl+y"}}"#, expected.action),
            )
            .unwrap();
            let world = started(directory);
            world.terminal.send_input(SELECT_KEY);
            assert!(model_panel(&world).is_none());
            open_panel(&world, "\x19");
        },
    );
}

#[test]
fn model_callbacks_read_current_session_and_release_the_mode() {
    isolated(
        "model_callbacks_read_current_session_and_release_the_mode",
        |directory| {
            let world = started(directory);
            register(&world, "a", &["alpha"]);
            let panel = open_panel(&world, SELECT_KEY);
            let World {
                mode,
                runtime,
                host,
                ..
            } = world;
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(
                weak.upgrade().is_none(),
                "retained callbacks release the mode"
            );
            panel.handle_input("\r");
            assert_eq!(runtime.futures(), 0);
            assert_ne!(host.session().model().read().unwrap().id, "alpha");
            let world = started(directory);
            register(&world, "a", &["alpha"]);
            open_panel(&world, SELECT_KEY).handle_input("\r");
            let World {
                mode,
                runtime,
                host,
                ..
            } = world;
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(
                weak.upgrade().is_some(),
                "an admitted selection owns the mode"
            );
            assert!(runtime.drain_futures().is_empty());
            assert_eq!(host.session().model().read().unwrap().id, "alpha");
            assert!(weak.upgrade().is_none());
        },
    );
}
