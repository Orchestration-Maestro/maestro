//! Compaction and retry progress through the mode's own interface: the labels, the
//! completion rows, the countdown, the interrupt callbacks and the captured queue.

use super::{World, cases, isolated, painted, started};
use maestro_app::agent_session::CompactionReason;
use maestro_app::messages::CustomAgentMessages;
use maestro_app::{AgentSessionEvent, CompactionResult};
use maestro_tui::Component;
use maestro_tui::tui::InputHandler;
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
    time::Duration,
};

/// The recorded progress cases every witness drives.
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
    /// Asserts the presentation and returns the progress requests it recorded.
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
    /// Text the transcript must contain.
    #[serde(rename = "chatContains")]
    chat_contains: Option<String>,
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

/// Starts one compaction, runs `during` before its end, and ends it as aborted.
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
    isolated(TEST, |directory| {
        for (input, expected) in cases::<CompactionInput, Value>(FIXTURES, TEST) {
            label_case(directory, input, expected);
        }
    });
}

/// Drives one recorded label case.
fn label_case(directory: &Path, input: CompactionInput, expected: Value) {
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
            start_end_case(directory, &reason, &flags, &expected);
        }
        CompactionInput::ConfiguredCancelHint { keys } => {
            let expected: StartEnd = serde_json::from_value(expected).unwrap();
            let flags = Flags {
                progress: false,
                previous: true,
                keys: Some(keys),
            };
            start_end_case(directory, "manual", &flags, &expected);
            bind_interrupt(&[], true);
        }
        CompactionInput::LiveProgressSetting { before, after } => {
            let expected: LiveProgress = serde_json::from_value(expected).unwrap();
            live_progress_case(directory, (before, after), &expected);
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
fn start_end_case(directory: &Path, reason_name: &str, flags: &Flags, expected: &StartEnd) {
    let world = started(directory);
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
fn live_progress_case(directory: &Path, settings: (bool, bool), expected: &LiveProgress) {
    let (before, after) = settings;
    let world = started(directory);
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

#[test]
fn compaction_completion_replays_then_appends_summary() {
    const TEST: &str = "compaction_completion_replays_then_appends_summary";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<CompletionInput, Outcome>(FIXTURES, TEST) {
            completion_case(directory, input, &expected);
        }
    });
}

/// Drives one recorded compaction end and asserts what it left.
fn completion_case(directory: &Path, input: CompletionInput, expected: &Outcome) {
    let world = started(directory);
    draft(&world);
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
    let rows = world.mode.chat_container.render(100);
    match &expected.chat_contains {
        Some(text) => {
            let joined = rows.join("\n");
            assert!(joined.contains(text.as_str()), "{joined:?} lacks {text:?}");
        }
        None => assert_eq!(rows, painted(&theme, &expected.chat)),
    }
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
fn retry_countdown_uses_ceiling_and_live_cancel_keys() {
    const TEST: &str = "retry_countdown_uses_ceiling_and_live_cancel_keys";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<RetryInput, Value>(FIXTURES, TEST) {
            match input {
                RetryInput::RetryCountdown { delay_ms, success } => {
                    let expected: Countdown = serde_json::from_value(expected).unwrap();
                    countdown_case(directory, delay_ms, success, &expected);
                }
                RetryInput::RetryFractionalLimit {
                    max_attempts,
                    delay_ms,
                } => {
                    let expected: RetryStartEnd = serde_json::from_value(expected).unwrap();
                    let world = started(directory);
                    draft(&world);
                    let effects = tap(&world);
                    start_retry(&world, 1, max_attempts, delay_ms);
                    expected.start.assert(&world, &effects);
                    end_retry(&world, true, 1, None);
                    expected.end.assert(&world, &effects);
                }
            }
        }
    });
}

/// Shows the initial frame and one per second tick, then ends the retry.
fn countdown_case(directory: &Path, delay_ms: f64, success: bool, recorded: &Countdown) {
    let world = started(directory);
    draft(&world);
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
    end_retry(&world, success, 2, Some("last failure"));
    recorded.end.assert(&world, &effects);
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
fn retry_completion_disposes_progress_and_selects_failure_text() {
    const TEST: &str = "retry_completion_disposes_progress_and_selects_failure_text";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<RetryEndInput, Snapshot>(FIXTURES, TEST) {
            let RetryEndInput::RetryEnd {
                success,
                final_error,
            } = input;
            let world = started(directory);
            draft(&world);
            let effects = tap(&world);
            end_retry(&world, success, 3, final_error.as_deref());
            expected.assert(&world, &effects);
            assert!(world.mode.status_container.render(100).is_empty());
        }
    });
}

#[test]
fn response_progress_pairs_each_start_with_an_end() {
    isolated(
        "response_progress_pairs_each_start_with_an_end",
        |directory| {
            let world = started(directory);
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

/// The three presentations the replacement case compares.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    /// The retry after its first tick with the keys rebound.
    changed: Snapshot,
    /// The second retry's first presentation.
    after: Snapshot,
    /// The presentation after the second retry ends.
    end: Snapshot,
}

#[test]
fn progress_replacement_stops_old_countdown_and_restores_escape() {
    const TEST: &str = "progress_replacement_stops_old_countdown_and_restores_escape";
    isolated(TEST, |directory| {
        let (input, expected) = cases::<ReplaceInput, Replacement>(FIXTURES, TEST).remove(0);
        let ReplaceInput::RetryReplaceAndLiveKeys = input;
        let world = started(directory);
        draft(&world);
        let effects = tap(&world);
        start_retry(&world, 1, 4.0, 2500.0);
        let first_tick = world.runtime.millis() + 1000;
        bind_interrupt(&["ctrl+q".to_owned(), "alt+x".to_owned()], false);
        let _ = world.runtime.advance_to(Duration::from_millis(first_tick));
        expected.changed.assert_animated(&world, &effects);
        // A response starts: the retry's countdown and row go before the working row does.
        agent_event(&world, maestro_app::AgentEvent::AgentStart);
        assert_eq!(timers(&world), [80]);
        let status = world.mode.status_container.render(100);
        assert_eq!(status.len(), 2);
        assert!(status[0].is_empty());
        assert!(status[1].contains("Working..."));
        assert!(!status[1].contains("Retrying"));
        start_retry(&world, 2, 4.0, 1000.0);
        expected.after.assert_animated(&world, &effects);
        end_retry(&world, true, 2, None);
        expected.end.assert(&world, &effects);
        bind_interrupt(&[], true);
    });
}

#[test]
fn compaction_escape_restores_main_action_and_admits_editor_input() {
    isolated(
        "compaction_escape_restores_main_action_and_admits_editor_input",
        |directory| {
            let world = started(directory);
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
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let tasks = tokio::task::LocalSet::new();
            drop(tasks.block_on(&runtime, session.compact(None)));
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
        |directory| {
            let world = started(directory);
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
