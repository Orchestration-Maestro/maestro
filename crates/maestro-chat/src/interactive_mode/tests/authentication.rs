//! Signing in and removing saved provider credentials through the installed `/login`
//! and `/logout` commands.

mod completion;
mod navigation;
mod subscription;

use super::actions::install;
use super::models::{
    editor, editor_focused, refuse_availability, refuse_refresh, register_configured,
    replace_session, replaceable, scope, shows_only, submit_held,
};
use super::typed::{poll, recalled, submit, waiting};
use super::{World, isolated, isolated_case, started_with};
use crate::components::session_selector::list::tests::session_picker::decode_cases;
use crate::{
    AuthSelectorProvider, AuthType, ExtensionSelectorComponent, LoginDialogComponent,
    OAuthSelectorComponent,
};
use maestro_app::{
    AuthCredential, AuthStorage, BoxFuture, Fetch, Model, OAuthCallbacks, OAuthCredentials,
    OAuthError, OAuthProviderInterface, ReadonlyFooterDataProvider, SharedHandle,
};
use maestro_tui::{Component, Focusable, KeybindingKeys};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use std::{
    any::Any,
    borrow::Cow,
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Every recorded logout case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_logout.json");

/// The width rows are read at, wide enough to keep every recorded notice on one row.
const WIDE: usize = 400;

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    decode_cases(FIXTURES, test)
}

/// Whether the transcript holds `text`, read at a width that keeps a long notice whole.
fn notes(world: &World, text: &str) -> bool {
    world
        .mode
        .chat_container
        .render(WIDE)
        .concat()
        .contains(text)
}

/// The world's current credential store.
fn storage(world: &World) -> Arc<AuthStorage> {
    Arc::clone(world.host.session().model_registry().auth_storage())
}

/// The stored identities of the current session, in accepted order.
fn accepted(world: &World) -> Vec<String> {
    storage(world).list()
}

/// The footer's provider count.
fn count(world: &World) -> f64 {
    world
        .mode
        .footer_data_provider
        .get_available_provider_count()
}

/// The panel of type `T` the editor container shows alone, if it shows one.
fn shown_as<T: Any>(world: &World) -> Option<Rc<T>> {
    let shown = world.mode.editor_container.children().borrow().clone();
    let [panel] = shown.as_slice() else {
        return None;
    };
    (Rc::clone(panel) as Rc<dyn Any>).downcast::<T>().ok()
}

/// The credential picker the editor container shows, if it shows one.
fn picker(world: &World) -> Option<Rc<OAuthSelectorComponent>> {
    shown_as(world)
}

/// The sign-in method choice or a subscription's choice, if one is shown.
fn chooser(world: &World) -> Option<Rc<ExtensionSelectorComponent>> {
    shown_as(world)
}

/// The sign-in dialog the editor container shows.
fn dialog(world: &World) -> Rc<LoginDialogComponent> {
    shown_as(world).expect("the sign-in dialog is shown")
}

/// The rows the editor container shows, read wide enough to keep each notice whole.
fn panel_rows(world: &World) -> String {
    world.mode.editor_container.render(WIDE).join("\n")
}

/// Replaces the browser openers with ones that do nothing, so an authorization link
/// opens no browser.
fn quiet_browser(directory: &Path) {
    for name in ["xdg-open", "open"] {
        install(directory, name, "#!/bin/sh\n");
    }
}

/// Runs the terminal runtime's work until it waits, expecting no unhandled failure.
fn settle(world: &World) {
    let errors = world.runtime.drain_futures();
    assert!(errors.is_empty(), "no unhandled failure: {errors:?}");
}

/// Submits `/login` and chooses the sign-in method for `kind`.
fn choose_method(world: &World, kind: AuthType) {
    submit_held(world, "/login");
    assert!(chooser(world).is_some(), "the method choice is shown");
    if kind == AuthType::ApiKey {
        world.terminal.send_input("\x1b[B");
    }
    world.terminal.send_input("\r");
}

/// Chooses `id` among the providers offered for `kind` and runs the sign-in it starts
/// until it waits.
fn choose_provider(world: &World, kind: AuthType, id: &str) {
    let options = world.mode.get_login_provider_options(Some(kind)).unwrap();
    let position = options
        .iter()
        .position(|option| option.id == id)
        .expect("the provider is offered");
    assert!(picker(world).is_some(), "the provider picker is shown");
    for _ in 0..position {
        world.terminal.send_input("\x1b[B");
    }
    world.terminal.send_input("\r");
    settle(world);
}

/// Signs in to `id` with `kind` through the installed command until the sign-in waits.
fn sign_in_to(world: &World, kind: AuthType, id: &str) {
    choose_method(world, kind);
    choose_provider(world, kind, id);
}

/// Pastes `text` into the focused input, submits it and runs the work that follows.
fn answer(world: &World, text: &str) {
    world
        .terminal
        .send_input(&format!("\x1b[200~{text}\x1b[201~"));
    world.terminal.send_input("\r");
    settle(world);
}

/// What a scripted subscription does when asked to sign in.
type Script =
    Arc<dyn Fn(OAuthCallbacks) -> BoxFuture<Result<OAuthCredentials, OAuthError>> + Send + Sync>;

/// A subscription whose sign-in runs a test script.
struct Scripted {
    /// The display name.
    name: String,
    /// Whether it also accepts a pasted redirect.
    server: Option<bool>,
    /// The sign-in.
    script: Script,
}

impl OAuthProviderInterface for Scripted {
    fn id(&self) -> &'static str {
        "scripted"
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn uses_callback_server(&self) -> Option<bool> {
        self.server
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        (self.script)(callbacks)
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
    ) -> Result<Cow<'a, str>, OAuthError> {
        Ok(Cow::Borrowed(&credentials.access))
    }
}

/// Registers the subscription `id`, named `name`, whose sign-in runs `script`.
fn subscribe(
    world: &World,
    (id, name): (&str, &str),
    server: Option<bool>,
    script: impl Fn(OAuthCallbacks) -> BoxFuture<Result<OAuthCredentials, OAuthError>>
    + Send
    + Sync
    + 'static,
) {
    let scripted = SharedHandle::new(Scripted {
        name: name.to_owned(),
        server,
        script: Arc::new(script),
    });
    register_configured(world, id, &[], move |config| {
        config.callbacks.oauth = Some(scripted);
    });
}

/// Subscription credentials whose key is `access`.
fn signed(access: &str) -> OAuthCredentials {
    OAuthCredentials {
        access: access.to_owned(),
        refresh: "refresh".to_owned(),
        expires: 4_102_444_800_000.0,
        extra: Map::default(),
    }
}

/// The stored subscription key of `id`, if one is stored.
fn stored_access(world: &World, id: &str) -> Option<String> {
    match storage(world).get(id)? {
        AuthCredential::OAuth(credentials) => Some(credentials.access),
        AuthCredential::ApiKey(_) => None,
    }
}

/// The stored API key of `id`, if one is stored.
fn stored_key(world: &World, id: &str) -> Option<String> {
    match storage(world).get(id)? {
        AuthCredential::ApiKey(credential) => Some(credential.key),
        AuthCredential::OAuth(_) => None,
    }
}

/// Where the success text says credentials are saved.
fn saved_path() -> String {
    maestro_app::configuration::config::paths::get_auth_path(
        &maestro_app::configuration::config::get_agent_dir().unwrap(),
    )
}

/// Whether the editor alone is shown again.
fn editor_restored(world: &World) -> bool {
    shows_only(world, &editor(world))
}

/// Submits `/logout` through the installed handler and returns the picker it shows.
fn open(world: &World) -> Rc<OAuthSelectorComponent> {
    submit_held(world, "/logout");
    assert_eq!(
        world.mode.editor.get_text(),
        "",
        "the command clears the editor"
    );
    picker(world).expect("the saved credentials are listed")
}

/// The position of `selected` among the choices the picker captures.
fn position(world: &World, selected: &str) -> usize {
    world
        .mode
        .logout_provider_options()
        .unwrap()
        .iter()
        .position(|option| option.id == selected)
        .expect("the recorded identity is saved")
}

/// Moves the picker to `position` with its own key bindings, confirms it and settles the
/// removal it starts.
fn select(world: &World, position: usize) {
    assert!(
        !world.mode.editor.focus_flag().get(),
        "the picker holds the keyboard until the removal starts"
    );
    select_on(
        world,
        &picker(world).expect("the picker is shown"),
        position,
    );
}

/// Moves `panel` to `position`, confirms it and settles the removal it starts, then the
/// frame it requested. The terminal holds only what the removal painted.
fn select_on(world: &World, panel: &OAuthSelectorComponent, position: usize) {
    for _ in 0..position {
        panel.handle_input("\x1b[B").unwrap();
    }
    world.runtime.settle().unwrap();
    world.terminal.clear_writes();
    panel.handle_input("\r").unwrap();
    assert!(
        world.runtime.drain_futures().is_empty(),
        "the removal reports no unhandled failure"
    );
    world.runtime.settle().unwrap();
}

/// Whether the terminal was written `text`, however the frame wrapped it: every word of
/// `text` appears, in order, in what the terminal received.
fn painted(world: &World, text: &str) -> bool {
    let frame = maestro_app::strip_ansi(&world.terminal.writes().concat()).unwrap();
    let mut rest = frame.as_str();
    text.split_whitespace().all(|word| {
        rest.find(word).is_some_and(|at| {
            rest = &rest[at + word.len()..];
            true
        })
    })
}

/// The authentication kind of `option` as its case records it.
fn kind(option: &AuthSelectorProvider) -> &'static str {
    match option.auth_type {
        AuthType::OAuth => "oauth",
        AuthType::ApiKey => "api_key",
    }
}

/// One recorded saved credential.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    /// Provider identity.
    id: String,
    /// The credential in its stored form.
    record: Value,
    /// The provider's display name; the identity when absent.
    #[serde(default)]
    name: Option<String>,
}

/// One recorded catalog provider.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Provider {
    /// Provider identity.
    id: String,
    /// Model identities, in catalog order.
    #[serde(default)]
    models: Vec<String>,
    /// The display name; the identity when absent.
    #[serde(default)]
    name: Option<String>,
    /// Whether a configured key makes the provider available.
    #[serde(default)]
    key: bool,
    /// Whether a subscription registration carries the provider without a configured key.
    #[serde(default)]
    oauth_only: bool,
}

/// One recorded choice.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Choice {
    /// Provider identity.
    id: String,
    /// Displayed name.
    name: String,
    /// `oauth` or `api_key`.
    auth_type: String,
}

/// One recorded stored-choices list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Listing {
    /// The recorded case.
    case: String,
    /// The saved credentials, in their stored order.
    #[serde(default)]
    stored: Vec<Stored>,
    /// Providers authenticated outside the store.
    #[serde(default)]
    configured: Vec<Provider>,
}

/// The ordered choices a list produces.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Listed {
    /// Every choice, in order.
    options: Vec<Choice>,
}

/// One recorded submitted text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    /// The recorded case.
    case: String,
    /// The submitted text.
    text: String,
    /// Whether a credential is saved.
    saved: bool,
}

/// What one submission reached.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admitted {
    /// Whether the picker is shown.
    selector: bool,
    /// The exact notice, when the empty state is reported.
    #[serde(default)]
    status: Option<String>,
    /// The trimmed text delivered as ordinary input, when it was not a command.
    #[serde(default)]
    ordinary: Option<String>,
}

/// One recorded removal.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Removal {
    /// The recorded case.
    case: String,
    /// The chosen provider identity.
    selected: String,
    /// The catalog providers.
    #[serde(default)]
    providers: Vec<Provider>,
    /// The saved credentials.
    #[serde(default)]
    stored: Vec<Stored>,
    /// The scoped model references; unscoped when empty.
    #[serde(default)]
    scoped: Vec<String>,
    /// The replacement session's saved credentials.
    #[serde(default)]
    replacement: Vec<Stored>,
}

/// What one removal left behind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Removed {
    /// The footer's provider count.
    count: f64,
    /// The exact success text, when the success path ran.
    #[serde(default)]
    message: Option<String>,
    /// The exact failure text, when the failure path ran.
    #[serde(default)]
    error: Option<String>,
    /// The stored identities left on the current session.
    accepted: Vec<String>,
    /// The stored identities left on the session the choices were captured on.
    #[serde(default)]
    original: Vec<String>,
    /// The identities the backing bytes still hold.
    #[serde(default)]
    persisted: Vec<String>,
}

/// Writes the case's saved credentials into its own `auth.json` and opens the store over
/// it, so a record that does not decode stays listed but produces no credential.
fn document_store(directory: &Path, case: usize, stored: &[Stored]) -> Arc<AuthStorage> {
    let root = directory.join(format!("store{case}"));
    std::fs::create_dir(&root).unwrap();
    let document: Map<String, Value> = stored
        .iter()
        .map(|entry| (entry.id.clone(), entry.record.clone()))
        .collect();
    let path = root.join("auth.json");
    std::fs::write(&path, format!("{:#}", Value::Object(document))).unwrap();
    Arc::new(AuthStorage::create(path.to_str().unwrap()))
}

/// Registers one recorded provider.
fn register_one(world: &World, provider: &Provider) {
    let ids: Vec<&str> = provider.models.iter().map(String::as_str).collect();
    let (name, key, oauth) = (
        provider.name.clone(),
        provider.key.then(|| "fixture".to_owned()),
        provider.oauth_only.then(|| {
            SharedHandle::new(inert_subscription())
                as SharedHandle<dyn OAuthProviderInterface + Send + Sync>
        }),
    );
    register_configured(world, &provider.id, &ids, move |config| {
        config.name = name;
        config.api_key = key;
        config.callbacks.oauth = oauth;
    });
}

/// Registers `provider` as a catalog provider that names its providers only.
fn register_named(world: &World, provider: &str, name: Option<String>) {
    register_configured(world, provider, &[], move |config| config.name = name);
}

/// Stores one recorded credential on the current session.
fn store(world: &World, entry: &Stored) {
    let credential: AuthCredential = serde_json::from_value(entry.record.clone()).unwrap();
    storage(world).set(&entry.id, credential);
}

/// Stores a saved API key for `provider`.
fn store_key(world: &World, provider: &str) {
    let credential = serde_json::from_value(json!({"type": "api_key", "key": "saved"})).unwrap();
    storage(world).set(provider, credential);
}

/// Builds the recorded catalog, saved credentials and scope on the current session.
fn prepare(world: &World, input: &Removal) {
    for provider in &input.providers {
        register_one(world, provider);
    }
    for entry in &input.stored {
        store(world, entry);
    }
    if !input.scoped.is_empty() {
        let references = input
            .scoped
            .iter()
            .map(|reference| (reference.clone(), None))
            .collect::<Vec<_>>();
        scope(world, &references);
    }
}

/// Checks the recorded outcome of a removal.
fn removed(world: &World, expected: &Removed) {
    assert!(editor_restored(world), "the editor comes back first");
    assert!(
        world.runtime.drain_futures().is_empty(),
        "the removal reports no unhandled failure"
    );
    assert_eq!(count(world).to_bits(), expected.count.to_bits());
    assert_eq!(accepted(world), expected.accepted);
    match (&expected.message, &expected.error) {
        (Some(message), None) => {
            assert!(notes(world, message), "the chosen success text is shown");
            assert!(
                painted(world, message),
                "the success text reaches the terminal"
            );
            assert!(!notes(world, "Error: "), "no failure is shown");
        }
        (None, Some(error)) => {
            assert!(
                notes(world, &format!("Error: {error}")),
                "the failure is shown"
            );
            assert!(
                painted(world, &format!("Error: {error}")),
                "the failure reaches the terminal"
            );
            assert!(!notes(world, "Logged out of") && !notes(world, "Removed stored API key"));
        }
        _ => panic!("a removal either succeeds or fails"),
    }
    assert!(
        editor_focused(world),
        "the keyboard is back on the editor after the outcome"
    );
}

#[test]
fn logout_options_keep_saved_kinds_and_stable_names() {
    const TEST: &str = "logout_options_keep_saved_kinds_and_stable_names";
    // The fixture orders `Ångström` before `zebra`, as English collation does. The child's
    // environment is cleared already; the pin states the assumption where it is made.
    isolated_case(
        TEST,
        "child",
        &[("LC_ALL", "en_US.UTF-8")],
        async |directory| {
            for (index, (input, expected)) in cases::<Listing, Listed>(TEST).into_iter().enumerate()
            {
                assert!(!input.case.is_empty());
                let auth = document_store(directory, index, &input.stored);
                let world = started_with(directory, Some(auth)).await;
                for provider in &input.configured {
                    register_one(&world, provider);
                }
                for entry in &input.stored {
                    register_named(&world, &entry.id, entry.name.clone());
                }
                let options = world.mode.logout_provider_options().unwrap();
                let listed: Vec<_> = options
                    .iter()
                    .map(|option| (option.id.as_str(), option.name.as_str(), kind(option)))
                    .collect();
                let recorded: Vec<_> = expected
                    .options
                    .iter()
                    .map(|choice| {
                        (
                            choice.id.as_str(),
                            choice.name.as_str(),
                            choice.auth_type.as_str(),
                        )
                    })
                    .collect();
                assert_eq!(listed, recorded, "{}", input.case);
                if options.is_empty() {
                    submit_held(&world, "/logout");
                    assert!(picker(&world).is_none(), "{}", input.case);
                    continue;
                }
                let panel = open(&world);
                let rows = panel.render(WIDE).concat();
                let mut read = 0;
                for choice in &expected.options {
                    let at = rows[read..]
                        .find(&choice.name)
                        .unwrap_or_else(|| panic!("{}: {} is listed", input.case, choice.name));
                    read += at + choice.name.len();
                }
            }
        },
    );
}

#[test]
fn logout_empty_command_restores_editor_and_reports_scope() {
    const TEST: &str = "logout_empty_command_restores_editor_and_reports_scope";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Submission, Admitted>(TEST) {
            let world = started_with(directory, None).await;
            if input.saved {
                saved_credential(&world);
            }
            admit(&world, &input, &expected);
        }
    });
}

/// Registers the catalog provider `a` and saves an API key for it.
fn saved_credential(world: &World) {
    register_one(
        world,
        &Provider {
            id: "a".into(),
            models: vec!["one".into()],
            name: None,
            key: true,
            oauth_only: false,
        },
    );
    store_key(world, "a");
}

/// Checks what one submitted text reached through the installed handler.
fn admit(world: &World, input: &Submission, expected: &Admitted) {
    let mut waiter = waiting(&world.mode);
    world.mode.editor.set_text(&input.text);
    submit(&world.mode, &input.text);
    if let Some(ordinary) = &expected.ordinary {
        assert_eq!(poll(&mut waiter), Some(ordinary.clone()), "{}", input.case);
        assert_eq!(recalled(&world.mode.editor), std::slice::from_ref(ordinary));
        assert!(picker(world).is_none(), "{}", input.case);
    } else {
        assert_eq!(poll(&mut waiter), None, "{}", input.case);
        assert!(recalled(&world.mode.editor).is_empty(), "{}", input.case);
        assert_eq!(picker(world).is_some(), expected.selector, "{}", input.case);
    }
    if expected.selector || expected.ordinary.is_some() {
        assert_eq!(world.mode.editor.get_text(), "", "{}", input.case);
    }
    if let Some(status) = &expected.status {
        assert!(notes(world, status), "{}", input.case);
        assert!(picker(world).is_none());
        assert!(editor_focused(world), "the editor keeps focus");
    } else {
        assert!(!notes(world, &status_notice()));
    }
}

/// The empty-state notice the picker's absence reports.
fn status_notice() -> String {
    let (input, expected) =
        cases::<Submission, Admitted>("logout_empty_command_restores_editor_and_reports_scope")
            .into_iter()
            .find(|(input, _)| input.case == "nothing-saved")
            .unwrap();
    assert!(!input.saved);
    expected.status.unwrap()
}

#[test]
fn logout_selection_removes_only_chosen_saved_credential() {
    const TEST: &str = "logout_selection_removes_only_chosen_saved_credential";
    isolated(TEST, async |directory| {
        for (index, (input, expected)) in cases::<Removal, Removed>(TEST).into_iter().enumerate() {
            // The credentials live in a file and a model configuration sits in the agent
            // directory: a removal may change only the file.
            let auth = document_store(directory, index, &input.stored);
            let configuration = directory.join("models.json");
            std::fs::write(&configuration, "{\n  \"providers\": {}\n}\n").unwrap();
            let world = started_with(directory, Some(auth)).await;
            prepare(&world, &input);
            let chosen = position(&world, &input.selected);
            open(&world);
            if input.case == "captured-kind-and-name" {
                register_named(&world, "a", Some("Replaced name".into()));
                store(
                    &world,
                    &Stored {
                        id: "a".into(),
                        record: json!({
                            "type": "oauth",
                            "access": "access",
                            "refresh": "refresh",
                            "expires": 1
                        }),
                        name: None,
                    },
                );
            }
            select(&world, chosen);
            removed(&world, &expected);
            let backing = directory.join(format!("store{index}")).join("auth.json");
            assert_stored_without(&backing, &input);
            assert_eq!(
                std::fs::read_to_string(&configuration).unwrap(),
                "{\n  \"providers\": {}\n}\n",
                "the model configuration is unchanged"
            );
        }
    });
}

/// Checks that the file at `backing` holds every saved credential of `input` but the
/// chosen one, each as it was recorded.
fn assert_stored_without(backing: &Path, input: &Removal) {
    let document: Map<String, Value> =
        serde_json::from_str(&std::fs::read_to_string(backing).unwrap()).unwrap();
    let mut kept: Vec<&str> = document.keys().map(String::as_str).collect();
    kept.sort_unstable();
    let mut expected: Vec<&str> = input
        .stored
        .iter()
        .map(|entry| entry.id.as_str())
        .filter(|id| *id != input.selected)
        .collect();
    expected.sort_unstable();
    assert_eq!(kept, expected, "the file loses the chosen credential only");
    for entry in input
        .stored
        .iter()
        .filter(|entry| entry.id != input.selected)
    {
        assert_eq!(
            document[&entry.id], entry.record,
            "{} is kept as it was",
            entry.id
        );
    }
}

#[test]
fn logout_count_uses_scoped_or_available_candidates() {
    const TEST: &str = "logout_count_uses_scoped_or_available_candidates";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Removal, Removed>(TEST) {
            let world = started_with(directory, None).await;
            prepare(&world, &input);
            let chosen = position(&world, &input.selected);
            open(&world);
            select(&world, chosen);
            removed(&world, &expected);
        }
    });
}

#[test]
fn logout_selection_uses_current_session_with_captured_choice() {
    const TEST: &str = "logout_selection_uses_current_session_with_captured_choice";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Removal, Removed>(TEST) {
            let world = replaceable(directory).await;
            prepare(&world, &input);
            let chosen = position(&world, &input.selected);
            // The replacement's extension UI reset puts the editor back in the panel's place.
            let panel = open(&world);
            let captured = world.host.session();
            replace_session(&world);
            for entry in &input.replacement {
                store(&world, entry);
            }
            register_one(
                &world,
                &Provider {
                    id: "second-session".into(),
                    models: vec!["one".into()],
                    name: None,
                    key: true,
                    oauth_only: false,
                },
            );
            select_on(&world, &panel, chosen);
            removed(&world, &expected);
            let original: Vec<String> = Arc::clone(captured.model_registry().auth_storage()).list();
            assert_eq!(
                original, expected.original,
                "the first session keeps its own"
            );
        }
    });
}

/// A subscription registration that needs no stored credential, so its provider stays
/// unavailable without one.
fn inert_subscription() -> impl OAuthProviderInterface {
    RefusingAfter {
        succeeds: AtomicUsize::new(0),
    }
}

/// A catalog provider whose stored sign-in fails every refresh after `succeeds` of them.
struct RefusingAfter {
    /// Successful refreshes left.
    succeeds: AtomicUsize,
}

impl OAuthProviderInterface for RefusingAfter {
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
    ) -> Result<Cow<'a, str>, OAuthError> {
        Ok(Cow::Borrowed(&credentials.access))
    }
    fn modify_models<'m>(
        &self,
        _: &'m [Model],
        _: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, Model>>, OAuthError>> {
        let left = self.succeeds.load(Ordering::Relaxed);
        if left == 0 {
            return Some(Err(refused()));
        }
        self.succeeds.store(left - 1, Ordering::Relaxed);
        None
    }
}

/// The refusing provider's failure.
fn refused() -> OAuthError {
    maestro_app::DiagnosticErrorInfo::error("refresh refused").into()
}

/// Arms a catalog whose refresh succeeds `succeeds` more times before failing, and stores
/// the sign-in its transformation reads.
fn refuse_after(world: &World, succeeds: usize) {
    let oauth = SharedHandle::new(RefusingAfter {
        succeeds: AtomicUsize::new(succeeds),
    });
    register_configured(world, "refusing", &["one"], move |config| {
        config.callbacks.oauth = Some(oauth);
    });
    let credential = json!({
        "type": "oauth",
        "access": "access",
        "refresh": "refresh",
        "expires": 1
    });
    storage(world).set("refusing", serde_json::from_value(credential).unwrap());
}

#[test]
fn logout_failures_keep_reached_effects_and_restore_focus() {
    const TEST: &str = "logout_failures_keep_reached_effects_and_restore_focus";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Removal, Removed>(TEST) {
            let world = started_with(directory, None).await;
            prepare(&world, &input);
            let chosen = position(&world, &input.selected);
            open(&world);
            match input.case.as_str() {
                "first-refresh" => refuse_refresh(&world),
                "second-refresh" => refuse_after(&world, 1),
                other => panic!("{other} is a recorded failure"),
            }
            select(&world, chosen);
            removed(&world, &expected);
        }
    });
}

#[test]
fn logout_unavailable_candidates_set_zero_without_error() {
    const TEST: &str = "logout_unavailable_candidates_set_zero_without_error";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Removal, Removed>(TEST) {
            assert_eq!(input.case.as_str(), "available-failure");
            let world = started_with(directory, None).await;
            prepare(&world, &input);
            let chosen = position(&world, &input.selected);
            open(&world);
            refuse_availability(&world);
            select(&world, chosen);
            removed(&world, &expected);
        }
    });
}

#[test]
fn logout_persistence_failure_remains_a_store_diagnostic() {
    const TEST: &str = "logout_persistence_failure_remains_a_store_diagnostic";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Removal, Removed>(TEST) {
            assert_eq!(input.case.as_str(), "write-failure");
            let auth = document_store(directory, 0, &input.stored);
            let world = started_with(directory, Some(Arc::clone(&auth))).await;
            let root = directory.join("store0");
            let Some(_restored) =
                maestro_test_directory::deny(root.join("auth.json"), 0o400).unwrap()
            else {
                return;
            };
            let chosen = position(&world, &input.selected);
            open(&world);
            select(&world, chosen);
            assert!(editor_restored(&world));
            assert!(notes(&world, expected.message.as_deref().unwrap()));
            assert_eq!(
                accepted(&world),
                expected.accepted,
                "memory loses the credential"
            );
            let backing = std::fs::read_to_string(root.join("auth.json")).unwrap();
            let document: Map<String, Value> = serde_json::from_str(&backing).unwrap();
            let mut persisted: Vec<String> = document.keys().cloned().collect();
            persisted.sort();
            assert_eq!(persisted, expected.persisted, "the file keeps both records");
            assert_eq!(
                auth.drain_errors().len(),
                1,
                "one write failure stays drainable"
            );
            assert_eq!(count(&world).to_bits(), expected.count.to_bits());
        }
    });
}

#[test]
fn logout_cancel_keeps_saved_credentials_and_restores_editor() {
    const TEST: &str = "logout_cancel_keeps_saved_credentials_and_restores_editor";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<Removal, Removed>(TEST) {
            let world = started_with(directory, None).await;
            prepare(&world, &input);
            let panel = open(&world);
            world.terminal.clear_writes();
            panel.handle_input("\x1b").unwrap();
            world.runtime.settle().unwrap();
            assert!(editor_restored(&world), "the editor comes back");
            assert!(!world.terminal.writes().is_empty(), "a frame is requested");
            assert_eq!(accepted(&world), expected.accepted);
            assert_eq!(count(&world).to_bits(), expected.count.to_bits());
            assert!(!notes(&world, "Logged out of"));
            assert!(!notes(&world, "Error: "));
            assert_eq!(
                world.runtime.drain_futures().len(),
                0,
                "cancelling starts nothing"
            );
        }
    });
}

#[test]
fn logout_cancel_follows_the_configured_key_and_leaves_the_file_as_it_was() {
    isolated(
        "logout_cancel_follows_the_configured_key_and_leaves_the_file_as_it_was",
        async |directory| {
            let recorded = "logout_cancel_keeps_saved_credentials_and_restores_editor";
            for (index, (input, expected)) in
                cases::<Removal, Removed>(recorded).into_iter().enumerate()
            {
                let auth = document_store(directory, index, &input.stored);
                let world = started_with(directory, Some(auth)).await;
                prepare(&world, &input);
                let backing = directory.join(format!("store{index}")).join("auth.json");
                let before = std::fs::read(&backing).unwrap();
                // Cancel is Ctrl+X here, so Escape and Ctrl+C stop cancelling.
                maestro_tui::get_keybindings().set_user_bindings(vec![(
                    "tui.select.cancel".to_owned(),
                    Some(KeybindingKeys::Single("ctrl+x".to_owned())),
                )]);
                let panel = open(&world);
                for old in ["\x1b", "\x03"] {
                    panel.handle_input(old).unwrap();
                    assert!(picker(&world).is_some(), "{old:?} no longer cancels");
                }
                world.terminal.clear_writes();
                panel.handle_input("\x18").unwrap();
                world.runtime.settle().unwrap();
                assert!(editor_restored(&world), "the configured key cancels");
                assert!(!world.terminal.writes().is_empty(), "a frame is requested");
                assert!(editor_focused(&world), "the keyboard is back on the editor");
                assert_eq!(accepted(&world), expected.accepted);
                assert_eq!(count(&world).to_bits(), expected.count.to_bits());
                assert!(!notes(&world, "Logged out of") && !notes(&world, "Error: "));
                assert_eq!(
                    std::fs::read(&backing).unwrap(),
                    before,
                    "cancelling leaves the credential file byte for byte as it was"
                );
            }
        },
    );
}

#[test]
fn logout_callbacks_do_not_retain_the_mode() {
    isolated(
        "logout_callbacks_do_not_retain_the_mode",
        async |directory| {
            let world = started_with(directory, None).await;
            register_one(
                &world,
                &Provider {
                    id: "a".into(),
                    models: vec!["one".into()],
                    name: None,
                    key: true,
                    oauth_only: false,
                },
            );
            store_key(&world, "a");
            let panel = open(&world);
            let mode = Rc::downgrade(&world.mode);
            let World {
                mode: owned,
                runtime,
                ..
            } = world;
            assert!(mode.upgrade().is_some());
            panel.handle_input("\r").unwrap();
            assert_eq!(runtime.futures(), 1, "the admitted removal owns the mode");
            drop(owned);
            assert!(
                mode.upgrade().is_some(),
                "admitted work keeps the mode until it settles"
            );
            assert!(runtime.drain_futures().is_empty());
            assert!(
                mode.upgrade().is_none(),
                "the settled work releases the mode"
            );
            panel.handle_input("\r").unwrap();
            panel.handle_input("\x1b").unwrap();
            assert_eq!(runtime.futures(), 0, "the retained callbacks start nothing");
        },
    );
}
