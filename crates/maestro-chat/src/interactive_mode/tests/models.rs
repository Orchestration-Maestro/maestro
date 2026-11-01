//! Model selection, model scopes and the subscription notice through the mode's installed
//! callbacks, its registered keys, the real panels and an in-memory catalog.
use super::actions::{install, run_action, shows};
use super::transcript::assistant;
use super::typed::{poll, recalled, submit, waiting};
use super::{Setup, World, isolated, started};
use crate::ModelSelectorComponent;
use crate::components::scoped_models_selector::{ModelsCallbacks, ScopedModelsSelectorComponent};
use crate::components::session_selector::list::tests::session_picker::decode_cases;
use maestro_app::{
    AuthCredential, BoxFuture, DiagnosticErrorInfo, ExtensionFactory, Fetch, Local, Model,
    ModelCost, ModelInput, ModelSelectEvent, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface, ProviderConfigInput, ProviderModelInput, ReadonlyFooterDataProvider,
    ScopedModel, SessionManager, Settings, SettingsListEntry, SettingsManager, SharedHandle,
    ThinkingLevel, WarningSettings,
};
use maestro_test_process::TestProcess;
use maestro_tui::{Component, Input, Text, tui::InputHandler};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    any::Any,
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Every recorded model-selection case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_models.json");

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    decode_cases(FIXTURES, test)
}

/// A catalog model definition named `id`.
pub(super) fn model_input(id: &str) -> ProviderModelInput {
    ProviderModelInput {
        id: id.to_owned(),
        name: id.to_owned(),
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
    }
}

/// Registers `provider` with `ids` as its models, authenticated by a configured key, and
/// returns the catalog's descriptors in order.
pub(super) fn register(world: &World, provider: &str, ids: &[&str]) -> Vec<Arc<RwLock<Model>>> {
    register_configured(world, provider, ids, |config| {
        config.api_key = Some("fixture".into());
    })
}

/// Registers `provider` with `ids` as its models after `configure` changes its record, and
/// returns the catalog's descriptors in order.
pub(super) fn register_configured(
    world: &World,
    provider: &str,
    ids: &[&str],
    configure: impl FnOnce(&mut ProviderConfigInput),
) -> Vec<Arc<RwLock<Model>>> {
    let session = world.host.session();
    let models = ids.iter().map(|id| model_input(id)).collect();
    let mut config = ProviderConfigInput {
        base_url: Some("https://example.invalid".into()),
        api: Some("openai-completions".into()),
        models: Some(models),
        ..ProviderConfigInput::default()
    };
    configure(&mut config);
    session
        .model_registry()
        .register_provider(provider, SharedHandle::new(RwLock::new(config)))
        .unwrap();
    ids.iter()
        .map(|id| session.model_registry().find(provider, id).unwrap())
        .collect()
}

/// The current session model's `provider/id`.
pub(super) fn current(world: &World) -> String {
    let model = world.host.session().model();
    let model = model.read().unwrap();
    format!("{}/{}", model.provider, model.id)
}

/// The components the editor container holds.
pub(super) fn shown(world: &World) -> Vec<Rc<dyn Component>> {
    world.mode.editor_container.children().borrow().clone()
}

/// Whether the editor container holds only `component`.
pub(super) fn shows_only(world: &World, component: &Rc<dyn Component>) -> bool {
    let shown = shown(world);
    shown.len() == 1 && std::ptr::addr_eq(Rc::as_ptr(&shown[0]), Rc::as_ptr(component))
}

/// The editor as a shown component.
pub(super) fn editor(world: &World) -> Rc<dyn Component> {
    world.mode.editor.clone()
}

/// Whether typed text reaches the editor, which then holds exactly that text.
pub(super) fn editor_focused(world: &World) -> bool {
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
type Event = Vec<serde_json::Value>;

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
        match event
            .iter()
            .map(|part| part.as_str().unwrap_or_default())
            .collect::<Vec<_>>()[..]
        {
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
        async |directory| {
            for (input, expected) in
                cases::<HelperClose, HelperEffects>("selector_completion_restores_editor_and_focus")
            {
                assert_eq!(input.op, "selector-helper-close");
                let world = started(directory).await;
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
    isolated(
        "selector_factory_failure_keeps_the_editor",
        async |directory| {
            let (input, expected) =
                cases::<HelperFailure, FailureEffects>("selector_factory_failure_keeps_the_editor")
                    .swap_remove(0);
            assert_eq!(input.op, "selector-helper-failure");
            let world = started(directory).await;
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
        },
    );
}

/// Checks the recorded final notice, a status or an error, against the transcript and the
/// border, and returns whether the selection succeeded.
fn final_notice(world: &World, notice: &Event) -> bool {
    let border = world.mode.editor.render(80).swap_remove(0);
    let [kind, text] = notice.as_slice() else {
        panic!("a final notice");
    };
    let text = text.as_str().unwrap();
    if kind == "status" {
        assert!(shows(&world.mode.chat_container, text));
        assert_ne!(border, "─".repeat(80));
        return true;
    }
    assert_eq!(kind, "error");
    assert!(shows(
        &world.mode.chat_container,
        "Error: No API key for a/alpha"
    ));
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
        async |directory| {
            for (input, expected) in cases::<PanelSelect, PanelSelected>(
                "model_selector_selection_restores_editor_before_its_notice",
            ) {
                assert_eq!(input.op, "selector-select");
                let world = started(directory).await;
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
        async |directory| {
            let (input, expected) = cases::<PanelCancel, PanelCancelled>(
                "model_selector_cancel_restores_editor_and_requests_a_frame",
            )
            .swap_remove(0);
            assert_eq!(input.op, "selector-cancel");
            let world = started(directory).await;
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
        async |directory| {
            let (input, expected) = cases::<SelectAction, SelectActionEffects>(
                "model_select_action_uses_the_registered_binding",
            )
            .swap_remove(0);
            assert_eq!(input.op, "model-select-action");
            assert_eq!(expected.events, [["opened"]]);
            assert!(expected.scope.is_empty() && expected.count.is_none() && !expected.shown);
            let world = started(directory).await;
            open_panel(&world, SELECT_KEY);
            std::fs::write(
                directory.join("keybindings.json"),
                format!(r#"{{"{}": "ctrl+y"}}"#, expected.action),
            )
            .unwrap();
            let world = started(directory).await;
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
        async |directory| {
            let world = started(directory).await;
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
            let world = started(directory).await;
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
            scope_callbacks_release_the_mode(directory).await;
            model_panel_writes_its_settings_and_selects_on_the_current_session(directory).await;
        },
    );
}

/// A shown scope panel does not keep the mode alive; its edits and saves then do nothing.
async fn scope_callbacks_release_the_mode(directory: &Path) {
    let world = started(directory).await;
    catalog(&world);
    world.mode.show_models_selector().unwrap();
    let panel = scope_panel(&world).unwrap();
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
    for key in ["\r", "\x13", "\x1b"] {
        panel.handle_input(key);
    }
    assert_eq!(runtime.futures(), 0);
    let session = host.session();
    assert!(session.scoped_models().is_empty());
    assert!(
        session
            .settings_manager()
            .borrow()
            .get_enabled_models()
            .is_none()
    );
}

/// The model panel saves its default through the settings it was opened with, while the
/// selection applies to the session current when it is chosen.
async fn model_panel_writes_its_settings_and_selects_on_the_current_session(directory: &Path) {
    let world = replaceable(directory).await;
    register(&world, "a", &["alpha"]);
    let panel = open_panel(&world, SELECT_KEY);
    let opened = world.host.session();
    replace_session(&world);
    register(&world, "a", &["alpha"]);
    panel.handle_input("\r");
    assert!(world.runtime.drain_futures().is_empty());
    let current = world.host.session();
    assert_eq!(current.model().read().unwrap().id, "alpha");
    assert_ne!(opened.model().read().unwrap().id, "alpha");
    let default = opened.settings_manager().borrow().get_default_model();
    assert_eq!(default.as_deref(), Some("alpha"));
}

/// The common observations of a model witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    /// The recorded effects.
    events: Vec<Event>,
    /// The session scope afterwards, by `provider/id` and level.
    scope: Vec<(String, Option<ThinkingLevel>)>,
    /// The footer's provider count, when it was set.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

/// The session scope by `provider/id` and level.
fn scope_of(world: &World) -> Vec<(String, Option<ThinkingLevel>)> {
    world
        .host
        .session()
        .scoped_models()
        .into_iter()
        .map(|scoped| (id_of(&scoped.model), scoped.thinking_level))
        .collect()
}

/// A descriptor's `provider/id`.
fn id_of(model: &Arc<RwLock<Model>>) -> String {
    let model = model.read().unwrap();
    format!("{}/{}", model.provider, model.id)
}

/// Checks the scope, the provider count when recorded, and the notice flag.
fn settled(world: &World, outcome: &Outcome) {
    assert_eq!(scope_of(world), outcome.scope);
    if let Some(count) = outcome.count {
        let provider = &world.mode.footer_data_provider;
        assert_eq!(
            provider.get_available_provider_count().to_bits(),
            count.to_bits()
        );
    }
    assert_eq!(
        world.mode.anthropic_subscription_warning_shown.get(),
        outcome.shown
    );
}

/// The recorded events named `name`, without their names.
fn named<'a>(
    events: &'a [Event],
    name: &'a str,
) -> impl Iterator<Item = &'a [serde_json::Value]> + 'a {
    events
        .iter()
        .filter(move |event| event[0] == *name)
        .map(|event| &event[1..])
}

/// The width the long subscription notice fits on one row at.
const NOTICE_WIDTH: usize = 400;

/// Checks the recorded subscription notices, and the recorded key lookups when `lookups`
/// counts them; reading stored credentials has no observable effect and is not compared.
fn notices(world: &World, outcome: &Outcome, lookups: Option<usize>) {
    if let Some(lookups) = lookups {
        assert_eq!(lookups, named(&outcome.events, "key").count());
    }
    let warnings: Vec<_> = named(&outcome.events, "warning").collect();
    let rows = world.mode.chat_container.render(NOTICE_WIDTH).concat();
    assert_eq!(rows.matches("Warning: ").count(), warnings.len());
    for warning in warnings {
        assert!(rows.contains(&format!("Warning: {}", warning[0].as_str().unwrap())));
    }
    settled(world, outcome);
}

/// The subscription provider's catalog model.
fn subscription_model(world: &World) -> Arc<RwLock<Model>> {
    let session = world.host.session();
    session
        .model_registry()
        .find("anthropic", "claude-sonnet-4-5")
        .unwrap()
}

/// A copy of the subscription model under `provider` and `id`.
fn model_named(world: &World, provider: &str, id: &str) -> Arc<RwLock<Model>> {
    let mut model = subscription_model(world).read().unwrap().clone();
    model.provider = provider.into();
    model.id = id.into();
    Arc::new(RwLock::new(model))
}

/// The key command's log of lookups for case `case`.
fn lookup_log(directory: &Path, case: usize) -> PathBuf {
    directory.join(format!("lookups{case}"))
}

/// The key lookups case `case` made.
fn lookups(directory: &Path, case: usize) -> usize {
    std::fs::read_to_string(lookup_log(directory, case)).map_or(0, |log| log.lines().count())
}

/// Stores, for `provider`, a key whose every lookup runs a command that logs the lookup
/// and prints `key`, or fails without one.
fn key_command(
    world: &World,
    directory: &Path,
    provider: &str,
    (case, key): (usize, Option<&str>),
) {
    let name = format!("lookup{case}");
    if !directory.join("bin").join(&name).exists() {
        let output = key.map_or("exit 1".to_owned(), |key| format!("printf '%s' '{key}'"));
        let log = lookup_log(directory, case);
        install(
            directory,
            &name,
            &format!("#!/bin/sh\necho >> '{}'\n{output}\n", log.display()),
        );
    }
    let credential = serde_json::json!({"type": "api_key", "key": format!("!{name}")});
    let session = world.host.session();
    session
        .model_registry()
        .auth_storage()
        .set(provider, serde_json::from_value(credential).unwrap());
}

/// Stores a sign-in for the subscription provider.
pub(super) fn sign_in(world: &World) {
    let session = world.host.session();
    session.model_registry().auth_storage().set(
        "anthropic",
        AuthCredential::OAuth(OAuthCredentials {
            access: "access".into(),
            refresh: "refresh".into(),
            expires: 0.0,
            extra: serde_json::Map::default(),
        }),
    );
}

/// Runs subscription checks for `models` together on the terminal runtime.
fn check(world: &World, models: Vec<Option<Arc<RwLock<Model>>>>) {
    for model in models {
        let mode = Rc::clone(&world.mode);
        world.runtime.handle().spawn_local(Box::pin(async move {
            mode.maybe_warn_about_anthropic_subscription_auth(model)
                .await
        }));
    }
    assert!(world.runtime.drain_futures().is_empty());
}

/// The subscription key the recorded regressions use.
const SUBSCRIPTION_KEY: &str = "sk-ant-oat01-test";

/// A subscription check under recorded settings, credentials and provider.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Regression {
    /// `warning-regression` or `provider-case-boundary`.
    op: String,
    /// The checked model's provider.
    provider: String,
    /// The provider key, when one is stored.
    #[serde(default)]
    key: Option<String>,
    /// How often the check runs.
    #[serde(default)]
    repeats: Option<usize>,
    /// The stored credential's kind, when one is stored.
    #[serde(default)]
    credential: Option<serde_json::Value>,
    /// Whether a key lookup would fail.
    #[serde(default)]
    auth_error: Option<bool>,
    /// The settings in effect.
    #[serde(default)]
    settings: Option<serde_json::Value>,
}

/// Runs one recorded regression as case `case` and checks its notices and lookups.
async fn regression(directory: &Path, case: usize, input: Regression, expected: &Outcome) {
    assert!(["warning-regression", "provider-case-boundary"].contains(&input.op.as_str()));
    let world = started(directory).await;
    if let Some(settings) = input.settings {
        let warnings = serde_json::from_value(settings["warnings"].clone()).unwrap();
        let session = world.host.session();
        session
            .settings_manager()
            .borrow_mut()
            .set_warnings(warnings);
    }
    let key = (!input.auth_error.unwrap_or(false))
        .then(|| input.key.as_deref().unwrap_or(SUBSCRIPTION_KEY));
    for provider in ["anthropic", input.provider.as_str()] {
        key_command(&world, directory, provider, (case, key));
    }
    if let Some(credential) = input.credential {
        assert_eq!(credential, serde_json::json!({"type": "oauth"}));
        sign_in(&world);
    }
    let model = model_named(&world, &input.provider, "claude-sonnet-4-5");
    for _ in 0..input.repeats.unwrap_or(1) {
        check(&world, vec![Some(Arc::clone(&model))]);
    }
    notices(&world, expected, Some(lookups(directory, case)));
}

/// Runs every recorded regression of `test`.
fn regressions(test: &str) {
    isolated(test, async |directory| {
        for (case, (input, expected)) in cases::<Regression, Outcome>(test).into_iter().enumerate()
        {
            regression(directory, case, input, &expected).await;
        }
    });
}

#[test]
fn subscription_warning_runs_once_for_a_subscription_key() {
    regressions("subscription_warning_runs_once_for_a_subscription_key");
}

#[test]
fn stored_sign_in_warns_without_refreshing() {
    regressions("stored_sign_in_warns_without_refreshing");
}

#[test]
fn other_providers_do_not_query_credentials() {
    regressions("other_providers_do_not_query_credentials");
}

#[test]
fn disabled_subscription_warning_does_not_query_credentials() {
    regressions("disabled_subscription_warning_does_not_query_credentials");
}

/// A subscription check with one recorded key kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefixCase {
    /// Always `warning-prefix`.
    op: String,
    /// The key; only text can be stored as a key.
    key: serde_json::Value,
    /// Whether no key is stored at all.
    missing: bool,
}

#[test]
fn subscription_warning_checks_only_the_exact_key_prefix() {
    let test = "subscription_warning_checks_only_the_exact_key_prefix";
    isolated(test, async |directory| {
        for (input, expected) in cases::<PrefixCase, Outcome>(test) {
            assert_eq!(input.op, "warning-prefix");
            let world = started(directory).await;
            if let (false, Some(key)) = (input.missing, input.key.as_str()) {
                let session = world.host.session();
                let auth = session.model_registry().auth_storage();
                auth.set_runtime_api_key("anthropic", key);
            }
            check(&world, vec![Some(subscription_model(&world))]);
            notices(&world, &expected, None);
        }
    });
}

/// A subscription check under one recorded notice preference.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferenceCase {
    /// Always `warning-setting`.
    op: String,
    /// The preference value.
    value: serde_json::Value,
    /// Whether the preference is absent.
    missing: bool,
}

#[test]
fn warning_preferences_disable_only_on_false() {
    let test = "warning_preferences_disable_only_on_false";
    isolated(test, async |directory| {
        for (input, expected) in cases::<PreferenceCase, Outcome>(test) {
            assert_eq!(input.op, "warning-setting");
            let world = started(directory).await;
            let session = world.host.session();
            if !input.missing {
                let mut warnings = serde_json::Map::new();
                warnings.insert("anthropicExtraUsage".into(), input.value);
                let mut settings = session.settings_manager().borrow_mut();
                settings.set_warnings(WarningSettings(warnings));
            }
            let auth = session.model_registry().auth_storage();
            auth.set_runtime_api_key("anthropic", SUBSCRIPTION_KEY);
            check(&world, vec![Some(subscription_model(&world))]);
            notices(&world, &expected, None);
        }
    });
}

/// A named recorded operation without arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    /// The operation's name.
    op: String,
}

#[test]
fn failed_warning_lookup_leaves_the_check_retryable() {
    let test = "failed_warning_lookup_leaves_the_check_retryable";
    isolated(test, async |directory| {
        let (input, expected) = cases::<Operation, Outcome>(test).swap_remove(0);
        assert_eq!(input.op, "auth-failure-then-success");
        let world = started(directory).await;
        key_command(&world, directory, "anthropic", (0, None));
        check(&world, vec![Some(subscription_model(&world))]);
        assert!(!world.mode.anthropic_subscription_warning_shown.get());
        assert!(!shows(&world.mode.chat_container, "Warning: "));
        sign_in(&world);
        check(&world, vec![Some(subscription_model(&world))]);
        notices(&world, &expected, Some(lookups(directory, 0)));
    });
}

/// A check with or without an explicit model.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultCase {
    /// Always `warning-default`.
    op: String,
    /// `missing`, `current`, `explicit` or `already-shown`.
    kind: String,
}

#[test]
fn warning_uses_the_default_model_only_when_not_supplied() {
    let test = "warning_uses_the_default_model_only_when_not_supplied";
    isolated(test, async |directory| {
        for (case, (input, expected)) in cases::<DefaultCase, Outcome>(test).into_iter().enumerate()
        {
            assert_eq!(input.op, "warning-default");
            let world = started(directory).await;
            key_command(
                &world,
                directory,
                "anthropic",
                (case, Some(SUBSCRIPTION_KEY)),
            );
            if input.kind != "missing" {
                let session = world.host.session();
                session.agent().state().write().unwrap().model = subscription_model(&world);
            }
            world
                .mode
                .anthropic_subscription_warning_shown
                .set(input.kind == "already-shown");
            let explicit = (input.kind == "explicit").then(|| model_named(&world, "openai", "gpt"));
            check(&world, vec![explicit]);
            notices(&world, &expected, Some(lookups(directory, case)));
        }
    });
}

#[test]
fn subscription_warning_overlap_is_shown_once() {
    let test = "subscription_warning_overlap_is_shown_once";
    isolated(test, async |directory| {
        let (input, expected) = cases::<Operation, Outcome>(test).swap_remove(0);
        assert_eq!(input.op, "warning-overlap");
        let world = started(directory).await;
        key_command(&world, directory, "anthropic", (0, Some(SUBSCRIPTION_KEY)));
        let model = subscription_model(&world);
        check(&world, vec![Some(Arc::clone(&model)), Some(model)]);
        // The native key lookup finishes without suspending, so the second check finds the
        // notice shown before it looks a key up.
        notices(&world, &expected, None);
        assert_eq!(lookups(directory, 0), 1);
    });
}

/// One caller of the subscription check.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallerCase {
    /// Always `warning-callers`.
    op: String,
    /// The calling route.
    route: String,
}

#[test]
fn model_cycle_reuses_the_subscription_warning_owner() {
    let test = "model_cycle_reuses_the_subscription_warning_owner";
    isolated(test, async |directory| {
        let (input, expected) = cases::<CallerCase, Outcome>(test).swap_remove(0);
        assert_eq!(
            (input.op.as_str(), input.route.as_str()),
            ("warning-callers", "cycle")
        );
        for (case, key) in [(0, SUBSCRIPTION_KEY), (1, "sk-ant-api03-ordinary")] {
            let world = started(directory).await;
            key_command(&world, directory, "anthropic", (case, Some(key)));
            let session = world.host.session();
            session.agent().state().write().unwrap().model = subscription_model(&world);
            run_action(&world, "app.model.cycleForward");
            assert!(world.runtime.drain_futures().is_empty());
            assert!(shows(&world.mode.chat_container, "Switched to"));
            if case == 0 {
                notices(&world, &expected, Some(lookups(directory, case)));
            } else {
                assert!(!shows(&world.mode.chat_container, "Warning: "));
                assert_eq!(lookups(directory, case), 1);
            }
        }
    });
}

/// A catalog provider whose stored sign-in makes every catalog refresh fail.
struct RefusingCatalog;

impl OAuthProviderInterface for RefusingCatalog {
    fn id(&self) -> &'static str {
        "refusing"
    }
    fn name(&self) -> &'static str {
        "Refusing"
    }
    fn login(
        &self,
        _: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::ready(Err(refused())))
    }
    fn refresh_token(
        &self,
        _: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::ready(Err(refused())))
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        Ok(std::borrow::Cow::Borrowed(&credentials.access))
    }
    fn modify_models<'m>(
        &self,
        _: &'m [Model],
        _: &OAuthCredentials,
    ) -> Option<Result<Vec<std::borrow::Cow<'m, Model>>, OAuthError>> {
        Some(Err(refused()))
    }
}

/// The refusing provider's failure.
fn refused() -> OAuthError {
    DiagnosticErrorInfo::error("refresh refused").into()
}

/// Makes every later catalog refresh fail.
pub(super) fn refuse_refresh(world: &World) {
    let session = world.host.session();
    let registry = session.model_registry();
    registry
        .register_provider(
            "refusing",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                base_url: Some("https://example.invalid".into()),
                api: Some("openai-completions".into()),
                callbacks: maestro_app::ProviderCallbacks {
                    oauth: Some(SharedHandle::new(RefusingCatalog)),
                    ..Default::default()
                },
                models: Some(vec![model_input("refused")]),
                ..ProviderConfigInput::default()
            })),
        )
        .unwrap();
    registry.auth_storage().set(
        "refusing",
        AuthCredential::OAuth(OAuthCredentials {
            access: "access".into(),
            refresh: "refresh".into(),
            expires: 0.0,
            extra: serde_json::Map::default(),
        }),
    );
}

/// Makes reading the catalog's available models fail.
pub(super) fn refuse_availability(world: &World) {
    let session = world.host.session();
    let auth = session.model_registry().auth_storage();
    auth.set_fallback_resolver(|_| Err(DiagnosticErrorInfo::error("availability refused").into()));
}

/// Registers the recorded catalog: `z/shared`, `z/tail`, `a/alpha` and `a/shared`.
fn catalog(world: &World) -> Vec<Arc<RwLock<Model>>> {
    [
        register(world, "z", &["shared", "tail"]),
        register(world, "a", &["alpha", "shared"]),
    ]
    .concat()
}

/// Makes the recorded `provider/id` references the session scope, without levels.
pub(super) fn scope(world: &World, references: &[(String, Option<ThinkingLevel>)]) {
    let session = world.host.session();
    let registry = session.model_registry();
    let scope = references
        .iter()
        .map(|(reference, level)| {
            let (provider, id) = reference.split_once('/').unwrap();
            ScopedModel {
                model: registry.find(provider, id).unwrap(),
                thinking_level: level.clone(),
            }
        })
        .collect();
    session.set_scoped_models(scope);
}

/// One recorded candidate list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateCase {
    /// Always `candidates`.
    op: String,
    /// `all`, `scoped`, `unavailable`, `refresh-error` or `empty`.
    kind: String,
}

/// The candidates by `provider/id` or the failure, and whether the catalog was read.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidates {
    /// The candidates in order.
    ids: Option<Vec<String>>,
    /// The failure.
    error: Option<String>,
    /// The catalog reads: a refresh and the available models.
    events: Vec<Event>,
}

/// Prepares the catalog for a recorded candidate kind, returning a descriptor whose
/// replacement shows a catalog refresh.
fn candidate_catalog(world: &World, kind: &str) -> Option<Arc<RwLock<Model>>> {
    match kind {
        "empty" => return None,
        "refresh-error" => refuse_refresh(world),
        _ => {}
    }
    let models = catalog(world);
    match kind {
        "scoped" => scope(
            world,
            &["z/tail", "z/shared", "z/tail"].map(|id| (id.to_owned(), None)),
        ),
        "unavailable" => refuse_availability(world),
        _ => {}
    }
    models.into_iter().next()
}

#[test]
fn model_candidates_keep_scope_order_and_availability_failure_boundary() {
    let test = "model_candidates_keep_scope_order_and_availability_failure_boundary";
    isolated(test, async |directory| {
        for (input, expected) in cases::<CandidateCase, Candidates>(test) {
            assert_eq!(input.op, "candidates");
            let world = started(directory).await;
            let probe = candidate_catalog(&world, &input.kind);
            let result = world.mode.get_model_candidates();
            if let Some(probe) = probe {
                let current = world.host.session().model_registry().find("z", "shared");
                let refreshed = !current.is_some_and(|current| Arc::ptr_eq(&current, &probe));
                assert_eq!(refreshed, named(&expected.events, "refresh").count() == 1);
            }
            match (result, expected.ids, expected.error) {
                (Ok(models), Some(ids), None) if input.kind == "all" => {
                    // The native catalog lists each provider's models together, in
                    // registration order.
                    let mut native: Vec<_> = models.iter().map(id_of).collect();
                    let session = world.host.session();
                    let available = session.model_registry().get_available().unwrap();
                    assert_eq!(native, available.iter().map(id_of).collect::<Vec<_>>());
                    native.sort();
                    let mut ids = ids;
                    ids.sort();
                    assert_eq!(native, ids);
                }
                (Ok(models), Some(ids), None) => {
                    assert_eq!(models.iter().map(id_of).collect::<Vec<_>>(), ids);
                }
                (Err(error), None, Some(message)) => assert_eq!(error.to_string(), message),
                _ => panic!("the recorded outcome kind"),
            }
        }
    });
}

/// The provider count for a recorded scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountCase {
    /// Always `provider-count`.
    op: String,
    /// The session scope.
    scope: Vec<(String, Option<ThinkingLevel>)>,
}

/// Whether the footer prefixes the model with its provider.
fn provider_badge(world: &World) -> bool {
    let provider = world
        .host
        .session()
        .model()
        .read()
        .unwrap()
        .provider
        .clone();
    let footer = world.mode.footer.render(120).concat();
    footer.contains(&format!("({provider}) "))
}

#[test]
fn provider_count_deduplicates_providers_without_reordering_candidates() {
    let test = "provider_count_deduplicates_providers_without_reordering_candidates";
    isolated(test, async |directory| {
        for (input, expected) in cases::<CountCase, Outcome>(test) {
            assert_eq!(input.op, "provider-count");
            let world = started(directory).await;
            assert!(!provider_badge(&world));
            catalog(&world);
            scope(&world, &input.scope);
            world.mode.update_available_provider_count().unwrap();
            let [count] = named(&expected.events, "providers").collect::<Vec<_>>()[..] else {
                panic!("one provider count");
            };
            assert_eq!(count[0].as_f64(), expected.count);
            settled(&world, &expected);
            assert_eq!(provider_badge(&world), expected.count > Some(1.0));
            let candidates = world.mode.get_model_candidates().unwrap();
            if !input.scope.is_empty() {
                let ids: Vec<_> = input.scope.iter().map(|(id, _)| id.clone()).collect();
                assert_eq!(candidates.iter().map(id_of).collect::<Vec<_>>(), ids);
            }
        }
    });
}

/// Runs `/model` with `search` on the terminal runtime.
fn model_command(world: &World, search: Option<String>) {
    let mode = Rc::clone(&world.mode);
    world.runtime.handle().spawn_local(Box::pin(
        async move { mode.handle_model_command(search).await },
    ));
    assert!(world.runtime.drain_futures().is_empty());
}

/// Checks a recorded command outcome: the selected model and its status, or the model
/// panel and its search.
fn command_outcome(world: &World, outcome: &Outcome) {
    if let Some([search]) = named(&outcome.events, "selector-search").next() {
        let panel = model_panel(world).expect("the model panel is shown");
        assert_eq!(
            panel.get_search_input().get_value(),
            search.as_str().unwrap_or_default()
        );
        assert!(named(&outcome.events, "select").next().is_none());
    } else {
        let [selected] = named(&outcome.events, "select").next().unwrap() else {
            panic!("one selected model");
        };
        assert_eq!(current(world), selected.as_str().unwrap());
        let [status] = named(&outcome.events, "status").next().unwrap() else {
            panic!("one status");
        };
        assert!(shows(&world.mode.chat_container, status.as_str().unwrap()));
        assert!(shows_only(world, &editor(world)));
    }
    settled(world, outcome);
}

/// The term the recording searches over `scope`: its repeated reference, or else `alpha`.
fn recorded_search(scope: &[(String, Option<ThinkingLevel>)]) -> String {
    let repeated = scope[0].0 == "z/shared";
    if repeated { "z/shared" } else { "alpha" }.to_owned()
}

/// One recorded `/model` search, or an exact search over a recorded scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandCase {
    /// `model-command` or `scoped-exact-match`.
    op: String,
    /// The search term.
    #[serde(default)]
    search: Option<String>,
    /// Whether the term is absent.
    #[serde(default)]
    missing: Option<bool>,
    /// The session scope.
    #[serde(default)]
    scope: Option<Vec<(String, Option<ThinkingLevel>)>>,
}

#[test]
fn model_command_applies_unique_exact_references_or_keeps_the_search() {
    let test = "model_command_applies_unique_exact_references_or_keeps_the_search";
    isolated(test, async |directory| {
        for (input, expected) in cases::<CommandCase, Outcome>(test) {
            let world = started(directory).await;
            catalog(&world);
            let search = match (input.op.as_str(), input.scope) {
                ("model-command", None) => {
                    assert_eq!(input.missing, Some(input.search.is_none()));
                    input.search
                }
                ("scoped-exact-match", Some(references)) => {
                    scope(&world, &references);
                    Some(recorded_search(&references))
                }
                _ => panic!("a recorded command"),
            };
            model_command(&world, search);
            command_outcome(&world, &expected);
        }
    });
}

/// A failed selection's observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureOutcome {
    /// The model the session holds afterwards, when recorded.
    #[serde(default)]
    selected: Option<String>,
    /// The recorded effects, the error last.
    events: Vec<Event>,
    /// The session scope afterwards.
    scope: Vec<(String, Option<ThinkingLevel>)>,
    /// The footer's provider count, when it was set.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

/// Makes every later session recording fail after the session holds a record.
pub(super) fn refuse_recording(world: &World, directory: &Path) {
    let sessions = directory.join("sessions");
    let cwd = directory.to_str().unwrap();
    let mut manager = SessionManager::create(cwd, sessions.to_str(), cwd).unwrap();
    let seeded = assistant(&serde_json::json!([]), "stop", None);
    drop(manager.append_message(seeded).unwrap());
    *world.host.session().session_manager().borrow_mut() = manager;
    std::fs::remove_dir_all(sessions).unwrap();
}

#[test]
fn model_selection_failure_reports_without_success_effects() {
    let test = "model_selection_failure_reports_without_success_effects";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Operation, FailureOutcome>(test) {
            let world = started(directory).await;
            catalog(&world);
            let previous = current(&world);
            world.mode.editor.set_border_color(Rc::new(str::to_owned));
            let footer = world.mode.footer.render(80);
            match input.op.as_str() {
                "model-command-failed" => {
                    scope(&world, &[("a/alpha".to_owned(), None)]);
                    let session = world.host.session();
                    session.model_registry().unregister_provider("a").unwrap();
                }
                "selection-recording-failure-after-state-change" => {
                    refuse_recording(&world, directory);
                }
                _ => panic!("a recorded failure"),
            }
            model_command(&world, Some("alpha".into()));
            let [kind, _] = expected.events.last().unwrap().as_slice() else {
                panic!("a final error");
            };
            assert_eq!(kind, "error");
            assert!(shows(&world.mode.chat_container, "Error: "));
            assert!(!shows(&world.mode.chat_container, "Model: "));
            assert_eq!(world.mode.editor.render(80)[0], "─".repeat(80));
            let selected = (expected.selected.clone()).map_or(previous, |id| format!("a/{id}"));
            assert_eq!(current(&world), selected);
            if expected.selected.is_none() {
                assert_eq!(world.mode.footer.render(80), footer);
            }
            // A refusal needs an unauthenticated candidate, which only a scope supplies:
            // the catalog lists authenticated models only.
            let refused_scope = input.op == "model-command-failed";
            assert!(expected.scope.is_empty());
            let scope = if refused_scope {
                vec![("a/alpha".to_owned(), None)]
            } else {
                expected.scope
            };
            settled(
                &world,
                &Outcome {
                    events: expected.events,
                    scope,
                    count: expected.count,
                    shown: expected.shown,
                },
            );
        }
    });
}

/// One recorded change a model-selection handler makes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListenerCase {
    /// Always `selection-listener`.
    op: String,
    /// `mutate-id`, `replace-session` or `change-provider`.
    edit: String,
}

/// The change a handler makes to the session's current model.
type Replacement = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// An extension whose model-selection handler makes `edit`, running `replacement` for
/// `replace-session`.
fn editing(edit: String, replacement: Replacement) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (edit, replacement) = (edit.clone(), Rc::clone(&replacement));
        api.on::<ModelSelectEvent, ()>(
            "model_select".into(),
            Rc::new(move |event, _| {
                match edit.as_str() {
                    "mutate-id" => event.model.write().unwrap().id = "changed".into(),
                    "change-provider" => event.model.write().unwrap().provider = "anthropic".into(),
                    _ => replacement.borrow().as_ref().unwrap()(),
                }
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn model_selection_observes_the_selected_descriptor_after_listeners() {
    let test = "model_selection_observes_the_selected_descriptor_after_listeners";
    isolated(test, async |directory| {
        for (case, (input, expected)) in
            cases::<ListenerCase, Outcome>(test).into_iter().enumerate()
        {
            assert_eq!(input.op, "selection-listener");
            let replacement = Rc::default();
            let world = Setup {
                extension: Some(editing(input.edit.clone(), Rc::clone(&replacement))),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().await.unwrap();
            let tail = Arc::clone(&catalog(&world)[1]);
            let session = world.host.session();
            *replacement.borrow_mut() = Some(Box::new(move || {
                session.agent().state().write().unwrap().model = Arc::clone(&tail);
            }));
            key_command(
                &world,
                directory,
                "anthropic",
                (case, Some("sk-ant-api03-ordinary")),
            );
            model_command(&world, Some("alpha".into()));
            let [status] = named(&expected.events, "status").next().unwrap() else {
                panic!("one status");
            };
            assert!(shows(&world.mode.chat_container, status.as_str().unwrap()));
            notices(&world, &expected, Some(lookups(directory, case)));
        }
    });
}

/// One submitted editor text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    /// Always `submit`.
    op: String,
    /// The submitted text.
    text: String,
}

/// Submits `text` exactly through the editor's installed submission, after the editor
/// empties itself as it does before calling back, and runs the work it hands over; typed
/// text would have its tabs widened.
pub(super) fn submit_held(world: &World, text: &str) {
    world.mode.editor.set_text(text);
    world.mode.editor.set_text("");
    submit(&world.mode, text);
    assert!(world.runtime.drain_futures().is_empty());
}

/// Checks one recorded submission: a command clears the editor and runs, other text goes
/// to the waiting input and the history.
fn submission(world: &World, text: &str, outcome: &Outcome) {
    let mut waiting = waiting(&world.mode);
    submit_held(world, text);
    if let Some([unhandled]) = named(&outcome.events, "unhandled").next() {
        let unhandled = unhandled.as_str().unwrap();
        let trimmed = unhandled.trim_matches(maestro_app::is_whitespace_scalar);
        assert_eq!(poll(&mut waiting), Some(trimmed.to_owned()));
        assert_eq!(recalled(&world.mode.editor), [trimmed]);
        return;
    }
    assert!(poll(&mut waiting).is_none());
    assert!(recalled(&world.mode.editor).is_empty());
    let [cleared] = named(&outcome.events, "editor").next().unwrap() else {
        panic!("the editor is cleared");
    };
    assert_eq!(world.mode.editor.get_text(), cleared.as_str().unwrap());
    if named(&outcome.events, "scope-command").next().is_some() {
        assert!(scope_panel(world).is_some());
        return;
    }
    let [search] = named(&outcome.events, "model-command").next().unwrap() else {
        panic!("one model command");
    };
    match search.as_str() {
        Some(search) => assert!(shows(
            &world.mode.chat_container,
            &format!("Model: {search}")
        )),
        None => assert_eq!(
            model_panel(world).unwrap().get_search_input().get_value(),
            ""
        ),
    }
}

#[test]
fn model_commands_use_the_shared_whitespace_and_exact_tokens() {
    let test = "model_commands_use_the_shared_whitespace_and_exact_tokens";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Submission, Outcome>(test) {
            assert_eq!(input.op, "submit");
            let world = started(directory).await;
            catalog(&world);
            submission(&world, &input.text, &expected);
            settled(&world, &expected);
        }
    });
}

#[test]
fn model_commands_leave_waiter_and_history_untouched() {
    isolated(
        "model_commands_leave_waiter_and_history_untouched",
        async |directory| {
            let world = started(directory).await;
            catalog(&world);
            let mut waiting = waiting(&world.mode);
            for command in ["/model alpha", "/model", "/scoped-models"] {
                submit_held(&world, command);
                assert!(poll(&mut waiting).is_none());
            }
            assert!(shows(&world.mode.chat_container, "Model: alpha"));
            assert!(scope_panel(&world).is_some());
            world.mode.editor_container.clear();
            world
                .mode
                .editor_container
                .add_child(world.mode.editor.clone());
            world.mode.ui.set_focus(Some(world.mode.editor.clone()));
            submit_held(&world, "normal");
            assert_eq!(poll(&mut waiting), Some("normal".to_owned()));
            assert_eq!(recalled(&world.mode.editor), ["normal"]);
        },
    );
}

/// A mode whose session can be replaced by a fresh memory session.
pub(super) async fn replaceable(directory: &Path) -> World {
    let world = Setup {
        create: Some(Rc::new(|options| {
            Box::pin(async move { Ok(super::session::session(&options.cwd, None, |inner| inner)) })
        })),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// The default value of `T`, for option records the application root does not name.
fn defaults<T: Default>() -> T {
    T::default()
}

/// Replaces the runtime's session with a fresh one.
pub(super) fn replace_session(world: &World) {
    let host = Rc::clone(&world.host);
    world.runtime.handle().spawn_local(Box::pin(async move {
        host.new_session(defaults()).await?;
        Ok(())
    }));
    assert!(world.runtime.drain_futures().is_empty());
}

#[test]
fn warning_lifetime_survives_rebinding_but_not_another_mode() {
    isolated(
        "warning_lifetime_survives_rebinding_but_not_another_mode",
        async |directory| {
            let world = replaceable(directory).await;
            key_command(&world, directory, "anthropic", (0, Some(SUBSCRIPTION_KEY)));
            check(&world, vec![Some(subscription_model(&world))]);
            assert!(shows(&world.mode.chat_container, "Warning: "));
            let first = world.host.session();
            replace_session(&world);
            assert!(!Rc::ptr_eq(&first, &world.host.session()));
            key_command(&world, directory, "anthropic", (1, Some(SUBSCRIPTION_KEY)));
            world.mode.chat_container.clear();
            check(&world, vec![Some(subscription_model(&world))]);
            assert!(!shows(&world.mode.chat_container, "Warning: "));
            assert_eq!(lookups(directory, 1), 0);
            let other = started(directory).await;
            key_command(&other, directory, "anthropic", (2, Some(SUBSCRIPTION_KEY)));
            check(&other, vec![Some(subscription_model(&other))]);
            assert!(shows(&other.mode.chat_container, "Warning: "));
            assert_eq!(lookups(directory, 2), 1);
        },
    );
}

/// The scope panel the editor container shows, if it shows one.
fn scope_panel(world: &World) -> Option<Rc<ScopedModelsSelectorComponent>> {
    let shown = shown(world);
    let [panel] = shown.as_slice() else {
        return None;
    };
    (Rc::clone(panel) as Rc<dyn Any>)
        .downcast::<ScopedModelsSelectorComponent>()
        .ok()
}

/// The saved model patterns, absent when the setting is unset.
fn saved_patterns(world: &World) -> Option<Vec<String>> {
    let session = world.host.session();
    let entries = session.settings_manager().borrow().get_enabled_models()?;
    let patterns = entries.into_iter().map(|entry| match entry {
        SettingsListEntry::String(pattern) => pattern,
        SettingsListEntry::Unknown(value) => panic!("a saved pattern is text, not {value}"),
    });
    Some(patterns.collect())
}

/// Saves the scope panel's selection with its key and returns what it saved: the
/// panel's enabled models in order, or none when it enables all of them.
fn saved_by_panel(world: &World) -> Option<Vec<String>> {
    world.terminal.send_input("\x13");
    assert!(world.runtime.drain_futures().is_empty());
    assert!(shows(
        &world.mode.chat_container,
        "Model selection saved to settings"
    ));
    saved_patterns(world)
}

/// Layers `enabledModels` over the session's settings as `raw` JSON.
fn enabled_models_setting(world: &World, raw: serde_json::Value) {
    let mut overrides = serde_json::Map::new();
    overrides.insert("enabledModels".into(), raw);
    let session = world.host.session();
    let mut settings = session.settings_manager().borrow_mut();
    settings.apply_overrides(Settings(overrides));
}

/// One recorded opening of the scope panel.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeOpen {
    /// Always `scope-open`.
    op: String,
    /// The recorded scope, settings or catalog source.
    choice: String,
}

/// An opened scope panel's observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Opened {
    /// Whether the panel is shown.
    installed: bool,
    /// The panel's enabled models in order, absent when all are enabled.
    enabled: Option<Vec<String>>,
    /// The opening's failure.
    error: Option<String>,
    /// The recorded effects.
    events: Vec<Event>,
    /// The session scope afterwards.
    scope: Vec<(String, Option<ThinkingLevel>)>,
    /// The footer's provider count, when it was set.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

impl Opened {
    /// The common observations.
    fn outcome(&self) -> Outcome {
        Outcome {
            events: self.events.clone(),
            scope: self.scope.clone(),
            count: self.count,
            shown: self.shown,
        }
    }
}

/// Prepares the catalog, scope and settings of a recorded opening.
fn scope_source(world: &World, choice: &str, scope_after: &[(String, Option<ThinkingLevel>)]) {
    if choice != "empty-catalog" {
        catalog(world);
    }
    let patterns = match choice {
        "session" => {
            scope(world, scope_after);
            return;
        }
        "settings" => serde_json::json!(["z/tail:high", "a/alpha"]),
        "empty-settings" => serde_json::json!([]),
        "all-unmatched" => serde_json::json!(["absent"]),
        "available-failure" => return refuse_availability(world),
        "refresh-failure" => return refuse_refresh(world),
        _ => return,
    };
    enabled_models_setting(world, patterns);
}

/// Checks an opening's panel, enabled models, failure and status.
fn opened(world: &World, result: Result<(), Box<dyn std::error::Error>>, expected: &Opened) {
    match (result, &expected.error) {
        (Ok(()), None) => {}
        (Err(error), Some(message)) => assert_eq!(&error.to_string(), message),
        (result, _) => panic!("the recorded opening outcome, not {result:?}"),
    }
    assert_eq!(scope_panel(world).is_some(), expected.installed);
    let status = named(&expected.events, "status").next();
    assert_eq!(
        shows(&world.mode.chat_container, "No models available"),
        status.is_some()
    );
    settled(world, &expected.outcome());
    if expected.installed {
        assert!(saved_patterns(world).is_none_or(|patterns| patterns != ["z/tail", "a/alpha"]));
        assert_eq!(saved_by_panel(world), expected.enabled);
    }
}

#[test]
fn scope_selector_prefers_session_scope_then_resolved_saved_patterns() {
    let test = "scope_selector_prefers_session_scope_then_resolved_saved_patterns";
    isolated(test, async |directory| {
        for (input, expected) in cases::<ScopeOpen, Opened>(test) {
            assert_eq!(input.op, "scope-open");
            let world = started(directory).await;
            scope_source(&world, &input.choice, &expected.scope);
            let result = world.mode.show_models_selector();
            opened(&world, result, &expected);
        }
    });
}

/// One recorded preference value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldDomain {
    /// Always `scope-field-domain`.
    op: String,
    /// The raw `enabledModels` value.
    raw: serde_json::Value,
    /// Whether the setting is absent.
    missing: bool,
}

#[test]
fn scope_preferences_reject_unknown_members_and_default_malformed_roots() {
    let test = "scope_preferences_reject_unknown_members_and_default_malformed_roots";
    isolated(test, async |directory| {
        for (input, expected) in cases::<FieldDomain, Opened>(test) {
            assert_eq!(input.op, "scope-field-domain");
            let world = started(directory).await;
            catalog(&world);
            if !input.missing {
                enabled_models_setting(&world, input.raw);
            }
            let result = world.mode.show_models_selector();
            let session = world.host.session();
            let global = session.settings_manager().borrow().get_global_settings();
            assert!(
                !global.0.contains_key("enabledModels"),
                "no repair is saved"
            );
            opened(&world, result, &expected);
        }
    });
}

/// One recorded live edit of the scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeChange {
    /// `scope-change` or `scope-change-after-catalog-growth`.
    op: String,
    /// The enabled models, absent for all.
    enabled: Option<Vec<String>>,
    /// The available models when the panel opened.
    #[serde(default)]
    opening: Option<usize>,
    /// The available models when the edit arrives.
    #[serde(default)]
    current: Option<usize>,
}

/// Opens the scope panel over the recorded catalog and applies the recorded edit through
/// the scope callbacks, returning those callbacks.
fn scope_edit(world: &World, input: &ScopeChange) -> ModelsCallbacks {
    catalog(world);
    world.mode.show_models_selector().unwrap();
    let opening = world
        .host
        .session()
        .model_registry()
        .get_available()
        .unwrap()
        .len();
    assert_eq!(input.opening.unwrap_or(opening), opening);
    if let Some(current) = input.current {
        register(world, "new", &["fifth"]);
        let session = world.host.session();
        assert_eq!(
            session.model_registry().get_available().unwrap().len(),
            current
        );
    }
    world.runtime.settle().unwrap();
    let callbacks = world.mode.scope_callbacks(opening, Rc::new(|| {}));
    let edit = (callbacks.on_change)(input.enabled.clone()).unwrap();
    world.runtime.handle().spawn_local(edit);
    assert!(world.runtime.drain_futures().is_empty());
    callbacks
}

#[test]
fn scope_changes_apply_live_without_saving_and_refresh_the_footer() {
    let test = "scope_changes_apply_live_without_saving_and_refresh_the_footer";
    isolated(test, async |directory| {
        for (input, expected) in cases::<ScopeChange, Outcome>(test) {
            assert!(input.op.starts_with("scope-change"));
            let world = started(directory).await;
            scope_edit(&world, &input);
            let [scope] = named(&expected.events, "scope").next().unwrap() else {
                panic!("one scope edit");
            };
            let ids: Vec<_> = scope_of(&world).into_iter().map(|(id, _)| id).collect();
            let recorded: Vec<_> = scope
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry[0].clone())
                .collect();
            assert_eq!(serde_json::json!(ids), serde_json::json!(recorded));
            assert_eq!(
                world.runtime.pending() > 0,
                named(&expected.events, "render").count() > 0
            );
            assert!(saved_patterns(&world).is_none());
            settled(&world, &expected);
        }
    });
}

/// A saved selection's observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveOutcome {
    /// The saved patterns, absent when the setting is removed.
    saved: Option<Vec<String>>,
    /// The recorded effects.
    events: Vec<Event>,
    /// The session scope afterwards.
    scope: Vec<(String, Option<ThinkingLevel>)>,
    /// The footer's provider count, when it was set.
    count: Option<f64>,
    /// Whether the subscription notice was shown.
    shown: bool,
}

#[test]
fn scope_save_preserves_order_and_distinguishes_none_from_all() {
    let test = "scope_save_preserves_order_and_distinguishes_none_from_all";
    isolated(test, async |directory| {
        for (input, expected) in cases::<ScopeChange, SaveOutcome>(test) {
            assert_eq!(input.op, "scope-save");
            let world = started(directory).await;
            let callbacks = scope_edit(&world, &input);
            let [persisted] = named(&expected.events, "persist").next().unwrap() else {
                panic!("one save");
            };
            let save = (callbacks.on_persist)(input.enabled.clone()).unwrap();
            world.runtime.handle().spawn_local(save);
            assert!(world.runtime.drain_futures().is_empty());
            assert_eq!(serde_json::json!(saved_patterns(&world)), *persisted);
            assert_eq!(saved_patterns(&world), expected.saved);
            assert!(shows(
                &world.mode.chat_container,
                "Model selection saved to settings"
            ));
            assert!(scope_panel(&world).is_some(), "saving keeps the panel open");
            settled(
                &world,
                &Outcome {
                    events: expected.events,
                    scope: expected.scope,
                    count: expected.count,
                    shown: expected.shown,
                },
            );
        }
    });
}

#[test]
fn scope_cancel_keeps_live_edits_without_saving() {
    let test = "scope_cancel_keeps_live_edits_without_saving";
    isolated(test, async |directory| {
        let (input, expected) = cases::<Operation, Outcome>(test).swap_remove(0);
        assert_eq!(input.op, "scope-cancel-after-edit");
        let world = started(directory).await;
        let edit = ScopeChange {
            op: "scope-change".into(),
            enabled: Some(vec!["z/tail".into()]),
            opening: None,
            current: None,
        };
        scope_edit(&world, &edit);
        world.runtime.settle().unwrap();
        world.terminal.send_input("\x1b");
        let cancelled = effects(&expected.events);
        assert!(shows_only(&world, &editor(&world)));
        assert_eq!(cancelled.children, ["editor"]);
        assert_eq!(world.runtime.pending() > 0, cancelled.render);
        assert!(editor_focused(&world));
        assert!(saved_patterns(&world).is_none());
        settled(&world, &expected);
    });
}

/// Clears and enables every model with their keys, saving each; the reopened storage keeps
/// the empty selection and then has no setting, and cancelling saves nothing.
async fn bulk_saves_reopen_and_cancel_saves_nothing(world: &World, directory: &Path) {
    world.terminal.send_input("\x18");
    assert_eq!(saved_by_panel(world), Some(Vec::new()));
    assert_eq!(reopened_patterns(directory, world).await, Some(Vec::new()));
    world.terminal.send_input("\x01");
    assert_eq!(saved_by_panel(world), None);
    assert_eq!(reopened_patterns(directory, world).await, None);
    world.terminal.send_input("\x18");
    world.terminal.send_input("\x1b");
    assert!(world.runtime.drain_futures().is_empty());
    assert!(shows_only(world, &editor(world)));
    assert_eq!(
        reopened_patterns(directory, world).await,
        None,
        "cancelling saves nothing"
    );
}

/// The patterns a reopened file-backed settings manager in `directory` reads.
async fn reopened_patterns(directory: &Path, world: &World) -> Option<Vec<String>> {
    let session = world.host.session();
    let flushed = session.settings_manager().borrow().flush();
    flushed.await;
    let root = directory.to_str().unwrap();
    let reopened = SettingsManager::create(root, root, root);
    reopened.get_enabled_models().map(|entries| {
        entries
            .into_iter()
            .map(|entry| match entry {
                SettingsListEntry::String(pattern) => pattern,
                SettingsListEntry::Unknown(value) => panic!("a saved pattern is text, not {value}"),
            })
            .collect()
    })
}

#[test]
fn model_scope_persistence_flushes_and_reopens_in_order() {
    isolated(
        "model_scope_persistence_flushes_and_reopens_in_order",
        async |directory| {
            let world = started(directory).await;
            let root = directory.to_str().unwrap();
            *world.host.session().settings_manager().borrow_mut() =
                SettingsManager::create(root, root, root);
            let ids: Vec<_> = catalog(&world).iter().map(id_of).collect();
            submit_held(&world, "/scoped-models");
            assert!(scope_panel(&world).is_some());
            for key in ["\r", "\x1b[B", "\r"] {
                world.terminal.send_input(key);
            }
            assert!(world.runtime.drain_futures().is_empty());
            assert_eq!(scope_of(&world).len(), 2);
            assert_eq!(
                reopened_patterns(directory, &world).await,
                None,
                "live edits are not saved"
            );
            let saved = saved_by_panel(&world).unwrap();
            assert_eq!(saved.len(), 2);
            assert!(
                ids.iter().filter(|id| saved.contains(id)).eq(saved.iter()),
                "order kept"
            );
            assert_eq!(reopened_patterns(directory, &world).await, Some(saved));
            bulk_saves_reopen_and_cancel_saves_nothing(&world, directory).await;
        },
    );
}

#[test]
fn scope_resolution_warnings_reach_standard_error() {
    let test = "scope_resolution_warnings_reach_standard_error";
    // A further child opens the panel, so its standard error reaches this parent unmixed.
    if maestro_test_process::selection().is_child() {
        super::isolated_case(test, "warning", &[], async |directory| {
            let world = started(directory).await;
            catalog(&world);
            enabled_models_setting(&world, serde_json::json!(["missing/none", "a/alpha:bogus"]));
            submit_held(&world, "/scoped-models");
            assert!(scope_panel(&world).is_some());
        });
    }
    let lines = [
        "Warning: No models match pattern \"missing/none\"",
        "Warning: Invalid thinking level \"bogus\" in pattern \"a/alpha:bogus\". \
         Using default instead.",
    ];
    // Captured streams: only a forced colour level styles the warnings.
    let rows: [(&[(&str, &str)], bool); 4] = [
        (&[], false),
        (&[("FORCE_COLOR", "1")], true),
        (&[("FORCE_COLOR", "0")], false),
        (&[("TERM", "dumb")], false),
    ];
    isolated(test, async |_| {
        for (envs, yellow) in rows {
            let output = TestProcess::current(test)
                .unwrap()
                .case("warning")
                .envs(envs.iter().copied())
                .output()
                .unwrap();
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(output.status.success(), "{stderr}");
            let (open, close) = if yellow {
                ("\u{1b}[33m", "\u{1b}[39m")
            } else {
                ("", "")
            };
            let expected: String = lines
                .iter()
                .map(|line| [open, line, close, "\n"].concat())
                .collect();
            assert_eq!(stderr, expected, "{envs:?}");
        }
    });
}
