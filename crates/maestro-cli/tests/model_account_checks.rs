//! The standalone account command: help, provider tables, selection, callbacks and the
//! account file, observed through its public entry.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use maestro_app::{
    BoxFuture, DiagnosticErrorInfo, Fetch, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface,
    presentation_data::{OAuthAuthInfo, OAuthPrompt, OAuthProviderHandle, get_oauth_providers},
};
use maestro_cli::model_login::cli::{self, AccountConsole, AccountInput};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::support;

/// Identifier of the controlled provider every case offers after the application ones.
const FIXTURE_ID: &str = "fixture";

/// Display name of that controlled provider.
const FIXTURE_NAME: &str = "Fixture account";

/// Authorization URL the controlled provider supplies.
const AUTH_URL: &str = "https://example.test/login";

/// Message of the authorization prompt the controlled provider asks.
const PROMPT_MESSAGE: &str = "Paste code";

/// One case's controlled host input.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CaseInput {
    /// Command arguments after the executable name.
    #[serde(default)]
    args: Vec<String>,
    /// Answers the case's input interfaces return, in order.
    #[serde(default)]
    answers: Vec<String>,
    /// `builtin` when the case offers the application registry alone.
    #[serde(default)]
    registry: Option<String>,
    /// Original account-file text the case seeds.
    #[serde(default)]
    seed: Option<String>,
    /// Whether the provider runs the authorization callbacks.
    #[serde(default)]
    callbacks: bool,
    /// Instructions the authorization supplies.
    #[serde(default)]
    instructions: Option<String>,
    /// Placeholder the authorization prompt supplies.
    #[serde(default)]
    placeholder: Option<String>,
    /// Credential members the provider returns instead of its own defaults.
    #[serde(default)]
    credentials: Option<Map<String, Value>>,
    /// Further providers the case offers after the fixture provider.
    #[serde(default)]
    extra_providers: Vec<ProviderSpec>,
    /// Expiry the provider returns instead of a finite one.
    #[serde(default)]
    nonfinite: Option<String>,
    /// Provider implementation change made while the numbered question is open.
    #[serde(default)]
    live: Option<String>,
    /// Failure the case injects: `denied` by the provider, or `input` from an interface.
    #[serde(default)]
    failure: Option<String>,
    /// Whether the provider keeps its callback handle after authorizing.
    #[serde(default)]
    retain: bool,
    /// Whether the case puts a directory at the account-file path.
    #[serde(default)]
    directory: bool,
}

/// One further provider a case offers after the fixture provider.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderSpec {
    /// Provider identifier.
    id: String,
    /// Provider display name.
    name: String,
}

/// One case's complete expected result.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Exit status the entry returns.
    exit: i32,
    /// Complete standard output.
    stdout: String,
    /// Complete standard error, absent where the case compares a native diagnostic instead.
    stderr: Option<String>,
    /// Interaction the host observed.
    trace: Trace,
    /// Account file after the run, absent when the case leaves none.
    auth_text: Option<String>,
}

/// Interaction one recorded run observed.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Trace {
    /// Input interfaces created.
    opened: usize,
    /// Released interface identifiers, in order.
    closed: Vec<usize>,
    /// Questions asked, in order.
    questions: Vec<String>,
    /// Authorizations served, by provider identifier.
    login: Vec<String>,
    /// Answers returned to authorization prompts, in order.
    answers: Vec<String>,
    /// Implementations that served an authorization, in order.
    resolved: Vec<String>,
    /// Interaction events, in order.
    events: Vec<String>,
    /// Whether the provider kept its callback handle, where the case says so.
    #[serde(default)]
    retained_callback: Option<bool>,
}

/// A provider implementation change made while the numbered question is open.
enum Live {
    /// Serve the identifier from this implementation.
    Replace(String, OAuthProviderHandle),
    /// Withdraw the identifier before login resolves it.
    Remove(String),
}

/// Everything one recorded run observed.
#[derive(Default)]
struct Recorder {
    /// Text written to the standard output.
    stdout: Mutex<String>,
    /// Text written to the standard error.
    stderr: Mutex<String>,
    /// Input interfaces created so far.
    opened: AtomicUsize,
    /// Released interface identifiers.
    closed: Mutex<Vec<usize>>,
    /// Questions asked.
    questions: Mutex<Vec<String>>,
    /// Answers still owed to input interfaces.
    answers: Mutex<VecDeque<String>>,
    /// Whether an input interface fails instead of answering.
    prompt_fail: bool,
    /// Authorizations served.
    login: Mutex<Vec<String>>,
    /// Implementations that served them.
    resolved: Mutex<Vec<String>>,
    /// Interaction events.
    events: Mutex<Vec<String>>,
    /// Answers returned to authorization prompts.
    supplied: Mutex<Vec<String>>,
    /// Callback handles a provider keeps.
    retained: Mutex<Vec<OAuthCallbacks>>,
    /// Pending provider change, applied at the next question.
    live: Mutex<Option<Live>>,
    /// Identifiers the live resolver was asked for, in order.
    lookups: Mutex<Vec<String>>,
}

impl Recorder {
    /// A recorder holding the supplied answers and the prompt failure choice.
    fn new(input: &CaseInput) -> Self {
        Self {
            answers: Mutex::new(input.answers.iter().cloned().collect()),
            prompt_fail: input.failure.as_deref() == Some("input"),
            ..Self::default()
        }
    }

    /// Record one write of the supplied text.
    fn write(&self, text: &str, is_error: bool) {
        let stream = if is_error { &self.stderr } else { &self.stdout };
        stream.lock().expect("stream lock").push_str(text);
    }

    /// Record a created input interface and return its identifier.
    fn open(&self) -> usize {
        let id = self.opened.fetch_add(1, Ordering::Relaxed) + 1;
        self.event(&format!("open:{id}"));
        id
    }

    /// Record a released input interface.
    fn close(&self, id: usize) {
        self.closed.lock().expect("closed lock").push(id);
        self.event(&format!("close:{id}"));
    }

    /// Ask one question of the recorded run, applying a pending provider change first.
    fn ask(&self, id: usize, question: String) -> Result<String, OAuthError> {
        self.event(&format!("question:{id}"));
        self.questions
            .lock()
            .expect("questions lock")
            .push(question);
        if self.prompt_fail {
            return Err(DiagnosticErrorInfo::new(None, "input failed").into());
        }
        self.answers
            .lock()
            .expect("answers lock")
            .pop_front()
            .ok_or_else(|| DiagnosticErrorInfo::new(None, "no recorded answer").into())
    }

    /// Record an authorization the supplied implementation served.
    fn record_login(&self, id: &str, label: &str) {
        self.event(&format!("login:{id}"));
        self.login.lock().expect("login lock").push(id.to_owned());
        self.resolved
            .lock()
            .expect("resolved lock")
            .push(label.to_owned());
    }

    /// Record the answer returned to an authorization prompt.
    fn record_answer(&self, answer: String) {
        self.supplied.lock().expect("supplied lock").push(answer);
    }

    /// Keep a callback handle beyond the authorization that supplied it.
    fn retain(&self, callbacks: OAuthCallbacks) {
        self.retained.lock().expect("retained lock").push(callbacks);
    }

    /// Record one interaction event.
    fn event(&self, event: &str) {
        self.events
            .lock()
            .expect("events lock")
            .push(event.to_owned());
    }

    /// The callback handles a provider kept beyond its authorization.
    fn retained_count(&self) -> usize {
        self.retained.lock().expect("retained lock").len()
    }

    /// Releases the callback handles a provider kept, which hold the console and so this
    /// recorder; the handles drop after the lock does.
    fn release_retained(&self) {
        let kept = std::mem::take(&mut *self.retained.lock().expect("retained lock"));
        drop(kept);
    }

    /// Record one identifier the live resolver was asked for.
    fn look_up(&self, id: &str) {
        self.lookups
            .lock()
            .expect("lookups lock")
            .push(id.to_owned());
    }

    /// The identifiers the live resolver was asked for, in order.
    fn lookups(&self) -> Vec<String> {
        self.lookups.lock().expect("lookups lock").clone()
    }

    /// The complete standard output written so far.
    fn stdout(&self) -> String {
        self.stdout.lock().expect("stdout lock").clone()
    }

    /// The complete standard error written so far.
    fn stderr(&self) -> String {
        self.stderr.lock().expect("stderr lock").clone()
    }

    /// The interaction this run observed, for the case's retained-callback choice.
    fn trace(&self, retained: bool) -> Trace {
        Trace {
            opened: self.opened.load(Ordering::Relaxed),
            closed: self.closed.lock().expect("closed lock").clone(),
            questions: self.questions.lock().expect("questions lock").clone(),
            login: self.login.lock().expect("login lock").clone(),
            answers: self.supplied.lock().expect("supplied lock").clone(),
            resolved: self.resolved.lock().expect("resolved lock").clone(),
            events: self.events.lock().expect("events lock").clone(),
            retained_callback: retained.then_some(true),
        }
    }
}

/// The console writing one recorded run's text into its recorder.
struct Console {
    /// Shared observations.
    recorder: Arc<Recorder>,
    /// Live implementations the case's question may change.
    live: Arc<Mutex<HashMap<String, OAuthProviderHandle>>>,
}

impl AccountConsole for Console {
    fn write(&self, text: &str, is_error: bool) {
        self.recorder.write(text, is_error);
    }

    fn create_interface(&self) -> Arc<dyn AccountInput> {
        Arc::new(Interface {
            id: self.recorder.open(),
            recorder: Arc::clone(&self.recorder),
            live: Arc::clone(&self.live),
        })
    }
}

/// One input interface of a recorded run.
struct Interface {
    /// Identifier in creation order from one.
    id: usize,
    /// Shared observations.
    recorder: Arc<Recorder>,
    /// Live implementations the case's question may change.
    live: Arc<Mutex<HashMap<String, OAuthProviderHandle>>>,
}

impl AccountInput for Interface {
    fn prompt(&self, question: String) -> BoxFuture<Result<String, OAuthError>> {
        apply_live(&self.recorder, &self.live);
        self.recorder.write(&question, false);
        let answer = self.recorder.ask(self.id, question);
        Box::pin(async move { answer })
    }

    fn close(&self) {
        self.recorder.close(self.id);
    }
}

/// Apply the provider change a case makes while its question is open.
fn apply_live(recorder: &Recorder, live: &Mutex<HashMap<String, OAuthProviderHandle>>) {
    let Some(change) = recorder.live.lock().expect("live lock").take() else {
        return;
    };
    let mut implementations = live.lock().expect("live map lock");
    match change {
        Live::Replace(id, provider) => {
            implementations.insert(id, provider);
        }
        Live::Remove(id) => {
            implementations.remove(&id);
        }
    }
}

/// One controlled provider that serves a recorded authorization.
struct TestProvider {
    /// Provider identifier.
    id: String,
    /// Provider display name.
    name: String,
    /// Implementation label recorded when it serves an authorization.
    label: &'static str,
    /// Credentials returned on success.
    credentials: OAuthCredentials,
    /// Whether the authorization runs its callbacks.
    callbacks: bool,
    /// Instructions the authorization supplies.
    instructions: Option<String>,
    /// Placeholder the authorization prompt supplies.
    placeholder: Option<String>,
    /// Whether the authorization fails.
    fail: bool,
    /// Whether the provider keeps its callback handle afterwards.
    retain: bool,
    /// Shared observations.
    recorder: Arc<Recorder>,
}

impl TestProvider {
    /// The labelled implementation one case offers under the fixture identity.
    fn fixture(recorder: &Arc<Recorder>, input: &CaseInput, label: &'static str) -> Self {
        Self {
            id: FIXTURE_ID.to_owned(),
            name: FIXTURE_NAME.to_owned(),
            label,
            credentials: credentials(input),
            callbacks: input.callbacks,
            instructions: input.instructions.clone(),
            placeholder: input.placeholder.clone(),
            fail: input.failure.as_deref() == Some("denied"),
            retain: input.retain,
            recorder: Arc::clone(recorder),
        }
    }

    /// The controlled implementation of one application registry identity.
    fn published(recorder: &Arc<Recorder>, provider: &OAuthProviderHandle) -> Self {
        Self {
            id: provider.id().to_owned(),
            name: provider.name().to_owned(),
            label: "initial",
            credentials: default_credentials(),
            callbacks: false,
            instructions: None,
            placeholder: None,
            fail: false,
            retain: false,
            recorder: Arc::clone(recorder),
        }
    }

    /// A further provider a case offers under its own identity.
    fn named(recorder: &Arc<Recorder>, spec: &ProviderSpec) -> Self {
        Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            label: "initial",
            credentials: default_credentials(),
            callbacks: false,
            instructions: None,
            placeholder: None,
            fail: false,
            retain: false,
            recorder: Arc::clone(recorder),
        }
    }
}

impl OAuthProviderInterface for TestProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn login(
        &self,
        callbacks: OAuthCallbacks,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        let id = self.id.clone();
        let label = self.label;
        let credentials = self.credentials.clone();
        let instructions = self.instructions.clone();
        let placeholder = self.placeholder.clone();
        let recorder = Arc::clone(&self.recorder);
        let (runs_callbacks, fail, retain) = (self.callbacks, self.fail, self.retain);
        Box::pin(async move {
            recorder.record_login(&id, label);
            if retain {
                recorder.retain(Arc::clone(&callbacks));
            }
            if runs_callbacks {
                callbacks.on_auth(OAuthAuthInfo {
                    url: AUTH_URL.to_owned(),
                    instructions,
                })?;
                callbacks.on_progress("Waiting for approval")?;
                let answer = callbacks
                    .on_prompt(OAuthPrompt {
                        message: PROMPT_MESSAGE.to_owned(),
                        placeholder,
                        allow_empty: None,
                    })
                    .await?;
                recorder.record_answer(answer);
            }
            if fail {
                return Err(DiagnosticErrorInfo::new(None, "denied by fixture").into());
            }
            Ok(credentials)
        })
    }

    fn refresh_token(
        &self,
        _credentials: OAuthCredentials,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Err(unused_operation()) })
    }

    fn get_api_key<'a>(
        &self,
        _credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        Err(unused_operation())
    }
}

/// A provider that counts the identity reads one table performs.
struct CountingProvider {
    /// Provider identifier.
    id: String,
    /// Provider display name.
    name: String,
    /// Identifier reads.
    id_reads: AtomicUsize,
    /// Display-name reads.
    name_reads: AtomicUsize,
}

impl CountingProvider {
    /// A provider whose identity and name are distinct for its position.
    fn new(position: usize) -> Self {
        Self {
            id: format!("provider-{position}"),
            name: format!("Provider {position}"),
            id_reads: AtomicUsize::new(0),
            name_reads: AtomicUsize::new(0),
        }
    }
}

impl OAuthProviderInterface for CountingProvider {
    fn id(&self) -> &str {
        self.id_reads.fetch_add(1, Ordering::Relaxed);
        &self.id
    }

    fn name(&self) -> &str {
        self.name_reads.fetch_add(1, Ordering::Relaxed);
        &self.name
    }

    fn login(
        &self,
        _callbacks: OAuthCallbacks,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Err(unused_operation()) })
    }

    fn refresh_token(
        &self,
        _credentials: OAuthCredentials,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Err(unused_operation()) })
    }

    fn get_api_key<'a>(
        &self,
        _credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        Err(unused_operation())
    }
}

/// The providers and host one case runs against.
struct World {
    /// Shared observations.
    recorder: Arc<Recorder>,
    /// Offered snapshot, in order.
    providers: Vec<OAuthProviderHandle>,
    /// Live implementations the resolver reads.
    live: Arc<Mutex<HashMap<String, OAuthProviderHandle>>>,
}

/// The controlled world one case input describes.
fn world(input: &CaseInput) -> World {
    let recorder = Arc::new(Recorder::new(input));
    let published = get_oauth_providers();
    let mut providers = if input.registry.as_deref() == Some("builtin") {
        published
    } else {
        published
            .iter()
            .map(|provider| handle(TestProvider::published(&recorder, provider)))
            .collect()
    };
    if input.registry.as_deref() != Some("builtin") {
        providers.push(handle(TestProvider::fixture(&recorder, input, "initial")));
        for spec in &input.extra_providers {
            providers.push(handle(TestProvider::named(&recorder, spec)));
        }
    }
    let live = providers
        .iter()
        .map(|provider| (provider.id().to_owned(), Arc::clone(provider)))
        .collect();
    let world = World {
        recorder,
        providers,
        live: Arc::new(Mutex::new(live)),
    };
    world.schedule_live(input);
    world
}

impl World {
    /// Schedule the provider change this case makes while its question is open.
    fn schedule_live(&self, input: &CaseInput) {
        let change = match input.live.as_deref() {
            Some("replace") => Some(Live::Replace(
                FIXTURE_ID.to_owned(),
                handle(TestProvider::fixture(&self.recorder, input, "replacement")),
            )),
            Some("remove") => Some(Live::Remove(FIXTURE_ID.to_owned())),
            _ => None,
        };
        *self.recorder.live.lock().expect("live lock") = change;
    }

    /// Run the public entry against this world and return its exit status.
    fn run(&self, args: &[String]) -> i32 {
        let live = Arc::clone(&self.live);
        let recorder = Arc::clone(&self.recorder);
        let resolver = move |id: &str| {
            recorder.look_up(id);
            live.lock().expect("live map lock").get(id).cloned()
        };
        let console: Arc<dyn AccountConsole> = Arc::new(Console {
            recorder: Arc::clone(&self.recorder),
            live: Arc::clone(&self.live),
        });
        let snapshot = self
            .providers
            .iter()
            .map(Arc::clone)
            .collect::<Vec<OAuthProviderHandle>>();
        crate::support::startup::runtime().block_on(cli::main(args, snapshot, &resolver, console))
    }
}

/// A handle for one controlled provider.
fn handle(provider: TestProvider) -> OAuthProviderHandle {
    Arc::new(provider)
}

/// The failure a provider raises for an operation the account command never calls.
fn unused_operation() -> OAuthError {
    DiagnosticErrorInfo::new(None, "unused provider operation").into()
}

/// The credentials a controlled provider returns, from the case's members or its defaults.
fn credentials(input: &CaseInput) -> OAuthCredentials {
    let mut credentials = input
        .credentials
        .as_ref()
        .map_or_else(default_credentials, |spec| {
            serde_json::from_value(Value::Object(spec.clone())).expect("credential members")
        });
    if let Some(nonfinite) = &input.nonfinite {
        credentials.expires = match nonfinite.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            _ => f64::NEG_INFINITY,
        };
    }
    credentials
}

/// The credentials a controlled provider returns when the case supplies none.
fn default_credentials() -> OAuthCredentials {
    OAuthCredentials {
        refresh: "refresh".to_owned(),
        access: "access".to_owned(),
        expires: 1_700_000_000_000.0,
        extra: Map::from_iter([("tenant".to_owned(), json!("team"))]),
    }
}

/// Put the case's original account file, or its directory, in place.
fn seed(input: &CaseInput) {
    if input.directory {
        std::fs::create_dir("auth.json").expect("account directory");
    }
    if let Some(text) = &input.seed {
        std::fs::write("auth.json", text).expect("seeded account file");
    }
}

/// The native diagnostic a write to a directory at the account-file path returns.
fn native_write_diagnostic() -> String {
    let error = std::fs::write("auth.json", b"").expect_err("a directory refuses the write");
    format!("Error: {error}\n")
}

/// The complete rows and help document one recorded provider table renders.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableExpected {
    /// Complete table rows, without the heading or a trailing newline.
    rows: String,
    /// Complete help document.
    help: String,
}

/// Run one recorded provider table through the list and help commands.
fn check_table(recorded: &support::Recorded) {
    let specs: Vec<ProviderSpec> =
        serde_json::from_value(recorded.input.clone()).expect("decodable providers");
    let expected: TableExpected =
        serde_json::from_value(recorded.expected.clone()).expect("decodable table");
    let name = recorded.case.as_str();
    let listed = spec_world(&specs);
    assert_eq!(listed.run(&["list".to_owned()]), 0, "{name}: list status");
    let expected_list = if expected.rows.is_empty() {
        "Available OAuth providers:\n\n".to_owned()
    } else {
        format!("Available OAuth providers:\n\n{}\n", expected.rows)
    };
    assert_eq!(
        listed.recorder.stdout(),
        expected_list,
        "{name}: list output"
    );
    let helped = spec_world(&specs);
    assert_eq!(helped.run(&["--help".to_owned()]), 0, "{name}: help status");
    assert_eq!(
        helped.recorder.stdout(),
        expected.help,
        "{name}: help output"
    );
}

/// A world offering the supplied provider records, in order, under a fresh recorder.
fn spec_world(specs: &[ProviderSpec]) -> World {
    let recorder = Arc::new(Recorder::new(&CaseInput::default()));
    let providers = specs
        .iter()
        .map(|spec| handle(TestProvider::named(&recorder, spec)))
        .collect();
    World {
        recorder,
        providers,
        live: Arc::new(Mutex::new(HashMap::new())),
    }
}

/// Run one recorded case and compare every observation.
fn check(name: &str) {
    let recorded = support::case_by_title("model_accounts", name);
    if recorded.operation == "table" {
        return check_table(&recorded);
    }
    let input: CaseInput = serde_json::from_value(recorded.input).expect("decodable input");
    let expected: Expected = serde_json::from_value(recorded.expected).expect("decodable result");
    seed(&input);
    let world = world(&input);
    let status = world.run(&input.args);
    assert_eq!(status, expected.exit, "{name}: exit status");
    assert_eq!(
        world.recorder.stdout(),
        expected.stdout,
        "{name}: standard output"
    );
    match &expected.stderr {
        Some(stderr) => assert_eq!(world.recorder.stderr(), *stderr, "{name}: standard error"),
        None => assert_eq!(
            world.recorder.stderr(),
            native_write_diagnostic(),
            "{name}: standard error"
        ),
    }
    assert_eq!(
        world.recorder.trace(input.retain),
        expected.trace,
        "{name}: recorded interaction"
    );
    assert_eq!(
        std::fs::read_to_string("auth.json").ok(),
        expected.auth_text,
        "{name}: account file"
    );
    if input.retain {
        assert_eq!(
            world.recorder.retained_count(),
            1,
            "{name}: kept callback handle"
        );
    }
    assert_released(world, name);
}

/// Releases what the run's providers kept and checks that nothing still holds its recorder.
fn assert_released(world: World, name: &str) {
    let recorder = Arc::clone(&world.recorder);
    drop(world);
    recorder.release_retained();
    assert_eq!(
        Arc::strong_count(&recorder),
        1,
        "{name}: a kept callback holds the recorder"
    );
}

/// Run one test's recorded cases, in a checked child that gives every case its own directory.
fn run_cases(test: &str, names: &[&str]) {
    in_child(test, || {
        for name in names {
            in_case_directory(|| check(name));
        }
    });
}

/// Run `body` in a fresh working directory, then return to the one it started in.
fn in_case_directory(body: impl FnOnce()) {
    let origin = std::env::current_dir().expect("origin directory");
    let scratch = maestro_test_directory::Scratch::new().expect("scratch directory");
    std::env::set_current_dir(scratch.path()).expect("case directory");
    body();
    std::env::set_current_dir(&origin).expect("restored directory");
}

/// Run `body` in a checked child of `test`, so no case writes to the test's own directory.
fn in_child(test: &str, body: impl FnOnce()) {
    let selection = maestro_test_process::selection();
    if selection.is_child() && selection.runs_case("rows") {
        body();
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().expect("scratch directory");
    let output = maestro_test_process::TestProcess::current(test)
        .expect("running test")
        .case("rows")
        .nocapture()
        .current_dir(scratch.path())
        .output()
        .expect("checked child");
    assert!(
        output.status.success(),
        "{test} failed its child: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Run the numbered login with no offered providers and one supplied number.
fn empty_snapshot(answer: &str) -> (i32, Arc<Recorder>) {
    let world = World {
        recorder: Arc::new(Recorder::new(&CaseInput {
            answers: vec![answer.to_owned()],
            ..CaseInput::default()
        })),
        providers: Vec::new(),
        live: Arc::new(Mutex::new(HashMap::new())),
    };
    let status = world.run(&["login".to_owned()]);
    (status, Arc::clone(&world.recorder))
}

#[test]
fn help_aliases_keep_command_identity() {
    run_cases(
        "help_aliases_keep_command_identity",
        &[
            "help_no_args",
            "help_empty",
            "help_word",
            "help_long",
            "help_short",
            "help_ignores_extra",
        ],
    );
}

#[test]
fn provider_tables_keep_snapshot_order_and_utf16_padding() {
    run_cases(
        "provider_tables_keep_snapshot_order_and_utf16_padding",
        &[
            "list_builtin",
            "list_registered",
            "list_ignores_extra",
            "empty",
            "spaced_identifier",
            "column_widths",
        ],
    );
}

#[test]
fn unknown_commands_keep_help_hint() {
    run_cases(
        "unknown_commands_keep_help_hint",
        &["unknown_command", "unknown_case"],
    );
}

#[test]
fn explicit_login_ignores_extra_arguments() {
    run_cases(
        "explicit_login_ignores_extra_arguments",
        &["explicit_login"],
    );
}

#[test]
fn numbered_login_closes_selection_before_login() {
    run_cases(
        "numbered_login_closes_selection_before_login",
        &[
            "selection_first",
            "selection_last",
            "selection_signed",
            "selection_leading_zero",
            "selection_whitespace",
            "selection_bom",
            "selection_trailing_bom",
            "selection_long_zeros",
        ],
    );
}

#[test]
fn invalid_selections_never_start_login() {
    run_cases(
        "invalid_selections_never_start_login",
        &[
            "selection_zero",
            "selection_past_last",
            "selection_negative",
            "selection_blank",
            "selection_whitespace_only",
            "selection_nonnumeric",
            "selection_trailing",
            "selection_decimal",
            "selection_exponent",
            "selection_hex",
            "selection_nel",
            "selection_large",
            "selection_minus_zero",
            "selection_sign_only",
            "selection_internal_space",
            "selection_embedded_bom",
            "selection_unicode_digit",
            "selection_infinity",
            "selection_existing_file",
        ],
    );
    let (status, recorder) = empty_snapshot("1");
    assert_eq!(status, 1, "empty snapshot: exit status");
    assert_eq!(
        recorder.stdout(),
        "Select a provider:\n\n\nEnter number (1-0): ",
        "empty snapshot: standard output"
    );
    assert_eq!(
        recorder.stderr(),
        "Invalid selection\n",
        "empty snapshot: standard error"
    );
    assert_eq!(
        recorder.trace(false),
        Trace {
            opened: 1,
            closed: vec![1],
            questions: vec!["Enter number (1-0): ".to_owned()],
            login: Vec::new(),
            answers: Vec::new(),
            resolved: Vec::new(),
            events: vec![
                "open:1".to_owned(),
                "question:1".to_owned(),
                "close:1".to_owned(),
            ],
            retained_callback: None,
        },
        "empty snapshot: recorded interaction"
    );
}

/// The snapshot offers `known`; the resolver also holds `live-only`, which the snapshot does
/// not. The input interface answers `answers`.
fn lookup_world(answers: &[&str]) -> World {
    let recorder = Arc::new(Recorder::new(&CaseInput {
        answers: answers.iter().map(|answer| (*answer).to_owned()).collect(),
        ..CaseInput::default()
    }));
    let provider = |id: &str, name: &str| {
        handle(TestProvider::named(
            &recorder,
            &ProviderSpec {
                id: id.to_owned(),
                name: name.to_owned(),
            },
        ))
    };
    let known = provider("known", "Known");
    let live = HashMap::from([
        ("known".to_owned(), Arc::clone(&known)),
        ("live-only".to_owned(), provider("live-only", "Live Only")),
    ]);
    World {
        recorder: Arc::clone(&recorder),
        providers: vec![known],
        live: Arc::new(Mutex::new(live)),
    }
}

#[test]
fn resolver_is_asked_only_after_the_snapshot_accepts_the_identifier() {
    in_child(
        "resolver_is_asked_only_after_the_snapshot_accepts_the_identifier",
        || {
            // Every path that ends before an accepted identifier asks the resolver nothing,
            // including a login for an identifier only the resolver holds.
            for (args, answers, status, error) in [
                (&["--help"][..], &[][..], 0, ""),
                (&["list"], &[], 0, ""),
                (&["nonsense"], &[], 1, "Unknown command: nonsense\n"),
                (
                    &["login", "live-only"],
                    &[],
                    1,
                    "Unknown provider: live-only\n",
                ),
                (&["login", "absent"], &[], 1, "Unknown provider: absent\n"),
                (&["login"], &["99"], 1, "Invalid selection\n"),
            ] {
                in_case_directory(|| {
                    let world = lookup_world(answers);
                    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
                    assert_eq!(world.run(&args), status, "{args:?}: exit status");
                    assert!(world.recorder.stderr().starts_with(error), "{args:?}");
                    assert_eq!(world.recorder.lookups(), Vec::<String>::new(), "{args:?}");
                });
            }
            // An accepted identifier is looked up once, by the identifier the snapshot accepted.
            in_case_directory(|| {
                let world = lookup_world(&[]);
                let args = ["login".to_owned(), "known".to_owned()];
                assert_eq!(world.run(&args), 0, "login: exit status");
                assert_eq!(world.recorder.lookups(), ["known"], "login: lookups");
            });
        },
    );
}

#[test]
fn live_lookup_observes_replaced_provider() {
    run_cases("live_lookup_observes_replaced_provider", &["live_replaced"]);
}

#[test]
fn live_lookup_reports_removed_provider_without_hint() {
    run_cases(
        "live_lookup_reports_removed_provider_without_hint",
        &["live_removed"],
    );
}

#[test]
fn snapshot_validation_rejects_unknown_provider() {
    run_cases(
        "snapshot_validation_rejects_unknown_provider",
        &["unknown_provider"],
    );
}

#[test]
fn auth_callbacks_keep_blank_lines_and_prompt_answers() {
    run_cases(
        "auth_callbacks_keep_blank_lines_and_prompt_answers",
        &[
            "callbacks_absent",
            "callbacks_empty",
            "callbacks_present",
            "callbacks_whitespace",
            "callbacks_empty_answer",
        ],
    );
}

#[test]
fn provider_failure_closes_input_and_keeps_file() {
    run_cases(
        "provider_failure_closes_input_and_keeps_file",
        &["login_failure", "callback_then_failure"],
    );
}

#[test]
fn account_write_preserves_unrelated_provider_and_order() {
    run_cases(
        "account_write_preserves_unrelated_provider_and_order",
        &[
            "auth_missing",
            "auth_malformed",
            "auth_object",
            "auth_indexed_members",
        ],
    );
}

#[test]
fn non_object_account_roots_are_not_overwritten() {
    run_cases(
        "non_object_account_roots_are_not_overwritten",
        &[
            "auth_array",
            "auth_null",
            "auth_string",
            "auth_number",
            "auth_boolean",
        ],
    );
}

#[test]
fn account_read_and_save_errors_keep_outer_diagnostic() {
    run_cases(
        "account_read_and_save_errors_keep_outer_diagnostic",
        &["auth_directory"],
    );
}

#[test]
fn credential_fields_keep_tag_precedence_and_extra_values() {
    run_cases(
        "credential_fields_keep_tag_precedence_and_extra_values",
        &[
            "credential_type_null",
            "credential_type_string",
            "credential_type_number",
            "credential_type_boolean",
            "credential_type_object",
            "credential_type_array",
            "credential_expiry_NaN",
            "credential_expiry_Infinity",
            "credential_expiry_-Infinity",
        ],
    );
}

#[test]
fn input_failures_release_owned_interface() {
    run_cases(
        "input_failures_release_owned_interface",
        &["callback_input_failure"],
    );
}

#[test]
fn retained_callbacks_close_when_login_finishes() {
    run_cases(
        "retained_callbacks_close_when_login_finishes",
        &["retained_callbacks"],
    );
    let input = CaseInput {
        args: vec!["login".to_owned(), FIXTURE_ID.to_owned()],
        retain: true,
        failure: Some("denied".to_owned()),
        ..CaseInput::default()
    };
    let failure = world(&input);
    assert_eq!(failure.run(&input.args), 1, "retained failure: exit status");
    assert_eq!(
        failure.recorder.stderr(),
        "Error: denied by fixture\n",
        "retained failure: standard error"
    );
    assert_eq!(
        failure.recorder.retained_count(),
        1,
        "retained failure: kept callback handle"
    );
    assert_eq!(
        failure.recorder.trace(true).closed,
        vec![1],
        "retained failure: released interface"
    );
    assert_released(failure, "retained failure");
}

#[test]
fn provider_table_work_is_linear() {
    for count in [128, 256, 512] {
        let mut handles: Vec<OAuthProviderHandle> = Vec::new();
        let mut counters: Vec<Arc<CountingProvider>> = Vec::new();
        for position in 0..count {
            let provider = Arc::new(CountingProvider::new(position));
            counters.push(Arc::clone(&provider));
            handles.push(provider);
        }
        let world = World {
            recorder: Arc::new(Recorder::new(&CaseInput::default())),
            providers: handles,
            live: Arc::new(Mutex::new(HashMap::new())),
        };
        assert_eq!(world.run(&["list".to_owned()]), 0, "{count}: list status");
        let id_reads: usize = counters
            .iter()
            .map(|counter| counter.id_reads.load(Ordering::Relaxed))
            .sum();
        let name_reads: usize = counters
            .iter()
            .map(|counter| counter.name_reads.load(Ordering::Relaxed))
            .sum();
        // Each provider is read a bounded number of times, whether the table reads a field
        // once or twice per row.
        assert!(
            id_reads <= 2 * count,
            "{count}: identifier reads {id_reads}"
        );
        assert!(
            name_reads <= 2 * count,
            "{count}: display-name reads {name_reads}"
        );
        assert_eq!(
            world.recorder.stdout().lines().count(),
            count + 2,
            "{count}: rendered rows"
        );
    }
}

#[test]
fn literal_provider_ids_remain_account_members() {
    run_cases(
        "literal_provider_ids_remain_account_members",
        &[
            "literal_provider___proto__",
            "literal_provider_constructor",
            "literal_provider_2",
        ],
    );
}

#[test]
fn empty_provider_argument_starts_numbered_selection() {
    run_cases(
        "empty_provider_argument_starts_numbered_selection",
        &["empty_provider_arg"],
    );
}
