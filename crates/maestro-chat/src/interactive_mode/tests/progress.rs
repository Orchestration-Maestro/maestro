//! Compaction and retry progress through the mode's own interface: the labels, the
//! completion rows, the countdown, the interrupt callbacks and the captured queue.

use super::super::queues::CompactionQueuedMessage;
use super::queues::{Kept, Mode};
use super::session;
use super::transcript::{append, recorded, user_record};
use super::{
    CommandHandler, Setup, World, answer, cases, held_response, isolated, painted, started,
};
use maestro_app::agent_session::{CompactionReason, PromptOptions};
use maestro_app::messages::{CustomAgentMessages, UserMessageDelivery};
use maestro_app::{
    AgentSession, AgentSessionEvent, AssistantMessageEventStream, CompactionResult,
    DiagnosticErrorInfo, Extension, ExtensionBindings, ExtensionCommandContext, ExtensionFactory,
    InputEvent, InputEventResult, Local, RegisteredCommand, ResourceLoader, Skill, SourceInfo,
    SourceOrigin, SourceScope, StopReason,
};
use maestro_tui::Component;
use maestro_tui::tui::InputHandler;
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
    time::Duration,
};

/// The recorded progress cases the fixture-driven witnesses read.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_progress.json");

/// One presentation the progress events leave behind.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    /// The status container's rows at width 100.
    status: Vec<String>,
    /// The transcript's rows at width 100.
    chat: Vec<String>,
    /// The terminal's progress requests.
    progress: Vec<bool>,
    /// The scheduled intervals in milliseconds.
    timers: Vec<u64>,
    /// The editor's text.
    editor: String,
}

impl Snapshot {
    /// Asserts the presentation, including the progress requests `effects` recorded.
    fn assert(&self, world: &World, effects: &Effects) {
        let theme = world.themes.theme().get().unwrap();
        assert_eq!(
            world.mode.status_container.render(100),
            painted(&theme, &self.status)
        );
        assert_eq!(
            world.mode.chat_container.render(100),
            painted(&theme, &self.chat)
        );
        assert_eq!(progress(effects), self.progress);
        assert_eq!(timers(world), self.timers);
        assert_eq!(world.mode.editor.get_text(), self.editor);
    }

    /// Asserts the presentation with the loader's animation frame replaced, for cases whose
    /// clock advanced: the frame is the loader's own test's claim, not the label's.
    fn assert_animated(&self, world: &World, effects: &Effects) {
        let theme = world.themes.theme().get().unwrap();
        assert_eq!(
            spinner_frames(&world.mode.status_container.render(100)),
            spinner_frames(&painted(&theme, &self.status))
        );
        assert_eq!(
            world.mode.chat_container.render(100),
            painted(&theme, &self.chat)
        );
        assert_eq!(progress(effects), self.progress);
        assert_eq!(timers(world), self.timers);
        assert_eq!(world.mode.editor.get_text(), self.editor);
    }
}

/// A compaction start and the end that follows it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartEnd {
    /// The presentation while the compaction runs.
    start: Snapshot,
    /// The presentation after it ends.
    end: Snapshot,
    /// Whether the replaced interrupt callback is back.
    restored: bool,
}

/// What a compaction end leaves behind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    /// The transcript's rows at width 100.
    chat: Vec<String>,
    /// The terminal's progress requests.
    progress: Vec<bool>,
    /// The scheduled intervals in milliseconds.
    timers: Vec<u64>,
    /// The editor's text.
    editor: String,
}

/// The terminal's emulated effects since the last tap.
type Effects = Rc<RefCell<Vec<String>>>;

/// The emulated progress sequence a terminal shows.
const PROGRESS_ACTIVE: &str = "\u{1b}]9;4;3\u{7}";
/// The emulated sequence that clears progress.
const PROGRESS_CLEAR: &str = "\u{1b}]9;4;0;\u{7}";

/// Collects the effects the terminal is asked to emulate from now on.
fn tap(world: &World) -> Effects {
    let effects: Effects = Rc::default();
    let collect = Rc::clone(&effects);
    world
        .terminal
        .set_tap(move |effect| collect.borrow_mut().push(effect.to_owned()));
    effects
}

/// The terminal's progress requests in order.
fn progress(effects: &Effects) -> Vec<bool> {
    effects
        .borrow()
        .iter()
        .filter_map(|effect| match effect.as_str() {
            PROGRESS_ACTIVE => Some(true),
            PROGRESS_CLEAR => Some(false),
            _ => None,
        })
        .collect()
}

/// The editor's cursor-blink interval, which every case runs beside the progress timers.
const EDITOR_BLINK_MS: u64 = 16;

/// The intervals the progress presentation scheduled, in ascending order.
fn timers(world: &World) -> Vec<u64> {
    let now = world.runtime.millis();
    world
        .runtime
        .deadlines()
        .into_iter()
        .map(|due| u64::try_from(due.as_millis()).unwrap_or(u64::MAX) - now)
        .filter(|interval| *interval != EDITOR_BLINK_MS)
        .collect()
}

/// Puts the recorded draft in the editor, which every case starts from.
fn draft(world: &World) {
    world.mode.editor.set_text("draft");
}

/// The compaction reason a recorded name selects.
fn reason(name: &str) -> CompactionReason {
    match name {
        "manual" => CompactionReason::Manual,
        "threshold" => CompactionReason::Threshold,
        "overflow" => CompactionReason::Overflow,
        other => panic!("unrecorded compaction reason {other:?}"),
    }
}

/// A stored compaction result with the recorded fields.
fn result(summary: &str, tokens_before: f64) -> CompactionResult {
    serde_json::from_value(serde_json::json!({
        "summary": summary,
        "firstKeptEntryId": "kept",
        "tokensBefore": tokens_before,
    }))
    .unwrap()
}

/// Starts one compaction of `reason`.
fn start_compaction(world: &World, reason: CompactionReason) {
    world
        .mode
        .handle_event(&AgentSessionEvent::CompactionStart { reason })
        .unwrap();
}

/// Ends the running compaction with a stored result.
fn end_compaction(
    world: &World,
    reason: CompactionReason,
    result: Option<&CompactionResult>,
    will_retry: bool,
) {
    world
        .mode
        .handle_event(&AgentSessionEvent::CompactionEnd {
            reason,
            result: result.cloned(),
            aborted: false,
            will_retry,
            error_message: None,
        })
        .unwrap();
}

/// Ends the running compaction as aborted.
fn abort_compaction(world: &World, reason: CompactionReason) {
    world
        .mode
        .handle_event(&AgentSessionEvent::CompactionEnd {
            reason,
            result: None,
            aborted: true,
            will_retry: false,
            error_message: None,
        })
        .unwrap();
}

/// Ends the running compaction with the supplied failure.
fn fail_compaction(world: &World, reason: CompactionReason, message: &str) {
    world
        .mode
        .handle_event(&AgentSessionEvent::CompactionEnd {
            reason,
            result: None,
            aborted: false,
            will_retry: false,
            error_message: Some(message.to_owned()),
        })
        .unwrap();
}

/// The compaction start and end cases and their own interruption and setting cases.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum CompactionInput {
    /// A compaction of one reason with the recorded settings.
    StartEnd {
        /// The recorded reason.
        reason: String,
        /// Whether terminal progress is enabled when it starts.
        progress: bool,
        /// Whether an explicit interrupt callback was installed when it started.
        previous: bool,
    },
    /// A compaction with interrupt keys configured or emptied.
    ConfiguredCancelHint {
        /// The keys bound to the interrupt action.
        keys: Vec<String>,
    },
    /// A compaction whose progress setting changes between its start and its end.
    LiveProgressSetting {
        /// The setting when it starts.
        before: bool,
        /// The setting when it ends.
        after: bool,
    },
}

/// The live progress setting case's own observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveProgress {
    /// The transcript's rows at width 100.
    chat: Vec<String>,
    /// The terminal's progress requests.
    progress: Vec<bool>,
    /// The editor's text.
    editor: String,
}

#[test]
fn compaction_labels_keep_reason_live_keys_and_editor() {
    const TEST: &str = "compaction_labels_keep_reason_live_keys_and_editor";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<CompactionInput, Value>(FIXTURES, TEST) {
            label_case(directory, input, expected).await;
        }
    });
}

/// Drives one recorded label case.
async fn label_case(directory: &Path, input: CompactionInput, expected: Value) {
    match input {
        CompactionInput::StartEnd {
            reason,
            progress,
            previous,
        } => {
            let expected: StartEnd = serde_json::from_value(expected).unwrap();
            let flags = Flags {
                progress,
                previous,
                keys: None,
            };
            start_end_case(directory, &reason, &flags, &expected).await;
        }
        CompactionInput::ConfiguredCancelHint { keys } => {
            let expected: StartEnd = serde_json::from_value(expected).unwrap();
            let flags = Flags {
                progress: false,
                previous: true,
                keys: Some(keys),
            };
            start_end_case(directory, "manual", &flags, &expected).await;
            bind_interrupt(&[], true);
        }
        CompactionInput::LiveProgressSetting { before, after } => {
            let expected: LiveProgress = serde_json::from_value(expected).unwrap();
            live_progress_case(directory, (before, after), &expected).await;
        }
    }
}

/// The settings a recorded compaction start reads.
#[derive(Clone)]
struct Flags {
    /// Whether terminal progress is enabled.
    progress: bool,
    /// Whether an explicit interrupt callback was installed.
    previous: bool,
    /// The interrupt keys to bind before it starts, when the case records any.
    keys: Option<Vec<String>>,
}

/// Binds the interrupt action to `keys`, the first of several or none at all, and restores
/// the default bindings when `restore` is set.
fn bind_interrupt(keys: &[String], restore: bool) {
    let bindings = if restore {
        Vec::new()
    } else {
        let bound = match keys {
            [] => maestro_tui::KeybindingKeys::Multiple(Vec::new()),
            [key] => maestro_tui::KeybindingKeys::Single(key.clone()),
            several => maestro_tui::KeybindingKeys::Multiple(several.to_vec()),
        };
        vec![("app.interrupt".to_owned(), Some(bound))]
    };
    maestro_tui::get_keybindings().set_user_bindings(bindings);
}

/// Starts a compaction of `reason`, ends it as aborted, and checks both presentations and
/// that the replaced interrupt callback is back.
async fn start_end_case(directory: &Path, reason_name: &str, flags: &Flags, expected: &StartEnd) {
    let world = started(directory).await;
    draft(&world);
    if let Some(keys) = &flags.keys {
        bind_interrupt(keys, false);
    }
    let reason = reason(reason_name);
    set_progress_setting(&world, flags.progress);
    let calls: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let previous_callback: Rc<dyn Fn()> = {
        let calls = Rc::clone(&calls);
        Rc::new(move || calls.borrow_mut().push("previous"))
    };
    if flags.previous {
        world
            .mode
            .editor
            .set_on_escape(Some(Rc::clone(&previous_callback)));
    }
    let effects = tap(&world);
    start_compaction(&world, reason);
    expected.start.assert(&world, &effects);
    // The interrupt key aborts this compaction instead of running the replaced callback.
    world.mode.editor.on_escape().unwrap()();
    assert_eq!(*calls.borrow(), Vec::<&'static str>::new());
    abort_compaction(&world, reason);
    expected.end.assert(&world, &effects);
    let restored = world.mode.editor.on_escape();
    assert_eq!(restored.is_some(), expected.restored, "{reason_name}");
    assert_eq!(expected.restored, flags.previous, "{reason_name}");
    if let Some(restored) = restored {
        assert!(Rc::ptr_eq(&restored, &previous_callback));
        restored();
        assert_eq!(*calls.borrow(), ["previous"]);
    }
}

/// Sets whether the terminal shows progress.
fn set_progress_setting(world: &World, shown: bool) {
    world
        .mode
        .runtime_host
        .session()
        .settings_manager()
        .borrow_mut()
        .set_show_terminal_progress(shown);
}

/// Starts with the setting `before`, changes it to `after` and ends the compaction, proving
/// each event reads the current value.
async fn live_progress_case(directory: &Path, settings: (bool, bool), expected: &LiveProgress) {
    let (before, after) = settings;
    let world = started(directory).await;
    draft(&world);
    set_progress_setting(&world, before);
    let effects = tap(&world);
    start_compaction(&world, CompactionReason::Manual);
    set_progress_setting(&world, after);
    abort_compaction(&world, CompactionReason::Manual);
    let theme = world.themes.theme().get().unwrap();
    assert_eq!(
        world.mode.chat_container.render(100),
        painted(&theme, &expected.chat)
    );
    assert_eq!(progress(&effects), expected.progress);
    assert_eq!(world.mode.editor.get_text(), expected.editor);
}

/// One compaction end of the recorded reason and outcome.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum CompletionInput {
    /// A compaction end of one reason and outcome.
    CompactionOutcome {
        /// The recorded reason.
        reason: String,
        /// The recorded producer outcome.
        outcome: String,
        /// Whether the failed prompt is retried.
        #[serde(rename = "willRetry")]
        will_retry: bool,
    },
    /// A successful compaction whose summary and token count are empty.
    CompactionEmptySummaryZeroTokens,
}

impl CompletionInput {
    /// Whether the compaction ends with a stored result, which replays the history.
    fn succeeds(&self) -> bool {
        match self {
            Self::CompactionOutcome { outcome, .. } => outcome == "result",
            Self::CompactionEmptySummaryZeroTokens => true,
        }
    }
}

/// The first user record of the seeded history.
const HISTORY_ALPHA: &str = "HISTORY_ALPHA";
/// The second user record of the seeded history.
const HISTORY_BETA: &str = "HISTORY_BETA";
/// A displayed row the session's history lacks, which the replay removes.
const STALE_TRANSCRIPT: &str = "STALE_TRANSCRIPT";

/// Seeds the session's history with two distinct user records, shows a row the history
/// lacks and expands summaries, so the replay and the whole summary are observable.
fn seed_history(world: &World) {
    append(
        world,
        vec![user_record(HISTORY_ALPHA), user_record(HISTORY_BETA)],
    );
    world.mode.show_status(STALE_TRANSCRIPT).unwrap();
    let rows = world.mode.chat_container.render(100);
    assert!(rows.iter().any(|row| row.contains(STALE_TRANSCRIPT)));
    world.mode.tool_output_expanded.set(true);
}

#[test]
fn compaction_completion_keeps_seeded_replay_and_one_summary() {
    const TEST: &str = "compaction_completion_keeps_seeded_replay_and_one_summary";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<CompletionInput, Outcome>(FIXTURES, TEST) {
            completion_case(directory, input, &expected).await;
        }
    });
}

/// Drives one recorded compaction end and asserts what it left.
async fn completion_case(directory: &Path, input: CompletionInput, expected: &Outcome) {
    let world = recorded(directory, Setup::default());
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    draft(&world);
    if input.succeeds() {
        seed_history(&world);
    }
    let effects = tap(&world);
    match input {
        CompletionInput::CompactionOutcome {
            reason: reason_name,
            outcome,
            will_retry,
        } => finish_compaction(&world, &reason_name, &outcome, will_retry),
        CompletionInput::CompactionEmptySummaryZeroTokens => {
            let stored = result("", 0.0);
            end_compaction(&world, CompactionReason::Manual, Some(&stored), false);
        }
    }
    world.runtime.settle().unwrap();
    let theme = world.themes.theme().get().unwrap();
    assert_eq!(
        world.mode.chat_container.render(100),
        painted(&theme, &expected.chat)
    );
    assert_eq!(progress(&effects), expected.progress);
    assert_eq!(timers(&world), expected.timers);
    assert_eq!(world.mode.editor.get_text(), expected.editor);
}

/// Starts and ends the recorded compaction outcome.
fn finish_compaction(world: &World, reason_name: &str, outcome: &str, will_retry: bool) {
    let reason = reason(reason_name);
    start_compaction(world, reason);
    match outcome {
        "aborted" => abort_compaction(world, reason),
        "result" => {
            let stored = result("summary", 123.0);
            end_compaction(world, reason, Some(&stored), will_retry);
        }
        "error" => fail_compaction(world, reason, "provider failure"),
        "empty" => end_without_outcome(world, reason, will_retry),
        "empty-error" => fail_compaction(world, reason, ""),
        other => panic!("unrecorded outcome {other:?}"),
    }
}

/// Ends the compaction with neither a result nor an error.
fn end_without_outcome(world: &World, reason: CompactionReason, will_retry: bool) {
    world
        .mode
        .handle_event(&AgentSessionEvent::CompactionEnd {
            reason,
            result: None,
            aborted: false,
            will_retry,
            error_message: None,
        })
        .unwrap();
}

/// Rows with the loader's animation frame replaced, as the recorded cases only ran the
/// countdown's own timer.
fn spinner_frames(rows: &[String]) -> Vec<String> {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    rows.iter()
        .map(|row| {
            FRAMES
                .iter()
                .fold(row.clone(), |row, frame| row.replace(frame, "⠋"))
        })
        .collect()
}

/// The default interrupt key.
const INTERRUPT: &str = "\x1b";

/// An interrupt callback installed before a progress state, counting its own runs.
struct PreviousAction {
    /// The installed callback.
    action: Rc<dyn Fn()>,
    /// How many times the callback ran.
    runs: Rc<Cell<usize>>,
}

impl PreviousAction {
    /// Installs a counting callback as the editor's interrupt action.
    fn install(world: &World) -> Self {
        let runs = Rc::new(Cell::new(0));
        let count = Rc::clone(&runs);
        let action: Rc<dyn Fn()> = Rc::new(move || count.set(count.get() + 1));
        world.mode.editor.set_on_escape(Some(Rc::clone(&action)));
        Self { action, runs }
    }

    /// Presses the interrupt key `key` and checks that it did not reach this callback.
    fn assert_replaced(&self, world: &World, key: &str) {
        let runs = self.runs.get();
        world.mode.editor.handle_input(key);
        assert_eq!(
            self.runs.get(),
            runs,
            "the interrupt reached the replaced callback"
        );
    }

    /// Checks that the editor's interrupt action is this callback again and that the
    /// interrupt key `key` runs it.
    fn assert_restored(&self, world: &World, key: &str) {
        let restored = world.mode.editor.on_escape().unwrap();
        assert!(
            Rc::ptr_eq(&restored, &self.action),
            "another interrupt action"
        );
        let runs = self.runs.get();
        world.mode.editor.handle_input(key);
        assert_eq!(self.runs.get(), runs + 1);
    }
}

/// Starts a retry with the recorded numbers.
fn start_retry(world: &World, attempt: u32, max_attempts: f64, delay_ms: f64) {
    world
        .mode
        .handle_event(&AgentSessionEvent::AutoRetryStart {
            attempt,
            max_attempts,
            delay_ms,
            error_message: "not a displayed retry label".to_owned(),
        })
        .unwrap();
}

/// Delivers one agent event as the session publishes it.
fn agent_event(world: &World, event: maestro_app::AgentEvent<CustomAgentMessages>) {
    world
        .mode
        .handle_event(&AgentSessionEvent::Agent(Arc::new(RwLock::new(event))))
        .unwrap();
}

/// A completed response that admitted no message.
fn agent_end() -> maestro_app::AgentEvent<CustomAgentMessages> {
    maestro_app::AgentEvent::AgentEnd {
        messages: Vec::new(),
    }
}

/// Ends the running retry.
fn end_retry(world: &World, success: bool, attempt: u32, final_error: Option<&str>) {
    world
        .mode
        .handle_event(&AgentSessionEvent::AutoRetryEnd {
            success,
            attempt,
            final_error: final_error.map(str::to_owned),
        })
        .unwrap();
}

/// The retry countdown cases, their frames and their end.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum RetryInput {
    /// A retry with the recorded wait and outcome.
    RetryCountdown {
        /// The wait before the attempt.
        #[serde(rename = "delayMs")]
        delay_ms: f64,
        /// Whether the retry succeeded.
        success: bool,
    },
    /// A retry whose limit and wait are fractional.
    RetryFractionalLimit {
        /// The configured limit.
        #[serde(rename = "maxAttempts")]
        max_attempts: f64,
        /// The wait before the attempt.
        #[serde(rename = "delayMs")]
        delay_ms: f64,
    },
}

/// The frames a countdown shows and the presentation its end leaves.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Countdown {
    /// The initial presentation and one per tick.
    frames: Vec<Snapshot>,
    /// The presentation after the retry ends.
    end: Snapshot,
}

/// A retry's start and end presentation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryStartEnd {
    /// The presentation while the countdown runs.
    start: Snapshot,
    /// The presentation after the retry ends.
    end: Snapshot,
}

#[test]
fn retry_countdown_interrupts_then_restores_previous_action() {
    const TEST: &str = "retry_countdown_interrupts_then_restores_previous_action";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<RetryInput, Value>(FIXTURES, TEST) {
            match input {
                RetryInput::RetryCountdown { delay_ms, success } => {
                    let expected: Countdown = serde_json::from_value(expected).unwrap();
                    countdown_case(directory, delay_ms, success, &expected).await;
                }
                RetryInput::RetryFractionalLimit {
                    max_attempts,
                    delay_ms,
                } => {
                    let expected: RetryStartEnd = serde_json::from_value(expected).unwrap();
                    let world = started(directory).await;
                    draft(&world);
                    let previous = PreviousAction::install(&world);
                    let effects = tap(&world);
                    start_retry(&world, 1, max_attempts, delay_ms);
                    expected.start.assert(&world, &effects);
                    previous.assert_replaced(&world, INTERRUPT);
                    end_retry(&world, true, 1, None);
                    expected.end.assert(&world, &effects);
                    previous.assert_restored(&world, INTERRUPT);
                }
            }
        }
    });
}

/// Shows the initial frame and one per second tick, then ends the retry.
async fn countdown_case(directory: &Path, delay_ms: f64, success: bool, recorded: &Countdown) {
    let world = started(directory).await;
    draft(&world);
    let previous = PreviousAction::install(&world);
    let effects = tap(&world);
    start_retry(&world, 2, 3.0, delay_ms);
    let mut frames = vec![describe(&world, &effects)];
    let started_at = world.runtime.millis();
    for tick in 1..recorded.frames.len() {
        let tick = u64::try_from(tick).unwrap_or(u64::MAX);
        let _ = world
            .runtime
            .advance_to(Duration::from_millis(started_at + 1000 * tick));
        frames.push(describe(&world, &effects));
    }
    let theme = world.themes.theme().get().unwrap();
    let expected: Vec<Snapshot> = recorded
        .frames
        .iter()
        .map(|frame| Snapshot {
            status: spinner_frames(&painted(&theme, &frame.status)),
            chat: painted(&theme, &frame.chat),
            progress: frame.progress.clone(),
            timers: frame.timers.clone(),
            editor: frame.editor.clone(),
        })
        .collect();
    let frames: Vec<Snapshot> = frames
        .into_iter()
        .map(|mut frame| {
            frame.status = spinner_frames(&frame.status);
            frame
        })
        .collect();
    assert_eq!(frames, expected, "{delay_ms} ms");
    previous.assert_replaced(&world, INTERRUPT);
    end_retry(&world, success, 2, Some("last failure"));
    recorded.end.assert(&world, &effects);
    previous.assert_restored(&world, INTERRUPT);
}

/// Reads the presentation as a comparable snapshot.
fn describe(world: &World, effects: &Effects) -> Snapshot {
    Snapshot {
        status: world.mode.status_container.render(100),
        chat: world.mode.chat_container.render(100),
        progress: progress(effects),
        timers: timers(world),
        editor: world.mode.editor.get_text(),
    }
}

/// One retry end and what it leaves.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum RetryEndInput {
    /// A retry end with the recorded outcome.
    RetryEnd {
        /// Whether the retry succeeded.
        success: bool,
        /// The final failure; absent when the producer reports none.
        #[serde(rename = "finalError", default)]
        final_error: Option<String>,
    },
}

#[test]
fn retry_completion_cancels_wait_and_restores_previous_action() {
    const TEST: &str = "retry_completion_cancels_wait_and_restores_previous_action";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<RetryEndInput, Snapshot>(FIXTURES, TEST) {
            let RetryEndInput::RetryEnd {
                success,
                final_error,
            } = input;
            let world = started(directory).await;
            draft(&world);
            let previous = PreviousAction::install(&world);
            let effects = tap(&world);
            start_retry(&world, 3, 3.0, 1000.0);
            previous.assert_replaced(&world, INTERRUPT);
            end_retry(&world, success, 3, final_error.as_deref());
            expected.assert(&world, &effects);
            assert!(world.mode.status_container.render(100).is_empty());
            previous.assert_restored(&world, INTERRUPT);
        }
        interrupted_wait_case(directory).await;
    });
}

/// The notice an interrupted first retry leaves at the end of the transcript.
const RETRY_CANCELLED: [&str; 2] = [
    "",
    " @error@Error: Retry failed after 1 attempts: Retry cancelled\u{1b}[39m                                              ",
];

/// Makes every provider request of `world`'s session count itself before the installed
/// provider answers it.
fn count_requests(world: &World) -> Rc<Cell<usize>> {
    let session = world.mode.runtime_host.session();
    let provider = session.agent().stream_fn.read().unwrap().clone().unwrap();
    let requests = Rc::new(Cell::new(0));
    let count = Rc::clone(&requests);
    *session.agent().stream_fn.write().unwrap() =
        Some(Rc::new(move |model, context, options, extra| {
            count.set(count.get() + 1);
            provider(model, context, options, extra)
        }));
    requests
}

/// The retry events a session publishes, in order with their payloads, and a notification
/// for each start and each end.
struct RetryWatch {
    /// Each start and end with its payload.
    events: Rc<RefCell<Vec<String>>>,
    /// Notified once a retry's start listeners have returned.
    started: Rc<tokio::sync::Notify>,
    /// Notified once a retry's end listeners have returned.
    ended: Rc<tokio::sync::Notify>,
    /// Ends the subscription.
    removal: Box<dyn Fn()>,
}

impl RetryWatch {
    /// Subscribes to the retry events of `session`.
    fn subscribe(session: &AgentSession) -> Self {
        let events: Rc<RefCell<Vec<String>>> = Rc::default();
        let (started, ended): (Rc<tokio::sync::Notify>, Rc<tokio::sync::Notify>) =
            Default::default();
        let (log, start, end) = (Rc::clone(&events), Rc::clone(&started), Rc::clone(&ended));
        let removal = session.subscribe(Rc::new(move |event| match event {
            AgentSessionEvent::AutoRetryStart {
                attempt,
                max_attempts,
                delay_ms,
                error_message,
            } => {
                let text = format!("start {attempt}/{max_attempts} {delay_ms} {error_message}");
                log.borrow_mut().push(text);
                start.notify_one();
            }
            AgentSessionEvent::AutoRetryEnd {
                success,
                attempt,
                final_error,
            } => {
                let text = format!("end {success} {attempt} {final_error:?}");
                log.borrow_mut().push(text);
                end.notify_one();
            }
            _ => {}
        }));
        Self {
            events,
            started,
            ended,
            removal: Box::new(removal),
        }
    }
}

/// Fails the held response of a real prompt with a retryable error, presses the interrupt
/// key while the retry waits under a paused clock, and checks that the wait ends cancelled
/// without another request and restores the replaced callback.
async fn interrupted_wait_case(directory: &Path) {
    let world = started(directory).await;
    draft(&world);
    let session = world.mode.runtime_host.session();
    session
        .settings_manager()
        .borrow_mut()
        .set_global_key("retry", "baseDelayMs", 100);
    let previous = PreviousAction::install(&world);
    let held = held_response(&world);
    let requests = count_requests(&world);
    let watch = RetryWatch::subscribe(&session);
    tokio::time::pause();
    let prompted = Rc::clone(&session);
    let run =
        tokio::task::spawn_local(
            async move { prompted.prompt("run", PromptOptions::default()).await },
        );
    let failure = answer();
    {
        let mut message = failure.write().unwrap();
        message.stop_reason = StopReason::Error;
        message.error_message = Some("overloaded_error".to_owned());
    }
    held.await.unwrap().stream.end(Some(failure));
    // The start's listener has returned and the producer now waits out its backoff.
    watch.started.notified().await;
    previous.assert_replaced(&world, INTERRUPT);
    assert!(
        !session.is_retrying(),
        "the interrupt left the retry waiting"
    );
    watch.ended.notified().await;
    run.await.unwrap().unwrap();
    tokio::time::resume();
    (watch.removal)();
    assert_eq!(requests.get(), 1);
    assert_eq!(
        *watch.events.borrow(),
        [
            "start 1/3 100 overloaded_error",
            "end false 1 Some(\"Retry cancelled\")"
        ]
    );
    let theme = world.themes.theme().get().unwrap();
    let rows = world.mode.chat_container.render(100);
    let notice = painted(&theme, &RETRY_CANCELLED.map(str::to_owned));
    assert!(rows.ends_with(&notice), "{rows:?}");
    let failures = rows.iter().filter(|row| row.contains("Retry failed"));
    assert_eq!(failures.count(), 1);
    previous.assert_restored(&world, INTERRUPT);
}

#[test]
fn response_progress_pairs_each_start_with_an_end() {
    isolated(
        "response_progress_pairs_each_start_with_an_end",
        async |directory| {
            let world = started(directory).await;
            draft(&world);
            set_progress_setting(&world, true);
            let effects = tap(&world);
            agent_event(&world, maestro_app::AgentEvent::AgentStart);
            assert_eq!(progress(&effects), [true]);
            agent_event(&world, agent_end());
            assert_eq!(progress(&effects), [true, false]);
            // A retried response pairs its own start with its own end.
            agent_event(&world, maestro_app::AgentEvent::AgentStart);
            agent_event(&world, agent_end());
            assert_eq!(progress(&effects), [true, false, true, false]);
            // The setting alone decides whether the terminal is written at all.
            set_progress_setting(&world, false);
            let silent = tap(&world);
            agent_event(&world, maestro_app::AgentEvent::AgentStart);
            agent_event(&world, agent_end());
            assert_eq!(progress(&silent), Vec::<bool>::new());
        },
    );
}

/// The replacement of one retry by a response and a later retry.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum ReplaceInput {
    /// A retry, a live key change, a response, another retry and its end.
    RetryReplaceAndLiveKeys,
}

/// The four presentations the replacement case compares.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    /// The retry after its first tick with the keys rebound.
    changed: Snapshot,
    /// The working row the response shows in place of the retry.
    working: Snapshot,
    /// The second retry's first presentation.
    after: Snapshot,
    /// The presentation after the second retry ends.
    end: Snapshot,
}

/// The rebound interrupt key `ctrl+q`.
const CTRL_Q: &str = "\x11";

#[test]
fn retry_replacement_preserves_working_row_and_previous_action() {
    const TEST: &str = "retry_replacement_preserves_working_row_and_previous_action";
    isolated(TEST, async |directory| {
        let (input, expected) = cases::<ReplaceInput, Replacement>(FIXTURES, TEST).remove(0);
        let ReplaceInput::RetryReplaceAndLiveKeys = input;
        let world = started(directory).await;
        draft(&world);
        let previous = PreviousAction::install(&world);
        let effects = tap(&world);
        start_retry(&world, 1, 4.0, 2500.0);
        let first_tick = world.runtime.millis() + 1000;
        bind_interrupt(&["ctrl+q".to_owned(), "alt+x".to_owned()], false);
        let _ = world.runtime.advance_to(Duration::from_millis(first_tick));
        expected.changed.assert_animated(&world, &effects);
        previous.assert_replaced(&world, CTRL_Q);
        // A response starts: the retry's countdown, row and callback go before the working
        // row starts.
        agent_event(&world, maestro_app::AgentEvent::AgentStart);
        let restored = world.mode.editor.on_escape().unwrap();
        assert!(Rc::ptr_eq(&restored, &previous.action));
        expected.working.assert(&world, &effects);
        // A late successful end of the replaced retry leaves the working row alone.
        end_retry(&world, true, 1, None);
        expected.working.assert(&world, &effects);
        let kept = world.mode.editor.on_escape().unwrap();
        assert!(Rc::ptr_eq(&kept, &previous.action));
        start_retry(&world, 2, 4.0, 1000.0);
        expected.after.assert_animated(&world, &effects);
        previous.assert_replaced(&world, CTRL_Q);
        end_retry(&world, true, 2, None);
        expected.end.assert(&world, &effects);
        previous.assert_restored(&world, CTRL_Q);
        bind_interrupt(&[], true);
    });
}

#[test]
fn compaction_escape_restores_main_action_and_admits_editor_input() {
    isolated(
        "compaction_escape_restores_main_action_and_admits_editor_input",
        async |directory| {
            let world = started(directory).await;
            let calls: Rc<RefCell<Vec<&'static str>>> = Rc::default();
            let probe: Rc<dyn Fn()> = {
                let calls = Rc::clone(&calls);
                Rc::new(move || calls.borrow_mut().push("previous"))
            };
            world.mode.editor.set_on_escape(Some(Rc::clone(&probe)));
            let session = world.mode.runtime_host.session();
            let aborted: Rc<RefCell<Vec<bool>>> = Rc::default();
            let ended = Rc::clone(&aborted);
            let observer = session.subscribe(Rc::new(move |event| {
                if let AgentSessionEvent::CompactionEnd { aborted, .. } = event {
                    ended.borrow_mut().push(*aborted);
                }
            }));
            let mode = Rc::downgrade(&world.mode);
            let interrupt = session.subscribe(Rc::new(move |event| {
                if let AgentSessionEvent::CompactionStart { .. } = event
                    && let Some(mode) = mode.upgrade()
                {
                    // The interrupt key aborts the compaction that just started.
                    mode.editor.handle_input("\x1b");
                }
            }));
            world
                .host
                .services()
                .auth_storage
                .set_runtime_api_key("unknown", "key");
            drop(session.compact(None).await);
            interrupt();
            observer();
            assert_eq!(*aborted.borrow(), [true], "{:?}", aborted.borrow());
            let restored = world.mode.editor.on_escape();
            assert!(restored.is_some());
            assert!(Rc::ptr_eq(&restored.unwrap(), &probe));
            // An end without a start leaves the replaced callback and a status row alone.
            world.mode.show_status("kept").unwrap();
            abort_compaction(&world, CompactionReason::Manual);
            let restored = world.mode.editor.on_escape().unwrap();
            assert!(Rc::ptr_eq(&restored, &probe));
            assert!(
                world
                    .mode
                    .chat_container
                    .render(100)
                    .iter()
                    .any(|row| row.contains("kept"))
            );
            // Ordinary input still reaches the editor after the restore.
            world.mode.editor.set_text("");
            world.mode.editor.handle_input("typed");
            assert_eq!(world.mode.editor.get_text(), "typed");
        },
    );
}

#[test]
fn progress_callbacks_release_mode_and_cancel_timers() {
    isolated(
        "progress_callbacks_release_mode_and_cancel_timers",
        async |directory| {
            let world = started(directory).await;
            start_compaction(&world, CompactionReason::Manual);
            start_retry(&world, 1, 3.0, 2500.0);
            let writes = world.terminal.writes().len();
            let interrupt = world.mode.editor.on_escape().unwrap();
            let released = Rc::downgrade(&world.mode);
            drop(world.mode);
            assert!(released.upgrade().is_none(), "the callbacks keep no mode");
            // The retained interrupt and the timers cannot paint or resurrect the mode.
            interrupt();
            let _ = world.runtime.advance_to(Duration::from_millis(4000));
            assert_eq!(world.terminal.writes().len(), writes);
        },
    );
}

// ---- the captured compaction queue -----------------------------------------

/// The phase a flush case refuses.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Fault {
    /// The registered command's handler fails, which the refused message reports.
    Command,
    /// The queued `/skill:broken` text fails to expand.
    Queue,
}

/// The controlled session's refusal for one flush case, and how its error reads.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Faults {
    /// The plain prompt text whose prompt the input handler refuses.
    #[serde(default)]
    prompt: Option<String>,
    /// The phase that refuses, when the case refuses one.
    #[serde(default)]
    fault: Option<Fault>,
    /// The refusal message every failing producer reports.
    #[serde(default)]
    message: String,
    /// Whether the refused prompt and the command wait for their releases first.
    #[serde(default)]
    delayed: bool,
    /// Whether a released command fails instead of returning.
    #[serde(default)]
    held_fails: bool,
}

impl Faults {
    /// Whether any producer refuses, which needs the failing error listener.
    fn refuses(&self) -> bool {
        self.prompt.is_some() || self.fault.is_some()
    }

    /// Whether the queued skill text refuses the expansion.
    fn queues(&self) -> bool {
        self.fault == Some(Fault::Queue)
    }

    /// Whether the registered command's handler refuses.
    fn commands(&self) -> bool {
        self.fault == Some(Fault::Command) || self.held_fails
    }
}

/// What a flush case's first command changes before the rest of the batch is sent.
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Edit {
    /// Nothing.
    #[default]
    None,
    /// The editor's text.
    Editor,
    /// The command `late` is registered.
    AddCommand,
    /// The command `late` is removed.
    RemoveCommand,
}

/// The release a held phase waits for.
type Wait = tokio::sync::oneshot::Receiver<()>;

/// The removals keeping the harness's subscriptions alive.
type Removals = Rc<RefCell<Vec<Box<dyn Fn()>>>>;

/// What the controlled producers delivered, in order, and the releases the cases drive.
#[derive(Clone, Default)]
struct Captured {
    /// Every operation the captured batch reached, in order: `a prompt <text>`,
    /// `a command /<name> <arguments>`, `a steer <text>`, `a follow-up <text>` or `a clear`.
    log: Rc<RefCell<Vec<String>>>,
    /// The steering and follow-up texts the last queue update reported.
    queued: Rc<RefCell<(Vec<String>, Vec<String>)>>,
    /// The plain prompt text the input handler refuses.
    refused: Rc<RefCell<Option<String>>>,
    /// The refusal message every failing producer reports.
    message: Rc<RefCell<String>>,
    /// Whether a refusal is configured, which registers the failing error listener.
    refuses: Rc<Cell<bool>>,
    /// Whether the refused prompt waits for its release before it fails.
    delayed: Rc<Cell<bool>>,
    /// Whether a command handler waits for its release.
    held: Rc<Cell<bool>>,
    /// Whether a command fails instead of returning.
    command_fails: Rc<Cell<bool>>,
    /// The release the test sends to the refused prompt.
    prompt_gate: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// The release the refused prompt waits for.
    prompt_wait: Rc<RefCell<Option<tokio::sync::oneshot::Receiver<()>>>>,
    /// The release the test sends to a waiting command.
    command_gate: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// The release a waiting command handler waits for.
    command_wait: Rc<RefCell<Option<tokio::sync::oneshot::Receiver<()>>>>,
    /// The change the first command makes.
    edit: Rc<Cell<Edit>>,
    /// Whether the first command replaces the session.
    replace: Rc<Cell<bool>>,
    /// Whether the session was replaced.
    replaced: Rc<Cell<bool>>,
    /// The subscriptions recording the sessions' queue updates.
    subscriptions: Removals,
}

impl Captured {
    /// The session an operation reached, `a` for the one the mode started with and `b` for
    /// its replacement.
    fn session(&self) -> &'static str {
        if self.replaced.get() { "b" } else { "a" }
    }

    /// Records `kind` and `text` against the session they reached.
    fn record(&self, kind: &str, text: &str) {
        self.log
            .borrow_mut()
            .push(format!("{} {kind} {text}", self.session()));
    }

    /// Records what one queue update changed: a grown steering or follow-up queue.
    fn record_queue(&self, steering: &[String], follow_up: &[String]) {
        let (was_steering, was_follow_up) = self.queued.borrow().clone();
        if let Some(text) = grown(&was_steering, steering) {
            self.record("steer", &text);
        } else if let Some(text) = grown(&was_follow_up, follow_up) {
            self.record("follow-up", &text);
        }
        *self.queued.borrow_mut() = (steering.to_vec(), follow_up.to_vec());
    }

    /// Creates the two releases a delayed case waits for.
    fn delay(&self) {
        let (release, wait) = tokio::sync::oneshot::channel();
        *self.prompt_gate.borrow_mut() = Some(release);
        *self.prompt_wait.borrow_mut() = Some(wait);
        let (release, wait) = tokio::sync::oneshot::channel();
        *self.command_gate.borrow_mut() = Some(release);
        *self.command_wait.borrow_mut() = Some(wait);
    }

    /// The release and message of a refused prompt, absent when `text` is not refused.
    fn refusal(&self, text: &str) -> Option<(Option<Wait>, String)> {
        if self.refused.borrow().as_deref() != Some(text) {
            return None;
        }
        let wait = self
            .delayed
            .get()
            .then(|| self.prompt_wait.borrow_mut().take())
            .flatten();
        Some((wait, self.message.borrow().clone()))
    }

    /// The release a waiting command handler waits for, absent when the case holds none.
    fn command_release_wait(&self) -> Option<Wait> {
        self.command_wait.borrow_mut().take()
    }

    /// The release the refused prompt waits for.
    fn release_prompt(&self) -> tokio::sync::oneshot::Sender<()> {
        self.prompt_gate
            .borrow_mut()
            .take()
            .expect("a delayed refusal holds a release")
    }

    /// The release a waiting command handler waits for.
    fn release_command(&self) -> tokio::sync::oneshot::Sender<()> {
        self.command_gate
            .borrow_mut()
            .take()
            .expect("a held command holds a release")
    }
}

/// The text that appeared at the end of `queue` since `was`, if any.
fn grown(was: &[String], queue: &[String]) -> Option<String> {
    (queue.len() > was.len()).then(|| queue[queue.len() - 1].clone())
}

/// The recorded input of one flush case.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FlushInput {
    /// Whether the compaction retries, which queues the batch behind the retry.
    will_retry: bool,
    /// The messages the compaction captured, in order.
    local: Vec<Kept>,
    /// What the controlled session refuses.
    #[serde(default)]
    faults: Faults,
    /// The change the first command makes.
    #[serde(default)]
    edit: Edit,
    /// Whether the first command replaces the session.
    #[serde(default)]
    replace: bool,
}

/// The recorded observation of one flush case.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FlushExpected {
    /// The operations the captured batch reached, in order.
    log: Vec<String>,
    /// The messages still kept for after a compaction.
    local: Vec<Kept>,
    /// The failure notices the transcript shows.
    errors: Vec<String>,
    /// The pending container's rows.
    pending: usize,
    /// The editor's text.
    editor: String,
}

/// The loader the replacement sessions share with the session the mode starts with.
type LoaderSlot = Rc<RefCell<Option<Rc<dyn ResourceLoader<Local>>>>>;

/// Builds a started mode whose session delivers a captured batch through controlled
/// producers: its input handler records every prompt and refuses the case's text, the
/// commands `cmd` and `late` record their runs and make the case's edit, a broken skill
/// fails the queued text's expansion, and every streamed answer is empty.
async fn flush_world(
    directory: &Path,
    input: &FlushInput,
    captured: &Captured,
) -> (World, Box<dyn Fn()>) {
    *captured.refused.borrow_mut() = input.faults.prompt.clone();
    *captured.message.borrow_mut() = input.faults.message.clone();
    captured.refuses.set(input.faults.refuses());
    captured.delayed.set(input.faults.delayed);
    captured.held.set(input.faults.delayed);
    captured.command_fails.set(input.faults.commands());
    captured.edit.set(input.edit);
    captured.replace.set(input.replace);
    let mut setup = Setup {
        extension: Some(captured_extension(captured)),
        ..Setup::default()
    };
    if input.faults.queues() {
        setup.skills = vec![broken_skill(directory)];
    }
    let loader: LoaderSlot = Rc::default();
    setup.create = Some(flush_factory(directory, captured, &loader));
    let world = setup.build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world.mode.chat_container.clear();
    let session = world.mode.runtime_host.session();
    answer_session(&session);
    session
        .settings_manager()
        .borrow_mut()
        .set_compaction_enabled(false);
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key("unknown", "key");
    *loader.borrow_mut() = Some(Rc::clone(session.resource_loader()));
    if input.replace {
        let bindings = ExtensionBindings {
            command_context_actions: Some(world.host.command_context_actions()),
            ..ExtensionBindings::default()
        };
        session.bind_extensions(bindings).await.unwrap();
    }
    refuse_on(&session, captured);
    let extension = Rc::clone(&session.resource_loader().get_extensions().extensions[0]);
    register_command(
        &extension,
        "cmd",
        command_handler(&extension, &world, captured),
    );
    if matches!(input.edit, Edit::Editor | Edit::RemoveCommand) {
        register_command(&extension, "late", recording_handler(captured, "late"));
    }
    let removal = subscribe_queue(&session, captured);
    (world, removal)
}

/// Creates the session each replacement starts, over the loader the mode's own session
/// reads its resources from.
fn flush_factory(
    directory: &Path,
    captured: &Captured,
    loader: &LoaderSlot,
) -> maestro_app::CreateAgentSessionRuntimeFactory {
    let loader = Rc::clone(loader);
    let captured = captured.clone();
    let cwd = directory.to_str().unwrap().to_owned();
    Rc::new(move |_| {
        let loader = loader.borrow().clone();
        let cwd = cwd.clone();
        let captured = captured.clone();
        Box::pin(async move {
            let result = session::session(&cwd, None, move |inner| loader.unwrap_or(inner));
            answer_session(&result.session);
            result
                .services
                .auth_storage
                .set_runtime_api_key("unknown", "key");
            result
                .session
                .settings_manager()
                .borrow_mut()
                .set_compaction_enabled(false);
            captured
                .subscriptions
                .borrow_mut()
                .push(subscribe_queue(&result.session, &captured));
            refuse_on(&result.session, &captured);
            Ok(result)
        })
    })
}

/// Subscribes the harness to `session`'s queue updates, keeping the removal alive.
fn subscribe_queue(session: &Rc<AgentSession>, captured: &Captured) -> Box<dyn Fn()> {
    let log = captured.clone();
    Box::new(session.subscribe(Rc::new(move |event: &AgentSessionEvent| {
        if let AgentSessionEvent::QueueUpdate {
            steering,
            follow_up,
        } = event
        {
            log.record_queue(steering, follow_up);
        }
    })))
}

/// Makes every failure listener of `session` fail with the case's message, so a refused
/// phase ends the operation instead of only being reported.
fn refuse_on(session: &Rc<AgentSession>, captured: &Captured) {
    if !captured.refuses.get() {
        return;
    }
    let message = captured.message.borrow().clone();
    drop(session.extension_runner().on_error(Rc::new(move |_| {
        Err(Box::new(DiagnosticErrorInfo::new(None, message.clone())))
    })));
}

/// The inline extension whose input handler records every prompt and refuses the case's text,
/// waiting for its release when the case delays it.
fn captured_extension(captured: &Captured) -> ExtensionFactory<Local> {
    let captured = captured.clone();
    Rc::new(move |api| {
        let captured = captured.clone();
        let registered = api.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                captured.record("prompt", &event.text);
                Box::pin(refused(captured.refusal(&event.text)))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// Refuses the prompt `refusal` describes once its release opens, and continues otherwise.
async fn refused(
    refusal: Option<(Option<Wait>, String)>,
) -> Result<Option<InputEventResult>, Box<dyn std::error::Error + Send + Sync>> {
    let Some((wait, message)) = refusal else {
        return Ok(None);
    };
    if let Some(wait) = wait {
        let _ = wait.await;
    }
    Err(message.into())
}

/// The skill whose instruction file is missing, so expanding its command reports an error.
fn broken_skill(directory: &Path) -> Skill {
    Skill {
        name: "broken".to_owned(),
        description: "A skill whose instructions are missing".to_owned(),
        file_path: directory
            .join("missing-skill.md")
            .to_str()
            .unwrap()
            .to_owned(),
        base_dir: directory.to_str().unwrap().to_owned(),
        source_info: SourceInfo {
            path: directory.join("skill").to_str().unwrap().to_owned(),
            source: "local".to_owned(),
            scope: SourceScope::Project,
            origin: SourceOrigin::TopLevel,
            base_dir: None,
        },
        disable_model_invocation: false,
    }
}

/// Answers every prompt of `session` with an empty finished message, and configures the
/// credentials the session's own refusal check reads.
pub(super) fn answer_session(session: &Rc<AgentSession>) {
    *session.agent().stream_fn.write().unwrap() = Some(Rc::new(|_, _, _, _| {
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(answer()));
        Box::pin(std::future::ready(Ok(stream)))
    }));
}

/// Registers `name` in the loaded extension with a handler that records its own run.
fn register_command(extension: &Rc<RwLock<Extension<Local>>>, name: &str, handler: CommandHandler) {
    let source_info = Arc::clone(&extension.read().unwrap().source_info);
    let command = RegisteredCommand::<Local> {
        name: name.to_owned(),
        source_info,
        description: None,
        get_argument_completions: None,
        handler,
    };
    extension
        .write()
        .unwrap()
        .commands
        .insert(name.to_owned(), Rc::new(RwLock::new(command)));
}

/// A handler for `name` that only records its run.
fn recording_handler(captured: &Captured, name: &'static str) -> CommandHandler {
    let captured = captured.clone();
    let text = format!("/{name} ");
    Rc::new(move |arguments, _| {
        let captured = captured.clone();
        let text = format!("{text}{arguments}");
        Box::pin(async move {
            captured.record("command", &text);
            Ok(())
        })
    })
}

/// Runs `edit` on the extension while it is still loaded; the handler holds it weakly, since
/// the extension owns the handler.
fn edit_extension(
    extension: &Weak<RwLock<Extension<Local>>>,
    edit: impl FnOnce(&Rc<RwLock<Extension<Local>>>),
) {
    if let Some(loaded) = extension.upgrade() {
        edit(&loaded);
    }
}

/// The handler of `cmd`: it records its text, makes the case's edit, replaces the session
/// when the case asks, and waits or fails as the case configures.
fn command_handler(
    extension: &Rc<RwLock<Extension<Local>>>,
    world: &World,
    captured: &Captured,
) -> CommandHandler {
    let captured = captured.clone();
    let extension = Rc::downgrade(extension);
    let mode = Rc::downgrade(&world.mode);
    Rc::new(move |arguments: String, context: ExtensionCommandContext| {
        captured.record("command", &format!("/cmd {arguments}"));
        match captured.edit.get() {
            Edit::Editor => {
                if let Some(mode) = mode.upgrade() {
                    mode.editor.set_text("replaced draft");
                }
            }
            Edit::AddCommand => edit_extension(&extension, |loaded| {
                register_command(loaded, "late", recording_handler(&captured, "late"));
            }),
            Edit::RemoveCommand => edit_extension(&extension, |loaded| {
                drop(loaded.write().unwrap().commands.shift_remove("late"));
            }),
            Edit::None => {}
        }
        let captured = captured.clone();
        Box::pin(async move {
            if captured.replace.get() {
                context.new_session(None).await?;
                captured.replaced.set(true);
            }
            let wait = captured.command_release_wait();
            if let Some(wait) = wait {
                let _ = wait.await;
            }
            if captured.command_fails.get() {
                return Err("the command failed".into());
            }
            Ok(())
        })
    })
}

/// Queues one steering text in the session, so a recovery's clearing is observable, and
/// starts the recorded log after it.
fn seed_session(world: &World, captured: &Captured) {
    world
        .mode
        .runtime_host
        .session()
        .steer("earlier", &[])
        .unwrap();
    captured.log.borrow_mut().clear();
    *captured.queued.borrow_mut() = (vec!["earlier".to_owned()], Vec::new());
}

/// The delivery a message records, named as the fixture reads it.
fn delivery(mode: &UserMessageDelivery) -> &'static str {
    match mode {
        UserMessageDelivery::Steer => "steer",
        UserMessageDelivery::FollowUp => "follow-up",
    }
}

/// The messages a recorded case starts with.
fn captured(local: &[Kept]) -> Vec<CompactionQueuedMessage> {
    local
        .iter()
        .map(|kept| CompactionQueuedMessage {
            text: kept.text.clone(),
            mode: kept.mode.delivery(),
        })
        .collect()
}

/// Captures `local` for a compaction that just ended and delivers it.
fn run_flush(world: &World, local: &[Kept], will_retry: bool) {
    *world.mode.compaction_queued_messages.borrow_mut() = captured(local);
    start_compaction(world, CompactionReason::Manual);
    end_quietly(world, will_retry);
}

/// Ends the running compaction with neither a result nor a failure.
fn end_quietly(world: &World, will_retry: bool) {
    world
        .mode
        .handle_event(&AgentSessionEvent::CompactionEnd {
            reason: CompactionReason::Manual,
            result: None,
            aborted: false,
            will_retry,
            error_message: None,
        })
        .unwrap();
}

/// Hands the terminal host one round of work and lets the session's own tasks run.
async fn round(world: &World) {
    tokio::task::yield_now().await;
    let _ = world.runtime.drain_futures();
}

/// Rounds until `done` holds, failing the case when the delivery never reaches it.
async fn until(world: &World, captured: &Captured, done: impl Fn() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        round(world).await;
    }
    panic!(
        "the captured batch did not reach the recorded state: {:?}",
        captured.log.borrow()
    );
}

/// Rounds until the terminal host holds no work.
pub(super) async fn settle_host(world: &World) {
    for _ in 0..200 {
        if world.runtime.futures() == 0 {
            return;
        }
        round(world).await;
    }
    panic!("the terminal host still holds work");
}

/// Checks what one delivered batch left.
fn assert_flush(world: &World, captured: &Captured, expected: &FlushExpected) {
    assert_eq!(*captured.log.borrow(), expected.log);
    if !expected.errors.is_empty() {
        // A recovery empties the session's queues, including the harness's own seed.
        let session = world.mode.runtime_host.session();
        assert_eq!(session.get_steering_messages(), Vec::<String>::new());
        assert_eq!(session.get_follow_up_messages(), Vec::<String>::new());
    }
    let local = world.mode.compaction_queued_messages.borrow();
    let local: Vec<(String, &'static str)> = local
        .iter()
        .map(|message| (message.text.clone(), delivery(&message.mode)))
        .collect();
    let wanted: Vec<(String, &'static str)> = expected
        .local
        .iter()
        .map(|kept| (kept.text.clone(), delivery(&kept.mode.delivery())))
        .collect();
    assert_eq!(local, wanted);
    drop(local);
    let transcript = world.mode.chat_container.render(100).join("\n");
    for notice in &expected.errors {
        assert!(
            transcript.contains(notice.as_str()),
            "{transcript:?} lacks {notice:?}"
        );
    }
    if expected.errors.is_empty() {
        assert!(
            !transcript.contains("Failed to send queued message"),
            "{transcript:?} shows a failure"
        );
    }
    assert_eq!(
        world
            .mode
            .pending_messages_container
            .children()
            .borrow()
            .len(),
        expected.pending
    );
    assert_eq!(world.mode.editor.get_text(), expected.editor);
}

/// What one flush case built, held weakly so the case can check that dropping its world
/// releases all of it.
struct Released {
    /// The mode under test.
    mode: Weak<super::super::InteractiveMode>,
    /// Each session the case ran, the one the mode started with first.
    sessions: Vec<Weak<AgentSession>>,
    /// The extension that owns the case's commands.
    extension: Weak<RwLock<Extension<Local>>>,
}

impl Released {
    /// The mode, its starting session and that session's first loaded extension.
    fn of(world: &World) -> Self {
        let session = world.mode.runtime_host.session();
        Self {
            mode: Rc::downgrade(&world.mode),
            extension: Rc::downgrade(&session.resource_loader().get_extensions().extensions[0]),
            sessions: vec![Rc::downgrade(&session)],
        }
    }

    /// Also tracks the session the mode runs now, which a replacement changes.
    fn track_current(&mut self, world: &World) {
        self.sessions
            .push(Rc::downgrade(&world.mode.runtime_host.session()));
    }

    /// Fails when anything the case built is still alive.
    fn assert_gone(&self) {
        assert!(self.mode.upgrade().is_none(), "the mode outlives the case");
        assert!(
            self.extension.upgrade().is_none(),
            "the extension outlives the case"
        );
        for session in &self.sessions {
            assert!(session.upgrade().is_none(), "a session outlives the case");
        }
    }
}

/// Delivers one recorded captured batch inside a Tokio set, checks what it left and that
/// dropping the world releases the mode, its sessions and the extension.
async fn flush_case(directory: &Path, input: &FlushInput, expected: &FlushExpected) {
    let captured = Captured::default();
    if input.faults.delayed {
        captured.delay();
    }
    let (world, removal) = flush_world(directory, input, &captured).await;
    let mut released = Released::of(&world);
    world.mode.editor.set_text("draft");
    if input.faults.refuses() && !input.replace {
        seed_session(&world, &captured);
    }
    run_flush(&world, &input.local, input.will_retry);
    if input.faults.delayed {
        until(&world, &captured, || {
            captured
                .log
                .borrow()
                .iter()
                .any(|entry| entry.contains("command"))
        })
        .await;
        world
            .mode
            .compaction_queued_messages
            .borrow_mut()
            .push(CompactionQueuedMessage {
                text: "newer".to_owned(),
                mode: UserMessageDelivery::Steer,
            });
        let _ = captured.release_prompt().send(());
        // The first prompt's failure is noticed while the tail's command still waits.
        until(&world, &captured, || {
            world
                .mode
                .chat_container
                .render(100)
                .join("\n")
                .contains("Failed to send queued message")
        })
        .await;
        let _ = captured.release_command().send(());
    }
    until(&world, &captured, || world.runtime.futures() == 0).await;
    assert_flush(&world, &captured, expected);
    released.track_current(&world);
    drop((world, removal, captured));
    released.assert_gone();
}

#[test]
fn compaction_flush_selects_first_prompt_and_retains_tail_modes() {
    const TEST: &str = "compaction_flush_selects_first_prompt_and_retains_tail_modes";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_failures_restore_the_snapshot_and_clear_session_queue() {
    const TEST: &str = "compaction_flush_failures_restore_the_snapshot_and_clear_session_queue";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_failure_uses_singular_error_line() {
    const TEST: &str = "compaction_flush_failure_uses_singular_error_line";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_delayed_failure_stops_admission_and_restores_once() {
    const TEST: &str = "compaction_flush_delayed_failure_stops_admission_and_restores_once";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_real_commands_and_modes_reach_provider() {
    const TEST: &str = "compaction_flush_real_commands_and_modes_reach_provider";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_selected_commands_still_prompt_after_removal() {
    const TEST: &str = "compaction_flush_selected_commands_still_prompt_after_removal";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_selected_prefix_still_prompts_after_removal() {
    const TEST: &str = "compaction_flush_selected_prefix_still_prompts_after_removal";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

#[test]
fn compaction_flush_follows_current_session_after_command() {
    const TEST: &str = "compaction_flush_follows_current_session_after_command";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<FlushInput, FlushExpected>(FIXTURES, TEST) {
            flush_case(directory, &input, &expected).await;
        }
    });
}

/// One captured message of a native case.
fn kept(text: &str, mode: Mode) -> Kept {
    Kept {
        text: text.to_owned(),
        mode,
    }
}

/// The stream a held answer keeps open.
struct HeldAnswer(Rc<RefCell<Option<AssistantMessageEventStream>>>);

impl HeldAnswer {
    /// Ends the open response with a finished answer.
    fn end(&self) {
        let stream = self.0.borrow_mut().take().expect("a held answer");
        stream.end(Some(answer()));
    }
}

/// Replaces the provider with one whose answer the case ends itself.
fn hold_answer(world: &World) -> HeldAnswer {
    let open: Rc<RefCell<Option<AssistantMessageEventStream>>> = Rc::default();
    let first = Rc::new(Cell::new(true));
    let held = HeldAnswer(Rc::clone(&open));
    let session = world.mode.runtime_host.session();
    *session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, _, _, _| {
        let stream = AssistantMessageEventStream::new();
        if first.replace(false) {
            *open.borrow_mut() = Some(stream.clone());
        } else {
            stream.end(Some(answer()));
        }
        Box::pin(std::future::ready(Ok(stream)))
    }));
    held
}

#[test]
fn compaction_flush_capture_precedes_async_handoff() {
    const TEST: &str = "compaction_flush_capture_precedes_async_handoff";
    isolated(TEST, async |directory| {
        let captured = Captured::default();
        let input = FlushInput {
            will_retry: false,
            local: vec![kept("first", Mode::Steer), kept("tail", Mode::FollowUp)],
            faults: Faults::default(),
            edit: Edit::None,
            replace: false,
        };
        let (world, _removal) = flush_world(directory, &input, &captured).await;
        let held = hold_answer(&world);
        run_flush(&world, &input.local, false);
        // The batch is captured and its rows are gone before the host runs any of it.
        assert!(world.mode.compaction_queued_messages.borrow().is_empty());
        assert_eq!(
            world
                .mode
                .pending_messages_container
                .children()
                .borrow()
                .len(),
            0
        );
        assert!(captured.log.borrow().is_empty());
        assert_eq!(world.runtime.futures(), 1, "the delivery is handed over");
        round(&world).await;
        assert_eq!(
            *captured.log.borrow(),
            ["a prompt first", "a follow-up tail"]
        );
        // A second flush finds nothing captured and adds no work.
        end_quietly(&world, false);
        round(&world).await;
        assert_eq!(
            *captured.log.borrow(),
            ["a prompt first", "a follow-up tail"]
        );
        held.end();
        until(&world, &captured, || world.runtime.futures() == 0).await;
        assert_eq!(
            *captured.log.borrow(),
            ["a prompt first", "a follow-up tail"]
        );
    });
}

#[test]
fn compaction_flush_real_session_rejects_without_duplicate_tail() {
    const TEST: &str = "compaction_flush_real_session_rejects_without_duplicate_tail";
    isolated(TEST, async |directory| {
        let captured = Captured::default();
        let input = FlushInput {
            will_retry: false,
            local: vec![kept("first", Mode::Steer), kept("tail-f", Mode::FollowUp)],
            faults: Faults::default(),
            edit: Edit::None,
            replace: false,
        };
        let (world, _removal) = flush_world(directory, &input, &captured).await;
        world
            .host
            .services()
            .auth_storage
            .remove_runtime_api_key("unknown");
        seed_session(&world, &captured);
        run_flush(&world, &input.local, false);
        until(&world, &captured, || world.runtime.futures() == 0).await;
        // The refused prompt stops the tail, so no duplicate reaches the session queue.
        assert_eq!(*captured.log.borrow(), ["a prompt first"]);
        let session = world.mode.runtime_host.session();
        assert!(session.get_follow_up_messages().is_empty());
        assert!(session.get_steering_messages().is_empty());
        let transcript = world.mode.chat_container.render(100).join("\n");
        assert_eq!(
            transcript.matches("Failed to send queued message").count(),
            1,
            "{transcript:?}"
        );
        let local = world.mode.compaction_queued_messages.borrow();
        assert_eq!(local.len(), 2);
        assert_eq!(local[1].text, "tail-f");
    });
}

#[test]
fn compaction_flush_delivers_each_message_once() {
    const TEST: &str = "compaction_flush_delivers_each_message_once";
    isolated(TEST, async |directory| {
        for count in [32, 64, 128] {
            let captured = Captured::default();
            let local: Vec<Kept> = (0..count)
                .map(|index| kept(&format!("queued {index}"), Mode::Steer))
                .collect();
            let input = FlushInput {
                will_retry: true,
                local,
                faults: Faults::default(),
                edit: Edit::None,
                replace: false,
            };
            let (world, _removal) = flush_world(directory, &input, &captured).await;
            run_flush(&world, &input.local, true);
            until(&world, &captured, || world.runtime.futures() == 0).await;
            // Every message reaches the session once at each size. The walk's own cost is not
            // measured here: no test-owned callback runs per visited message.
            let log = captured.log.borrow().clone();
            assert_eq!(log.len(), count);
            let unique: std::collections::BTreeSet<&String> = log.iter().collect();
            assert_eq!(unique.len(), count);
        }
    });
}
