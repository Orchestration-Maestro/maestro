//! The prompt loop over a real session answering through the scripted provider: startup
//! notices and prompts, refused prompts, typed prompts and the current session.
use super::{Setup, World, cases, isolated};
use crate::{
    AssistantMessageComponent, InteractiveMode, InteractiveModeOptions, ToolExecutionComponent,
    UserMessageComponent,
};
use maestro_app::{
    AgentMessage, AgentSession, AgentSessionEvent, ExtensionFactory, ExtensionHandler,
    ImageContent, InputEvent, InputEventResult, Local, Presence, PromptOptions,
    ProviderConfigInput, StopReason, UserBlock, UserContent,
};
use maestro_models::{
    ApiProvider, AssistantMessageEventStream, DiagnosticErrorInfo, FauxAssistantMessageOptions,
    FauxProviderRegistration, FauxProviderState, FauxResponseStep, Invoke, ProviderCall,
    ProviderExecution, RegisterFauxProviderOptions, StreamOptionsInput, faux_assistant_message,
    get_api_provider, register_faux_provider,
};
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::{Component, Container, Spacer, Text};
use serde::Deserialize;
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::BTreeMap,
    error::Error,
    future::{Future, poll_fn},
    path::Path,
    pin::{Pin, pin},
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
};

/// Every recorded prompt-loop case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_prompts.json");

/// The width transcript notices are rendered at.
const WIDTH: usize = 120;

/// Polls `run` until the mode waits for typed input; the loop never ends on its own.
pub(super) async fn until_waiting<F: Future<Output = Result<(), Box<dyn Error>>>>(
    mode: &InteractiveMode,
    mut run: Pin<&mut F>,
) {
    poll_fn(|context| {
        if let Poll::Ready(result) = run.as_mut().poll(context) {
            panic!("the prompt loop ended: {result:?}");
        }
        if mode.on_input_callback.borrow().is_some() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
}

/// A mode whose sessions answer through the scripted provider without retrying.
pub(super) struct Scripted {
    /// The mode and its runtime.
    pub(super) world: World,
    /// The scripted provider.
    pub(super) provider: FauxProviderRegistration,
    /// Makes the next credential lookup find no key, so the session refuses that prompt.
    refuse: Arc<AtomicBool>,
    /// What the application input hook saw of each prompt.
    hook: HookLog,
}

/// The text and image presence the application input hook saw of each prompt, in order.
type HookLog = Rc<RefCell<Vec<(String, Presence<Vec<ImageContent>>)>>>;

/// A factory whose extension records every prompt the application hands on.
fn input_hook(seen: &HookLog) -> ExtensionFactory<Local> {
    let seen = Rc::clone(seen);
    Rc::new(move |api| {
        let seen = Rc::clone(&seen);
        let handler: ExtensionHandler<InputEvent, InputEventResult, Local> =
            Rc::new(move |event, _| {
                seen.borrow_mut()
                    .push((event.text.clone(), event.images.clone()));
                Box::pin(async { Ok(None) })
            });
        api.on::<InputEvent, InputEventResult>("input".into(), handler)
            .unwrap();
        Box::pin(async { Ok(()) })
    })
}

/// Keeps the scripted callback available when the model catalog refreshes its registrations.
struct RetainedProvider(ApiProvider);

impl<E: ProviderExecution> Invoke<E, ProviderCall<E, StreamOptionsInput<E>>> for RetainedProvider {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;

    fn invoke(&self, input: ProviderCall<E, StreamOptionsInput<E>>) -> Self::Return {
        match input.options {
            Some(StreamOptionsInput::Raw(options)) => self.0.stream.call(
                input.model,
                input.context,
                E::submission(Some(options), input.submit),
            ),
            Some(StreamOptionsInput::Simple(options)) => self.0.stream_simple.call(
                input.model,
                input.context,
                E::submission(Some(options), input.submit),
            ),
            None => self.0.stream_simple.call(
                input.model,
                input.context,
                E::submission(None, input.submit),
            ),
        }
    }
}

impl Scripted {
    /// Builds a mode with `options` in `directory`; its key comes from the fallback resolver.
    pub(super) fn new(directory: &Path, options: InteractiveModeOptions) -> Self {
        let hook: HookLog = Rc::default();
        let world = Setup {
            options,
            extension: Some(input_hook(&hook)),
            ..Setup::default()
        }
        .build(directory);
        let provider = register_faux_provider(RegisterFauxProviderOptions::default());
        let callback = get_api_provider(&provider.api).unwrap();
        world
            .host
            .session()
            .model_registry()
            .register_provider(
                &provider.models[0].provider,
                Arc::new(RwLock::new(ProviderConfigInput {
                    api: Some(provider.api.clone()),
                    callbacks: maestro_app::ProviderCallbacks {
                        stream_simple: Some(Arc::new(RetainedProvider(callback))),
                        ..Default::default()
                    },
                    ..ProviderConfigInput::default()
                })),
            )
            .unwrap();
        let refuse = Arc::new(AtomicBool::new(false));
        let refused = Arc::clone(&refuse);
        let provider_id = provider.models[0].provider.clone();
        world
            .host
            .services()
            .auth_storage
            .set_fallback_resolver(move |id| {
                Ok(
                    (id == provider_id && !refused.swap(false, Ordering::SeqCst))
                        .then(|| "controlled-key".to_owned()),
                )
            });
        let scripted = Self {
            world,
            provider,
            refuse,
            hook,
        };
        scripted.adopt(&scripted.world.host.session());
        scripted
    }

    /// Makes `session` answer through the provider without retrying.
    pub(super) fn adopt(&self, session: &AgentSession) {
        session.set_auto_retry_enabled(false);
        session.state().write().unwrap().model =
            Arc::new(RwLock::new(self.provider.models[0].clone()));
    }

    /// The message of a prompt the current session refuses for want of a key.
    async fn refusal(&self) -> String {
        self.refuse.store(true, Ordering::SeqCst);
        let session = self.world.host.session();
        let refused = session.prompt("probe", PromptOptions::default()).await;
        assert_eq!(self.provider.state.call_count(), 0);
        refused.unwrap_err().message
    }

    /// Unregisters the provider and disposes of the current session.
    pub(super) fn finish(&self) {
        self.world.host.session().dispose();
        self.provider.unregister();
    }
}

/// An answer that stops normally, or fails with `error`.
fn answer(error: Option<&str>) -> FauxResponseStep {
    let options = FauxAssistantMessageOptions {
        stop_reason: error.map(|_| StopReason::Error),
        error_message: error.map(str::to_owned),
        ..FauxAssistantMessageOptions::default()
    };
    FauxResponseStep::Message(Box::new(faux_assistant_message("ok", options)))
}

/// The session a prompt reached.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Session {
    /// The session the mode started with.
    Old,
    /// The session that replaced it.
    New,
}

/// One step of the loop, in order.
#[derive(Debug, PartialEq)]
enum Observed {
    /// A transcript notice's rows.
    Notice(Vec<String>),
    /// A prompt a session admitted, with the image blocks of its message.
    Prompt {
        /// The prompt text.
        text: String,
        /// The image blocks.
        images: Vec<ImageContent>,
        /// The session that admitted it.
        session: Session,
    },
}

/// Records notices and each provider admission in the order they happen.
struct Recorder {
    /// The transcript notices are appended to.
    chat: Rc<Container>,
    /// The provider's admissions.
    calls: FauxProviderState,
    /// Refuses the prompt that follows `refuse_after` admissions.
    refuse: Arc<AtomicBool>,
    /// The admissions before the refused prompt, if any.
    refuse_after: Option<usize>,
    /// The transcript children already recorded.
    recorded: Cell<usize>,
    /// The admissions already recorded.
    admitted: Cell<usize>,
    /// The notices and the session of each admission.
    log: RefCell<Vec<Result<Session, Vec<String>>>>,
    /// Removes each session subscription.
    removals: RefCell<Vec<Box<dyn Fn()>>>,
}

impl Recorder {
    /// Records `scripted`, refusing the prompt that follows `refuse_after` admissions.
    fn new(scripted: &Scripted, refuse_after: Option<usize>) -> Rc<Self> {
        scripted
            .refuse
            .store(refuse_after == Some(0), Ordering::SeqCst);
        Rc::new(Self {
            chat: Rc::clone(&scripted.world.mode.chat_container),
            calls: scripted.provider.state.clone(),
            refuse: Arc::clone(&scripted.refuse),
            refuse_after,
            recorded: Cell::new(0),
            admitted: Cell::new(0),
            log: RefCell::default(),
            removals: RefCell::default(),
        })
    }

    /// Records the admissions `session` makes as `tag`.
    fn listen(self: &Rc<Self>, session: &AgentSession, tag: Session) {
        let recorder = Rc::clone(self);
        let removal = session.subscribe(Rc::new(move |event| {
            if matches!(event, AgentSessionEvent::Agent(_)) {
                recorder.observe(tag);
            }
        }));
        self.removals.borrow_mut().push(Box::new(removal));
    }

    /// Ends every subscription.
    fn stop(&self) {
        for removal in self.removals.take() {
            removal();
        }
    }

    /// Records the notices before a new admission, then the admission.
    fn observe(&self, tag: Session) {
        let calls = self.calls.call_count();
        if calls == self.admitted.get() {
            return;
        }
        self.flush();
        self.log.borrow_mut().push(Ok(tag));
        self.admitted.set(calls);
        if self.refuse_after == Some(calls) {
            self.refuse.store(true, Ordering::SeqCst);
        }
    }

    /// Records the rows of each notice added since the last record, skipping the
    /// resource diagnostics the startup listing appends as a text row and a spacer, and
    /// the message rows the session's events add. An error notice's own blank row, which a
    /// following spacer or the end shows, belongs to its notice.
    fn flush(&self) {
        let children = self.chat.children();
        let children = children.borrow();
        let added = without_messages(&children[self.recorded.get()..]);
        let blank = |index: usize| {
            added
                .get(index)
                .is_some_and(|row| row.render(WIDTH).iter().all(String::is_empty))
        };
        let mut index = 0;
        while index + 1 < added.len() {
            if !blank(index) {
                index += 2;
                continue;
            }
            let mut rows = added[index + 1].render(WIDTH);
            if blank(index + 2) && (index + 3 == added.len() || blank(index + 3)) {
                rows.push(String::new());
                index += 1;
            }
            self.log.borrow_mut().push(Err(rows));
            index += 2;
        }
        self.recorded.set(children.len());
    }

    /// The log with each admission replaced by the next prompt its session holds.
    fn observed(&self, sessions: &[(Session, Rc<AgentSession>)]) -> Vec<Observed> {
        let mut prompts: Vec<_> = sessions
            .iter()
            .map(|(tag, session)| (*tag, admitted(session).into_iter()))
            .collect();
        let log = self.log.borrow();
        let observed = log.iter().map(|entry| match entry {
            Err(rows) => Observed::Notice(rows.clone()),
            Ok(tag) => {
                let held = prompts.iter_mut().find(|(held, _)| held == tag).unwrap();
                let (text, images) = held.1.next().unwrap();
                Observed::Prompt {
                    text,
                    images,
                    session: *tag,
                }
            }
        });
        let observed = observed.collect();
        assert!(prompts.iter_mut().all(|(_, rest)| rest.next().is_none()));
        observed
    }
}

/// `rows` without the message rows the session's events add: user, assistant and tool rows,
/// and the spacer before a user row.
fn without_messages(rows: &[Rc<dyn Component>]) -> Vec<&Rc<dyn Component>> {
    let kind = |index: usize| rows.get(index).map(|row| -> &dyn Any { row.as_ref() });
    let user = |index| kind(index).is_some_and(<dyn Any>::is::<UserMessageComponent>);
    let kept = (0..rows.len()).filter(|&index| {
        let Some(row) = kind(index) else {
            return false;
        };
        let message = user(index)
            || row.is::<AssistantMessageComponent>()
            || row.is::<ToolExecutionComponent>();
        !(message || row.is::<Spacer>() && user(index + 1))
    });
    kept.map(|index| &rows[index]).collect()
}

/// The text and image blocks of each user message `session` holds, in order.
fn admitted(session: &AgentSession) -> Vec<(String, Vec<ImageContent>)> {
    let messages = session.messages();
    let messages = messages.read().unwrap();
    let users = messages.iter().filter_map(|message| match message {
        AgentMessage::User(user) => Some(Arc::clone(user)),
        _ => None,
    });
    users
        .map(|user| {
            let user = user.read().unwrap();
            let UserContent::Blocks(blocks) = &user.content else {
                panic!("a user message of blocks");
            };
            let [UserBlock::Text(text), images @ ..] = blocks.as_slice() else {
                panic!("a leading text block");
            };
            let images = images.iter().map(|block| match block {
                UserBlock::Image(image) => image.clone(),
                UserBlock::Text(_) => panic!("one text block"),
            });
            (text.text.clone(), images.collect())
        })
        .collect()
}

/// The failure text of each failed answer `session` holds, in order.
fn assistant_errors(session: &AgentSession) -> Vec<String> {
    let messages = session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::Assistant(answer) => {
                let answer = answer.read().unwrap();
                (answer.stop_reason == StopReason::Error).then(|| answer.error_message.clone())
            }
            _ => None,
        })
        .map(Option::unwrap)
        .collect()
}

/// The recorded startup options.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// The migrated providers.
    #[serde(default)]
    migrated_providers: Vec<String>,
    /// The model restoration warning.
    model_fallback_message: Option<String>,
    /// The first prompt.
    initial_message: Option<String>,
    /// The first prompt's images.
    initial_images: Option<Vec<ImageContent>>,
    /// The following prompts.
    #[serde(default)]
    initial_messages: Vec<String>,
}

/// How a prompt fails.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Failure {
    /// The session refuses the prompt operation.
    Error,
    /// The provider answers with a failure.
    Stream,
}

/// One recorded run of the loop.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Startup {
    /// The mode options.
    options: Options,
    /// The text of the catalog file; empty when there is none.
    #[serde(default)]
    catalog_error: String,
    /// The prompts typed once the startup prompts are sent.
    typed: Vec<String>,
    /// How each failing prompt fails, by its text.
    #[serde(default)]
    failures: BTreeMap<String, Failure>,
    /// The typed prompt before which the session is replaced.
    replace_before_typed: Option<usize>,
}

impl Startup {
    /// Every prompt the mode sends, in order, with the image presence the input hook sees.
    fn sent_images(&self) -> Vec<(String, Presence<Vec<ImageContent>>)> {
        let first = self
            .options
            .initial_message
            .iter()
            .filter(|text| !text.is_empty())
            .map(|text| {
                let images = self
                    .options
                    .initial_images
                    .clone()
                    .map_or(Presence::Missing, Presence::Present);
                (text.clone(), images)
            });
        let rest = self
            .options
            .initial_messages
            .iter()
            .chain(&self.typed)
            .map(|text| (text.clone(), Presence::Missing));
        first.chain(rest).collect()
    }

    /// The admissions before the refused prompt, if one is refused.
    fn refuse_after(&self) -> Option<usize> {
        let first = self
            .options
            .initial_message
            .iter()
            .filter(|text| !text.is_empty());
        let mut order = first
            .chain(&self.options.initial_messages)
            .chain(&self.typed);
        let (refused, _) = self
            .failures
            .iter()
            .find(|(_, failure)| **failure == Failure::Error)?;
        order.position(|text| text == refused)
    }
}

/// One recorded step of the loop.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Step {
    /// A prompt a session admitted.
    Prompt {
        /// The prompt text.
        prompt: String,
        /// The images sent with it.
        images: Option<Vec<ImageContent>>,
        /// The session that admitted it.
        session: Session,
    },
    /// A warning notice.
    Warning {
        /// Its text.
        warning: String,
    },
    /// An error notice.
    Error {
        /// Its text.
        error: String,
    },
    /// The catalog error notice for a catalog file holding this text.
    CatalogError {
        /// The catalog file's text.
        #[serde(rename = "catalogError")]
        catalog_error: String,
    },
    /// The error notice of the prompt the session refused.
    Refused {
        /// The refused prompt.
        refused: String,
    },
    /// A failed answer, which the session keeps and the mode shows no notice for.
    AssistantError {
        /// The failure text.
        #[serde(rename = "assistantError")]
        assistant_error: String,
    },
}

/// What the recorded steps look like through the transcript and the sessions.
struct Expectation<'a> {
    /// The replayed run.
    startup: &'a Startup,
    /// The published theme.
    theme: &'a Theme,
    /// The message of a refused prompt.
    refusal: &'a str,
    /// The current catalog error.
    catalog: Option<String>,
}

impl Expectation<'_> {
    /// The rows of a notice shown in `color`; an error notice ends with a blank row.
    fn notice(&self, color: &ThemeColor, text: &str) -> Observed {
        let painted = self.theme.fg(color, text).unwrap();
        let mut rows = Text::new(painted, 1, 0, None).render(WIDTH);
        if *color == ThemeColor::Error {
            rows.push(String::new());
        }
        Observed::Notice(rows)
    }

    /// The observation of `step`; none for a failed answer.
    fn observed(&self, step: &Step) -> Option<Observed> {
        Some(match step {
            Step::Prompt {
                prompt,
                images,
                session,
            } => Observed::Prompt {
                text: prompt.clone(),
                // The session admits no image block for absent or empty images alike.
                images: images.clone().unwrap_or_default(),
                session: *session,
            },
            Step::Warning { warning } => {
                self.notice(&ThemeColor::Warning, &format!("Warning: {warning}"))
            }
            Step::Error { error } => self.notice(&ThemeColor::Error, &format!("Error: {error}")),
            Step::CatalogError { catalog_error } => {
                assert_eq!(*catalog_error, self.startup.catalog_error);
                let catalog = self.catalog.as_deref().unwrap();
                assert!(catalog.starts_with("Failed to parse models.json"));
                let text = format!("Error: models.json error: {catalog}");
                self.notice(&ThemeColor::Error, &text)
            }
            Step::Refused { refused } => {
                let failure = self.startup.failures.get(refused);
                assert!(failure == Some(&Failure::Error), "{refused} is refused");
                self.notice(&ThemeColor::Error, &format!("Error: {}", self.refusal))
            }
            Step::AssistantError { .. } => return None,
        })
    }
}

/// Scripts one answer per admitted prompt, failing those a failed answer follows.
fn script(provider: &FauxProviderRegistration, steps: &[Step]) {
    let answers = steps.iter().enumerate().filter_map(|(index, step)| {
        let Step::Prompt { .. } = step else {
            return None;
        };
        Some(answer(match steps.get(index + 1) {
            Some(Step::AssistantError { assistant_error }) => Some(assistant_error.as_str()),
            _ => None,
        }))
    });
    provider.set_responses(answers.collect());
}

/// The default of an options record the application does not export by name.
fn defaulted<T: Default>() -> T {
    T::default()
}

/// Runs the loop through `startup` until it waits after the last typed prompt, replacing the
/// session where recorded; returns each session the prompts reached.
async fn drive(
    scripted: &Scripted,
    recorder: &Rc<Recorder>,
    startup: &Startup,
) -> Vec<(Session, Rc<AgentSession>)> {
    let host = &scripted.world.host;
    let mut sessions = vec![(Session::Old, host.session())];
    recorder.listen(&sessions[0].1, Session::Old);
    let mode = &scripted.world.mode;
    let mut run = pin!(mode.run());
    until_waiting(mode, run.as_mut()).await;
    for (index, text) in startup.typed.iter().enumerate() {
        if startup.replace_before_typed == Some(index) {
            let replaced = host.new_session(defaulted()).await.unwrap();
            assert!(!replaced.cancelled);
            sessions.push((Session::New, host.session()));
            scripted.adopt(&sessions[1].1);
            recorder.listen(&sessions[1].1, Session::New);
        }
        scripted.world.terminal.send_input(text);
        scripted.world.terminal.send_input("\r");
        until_waiting(mode, run.as_mut()).await;
    }
    recorder.flush();
    recorder.stop();
    sessions
}

/// Builds the mode of `startup` in `directory`, writing its catalog file while the catalog
/// loads.
fn build(directory: &Path, startup: &mut Startup) -> Scripted {
    let catalog = directory.join("models.json");
    let written = !startup.catalog_error.is_empty();
    if written {
        std::fs::write(&catalog, &startup.catalog_error).unwrap();
    }
    let options = &mut startup.options;
    Scripted::new(
        directory,
        InteractiveModeOptions {
            migrated_providers: std::mem::take(&mut options.migrated_providers),
            model_fallback_message: options.model_fallback_message.take(),
            initial_message: options.initial_message.take(),
            initial_images: options.initial_images.take(),
            initial_messages: std::mem::take(&mut options.initial_messages),
            ..InteractiveModeOptions::default()
        },
    )
}

/// Runs the loop through `startup`, then compares every notice, admission and failed answer
/// with `expected`.
async fn replay_case(directory: &Path, mut startup: Startup, expected: Vec<Step>) {
    let refuse_after = startup.refuse_after();
    let images = startup.sent_images();
    let scripted = build(directory, &mut startup);
    let refusal = scripted.refusal().await;
    // The refusal probe is not part of the recorded run.
    scripted.hook.borrow_mut().clear();
    script(&scripted.provider, &expected);
    let recorder = Recorder::new(&scripted, refuse_after);
    let sessions = drive(&scripted, &recorder, &startup).await;
    let theme = scripted.world.themes.theme().get().unwrap();
    let expectation = Expectation {
        startup: &startup,
        theme: &theme,
        refusal: &refusal,
        catalog: sessions[0].1.model_registry().get_error(),
    };
    let wanted: Vec<_> = expected
        .iter()
        .filter_map(|step| expectation.observed(step))
        .collect();
    assert_eq!(recorder.observed(&sessions), wanted);
    assert_eq!(*scripted.hook.borrow(), images);
    let admissions = wanted
        .iter()
        .filter(|step| matches!(step, Observed::Prompt { .. }));
    assert_eq!(scripted.provider.state.call_count(), admissions.count());
    let failed = expected.iter().filter_map(|step| match step {
        Step::AssistantError { assistant_error } => Some(assistant_error.clone()),
        _ => None,
    });
    assert_eq!(assistant_errors(&sessions[0].1), failed.collect::<Vec<_>>());
    scripted.finish();
}

/// Replays every recorded case of `test` in a fresh child.
fn replay(test: &str) {
    isolated(test, async |directory| {
        for (startup, expected) in cases::<Startup, Vec<Step>>(FIXTURES, test) {
            let case = maestro_test_directory::Scratch::new_in(directory).unwrap();
            (replay_case(case.path(), startup, expected)).await;
        }
    });
}

#[test]
fn startup_prompts_keep_order_images_empty_messages_and_error_recovery() {
    replay("startup_prompts_keep_order_images_empty_messages_and_error_recovery");
}

#[test]
fn startup_empty_options_skip_notices_and_orphan_images() {
    replay("startup_empty_options_skip_notices_and_orphan_images");
}

#[test]
fn startup_prompt_failures_do_not_stop_following_input() {
    replay("startup_prompt_failures_do_not_stop_following_input");
}

#[test]
fn typed_submission_follows_the_current_runtime_session() {
    replay("typed_submission_follows_the_current_runtime_session");
}

#[test]
fn prompt_operation_refusal_is_distinct_from_assistant_error_event() {
    const TEST: &str = "prompt_operation_refusal_is_distinct_from_assistant_error_event";
    isolated(TEST, async |directory| {
        (refusal_then_assistant_error(directory)).await;
    });
}

/// Refuses a prompt for want of a key, then sends one the provider answers with a failure
/// and one it answers normally, counting the agent events of each.
async fn refusal_then_assistant_error(directory: &Path) {
    let scripted = Scripted::new(directory, InteractiveModeOptions::default());
    let session = scripted.world.host.session();
    let events = Rc::new(Cell::new(0));
    let seen = Rc::clone(&events);
    let remove = session.subscribe(Rc::new(move |event| {
        if matches!(event, AgentSessionEvent::Agent(_)) {
            seen.set(seen.get() + 1);
        }
    }));
    let refused = scripted.refusal().await;
    assert!(refused.starts_with("No API key found"), "{refused}");
    assert_eq!(events.get(), 0);
    assert!(admitted(&session).is_empty());
    let storage = &scripted.world.host.services().auth_storage;
    storage.set_runtime_api_key("faux", "controlled-key");
    let failure = Some("controlled refusal");
    scripted
        .provider
        .set_responses(vec![answer(failure), answer(None)]);
    let failed = session.prompt("streamed", PromptOptions::default()).await;
    assert!(failed.is_ok());
    assert_eq!(scripted.provider.state.call_count(), 1);
    assert!(events.get() > 0);
    assert_eq!(assistant_errors(&session), ["controlled refusal"]);
    let after = session.prompt("after", PromptOptions::default()).await;
    assert!(after.is_ok());
    assert_eq!(scripted.provider.state.call_count(), 2);
    let texts: Vec<_> = admitted(&session)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    assert_eq!(texts, ["streamed", "after"]);
    assert_eq!(assistant_errors(&session), ["controlled refusal"]);
    remove();
    scripted.finish();
}

/// One startup list size.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Size {
    /// The number of startup prompts.
    n: usize,
}

/// The recorded work of one startup list size.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Work {
    /// The prompts sent.
    prompts: usize,
    /// The reads of a traversal that rescans every earlier prompt before each one.
    rescan_negative: usize,
}

/// Drives the loop over `texts` until it waits for typed input, returning the reads, the
/// `next` calls and the admitted prompts; a rescanning traversal reads every earlier prompt
/// again before each one.
async fn traverse(directory: &Path, texts: &[String], rescan: bool) -> (usize, usize, Vec<String>) {
    let scripted = Scripted::new(directory, InteractiveModeOptions::default());
    let answers = texts.iter().map(|_| answer(None)).collect();
    scripted.provider.set_responses(answers);
    let reads = Cell::new(0);
    let calls = Cell::new(0);
    let mut items = from_prompts(texts, rescan, &reads);
    let counted = std::iter::from_fn(|| {
        calls.set(calls.get() + 1);
        items.next()
    });
    let mode = &scripted.world.mode;
    until_waiting(mode, pin!(mode.run_prompts(counted))).await;
    let session = scripted.world.host.session();
    let admitted = admitted(&session)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    scripted.finish();
    (reads.get(), calls.get(), admitted)
}

/// The startup prompts as the loop reads them, counting every read of a prompt: one per
/// prompt, or the whole prefix before each one when `rescan` is set.
fn from_prompts<'a>(
    texts: &'a [String],
    rescan: bool,
    reads: &'a Cell<usize>,
) -> impl Iterator<Item = (&'a str, Option<&'a [ImageContent]>)> {
    let mut next = 0;
    std::iter::from_fn(move || {
        let text = texts.get(next)?;
        let read = if rescan {
            &texts[..=next]
        } else {
            &texts[next..=next]
        };
        for _ in read {
            reads.set(reads.get() + 1);
        }
        next += 1;
        Some((text.as_str(), None))
    })
}

#[test]
fn startup_message_traversal_has_linear_work() {
    const TEST: &str = "startup_message_traversal_has_linear_work";
    isolated(TEST, async |directory| {
        for (size, work) in cases::<Size, Work>(FIXTURES, TEST) {
            (measure(directory, size, work)).await;
        }
    });
}

/// Compares the single-pass and the rescanning traversal of `size` prompts with `work`.
async fn measure(directory: &Path, size: Size, work: Work) {
    let texts: Vec<_> = (0..size.n)
        .map(|index| format!("message {index}"))
        .collect();
    let (reads, calls, admitted) = traverse(directory, &texts, false).await;
    assert_eq!((reads, calls), (work.prompts, work.prompts + 1));
    assert_eq!(admitted, texts);
    let (reads, calls, admitted) = traverse(directory, &texts, true).await;
    assert_eq!(reads, work.rescan_negative);
    assert!(
        reads > calls,
        "the linear bound rejects a rescanning traversal"
    );
    assert_eq!(admitted, texts);
}

#[test]
fn startup_subscription_check_is_started_between_fallback_and_prompts() {
    isolated(
        "startup_subscription_check_is_started_between_fallback_and_prompts",
        startup_subscription_check,
    );
}

/// Observes warning admission before the first prompt, without awaiting its completion.
async fn startup_subscription_check(directory: &Path) {
    let scripted = Scripted::new(
        directory,
        InteractiveModeOptions {
            model_fallback_message: Some("controlled fallback".into()),
            initial_message: Some("first prompt".into()),
            ..Default::default()
        },
    );
    let world = &scripted.world;
    super::start(world).await;
    assert!(world.runtime.drain_futures().is_empty());
    let model = world.host.session().model();
    let mut selected = model.read().unwrap().clone();
    selected.provider = "anthropic".into();
    world.host.session().state().write().unwrap().model = Arc::new(RwLock::new(selected));
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key("anthropic", "sk-ant-oat-controlled");
    scripted.provider.set_responses(vec![answer(None)]);
    let observed = Rc::new(Cell::new(None));
    let seen = observed.clone();
    let mode = Rc::downgrade(&world.mode);
    let runtime = world.runtime.clone();
    let unsubscribe = world.host.session().subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && matches!(*event.read().unwrap(), maestro_app::AgentEvent::AgentStart)
        {
            let mode = mode.upgrade().unwrap();
            seen.set(Some((
                super::actions::shows(&mode.chat_container, "controlled fallback"),
                mode.anthropic_subscription_warning_shown.get(),
                runtime.futures(),
            )));
        }
    }));
    let mut run = pin!(world.mode.run());
    until_waiting(&world.mode, run.as_mut()).await;
    assert_eq!(
        observed.get(),
        Some((true, false, 4)),
        "fallback is shown and the independent check is admitted before the first prompt"
    );
    assert_eq!(scripted.provider.state.call_count(), 1);
    assert!(world.runtime.drain_futures().is_empty());
    assert!(world.mode.anthropic_subscription_warning_shown.get());
    let rows = maestro_app::strip_ansi(&world.mode.chat_container.render(1000).join("\n")).unwrap();
    assert_eq!(
        rows.matches("Anthropic subscription auth is active.")
            .count(),
        1
    );
    unsubscribe();
    scripted.finish();
}
