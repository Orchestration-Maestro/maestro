//! Runs of the print mode over recorded session, output and signal adapters.
use super::{
    FinalMessage, PrintEffects, PrintFuture, PrintMode, PrintModeOptions, PrintRebind,
    PrintRuntime, PrintSession, SignalPort, run_print_mode_with,
};
use maestro_app::{
    AgentSessionEvent, AgentSessionEventListener, AssistantContent, AssistantMessage,
    CommandContextOperations, DiagnosticErrorInfo, ExtensionBindings,
    ExtensionCommandContextActions, FileEntry, ImageContent, PromptOptions, SessionChangeResult,
    StopReason, TerminationSignal, TextContent, Usage, output_guard,
};
use maestro_listeners::Listeners;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    io,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot;

/// A recorded termination-signal callback.
type SignalCallback = Rc<dyn Fn(TerminationSignal)>;

/// Work a prompt runs before it returns.
type PromptHook = Rc<dyn Fn() -> PrintFuture<()>>;

/// One prompt a controlled session received.
#[derive(Clone, Debug, PartialEq)]
struct SentPrompt {
    /// The text sent.
    text: String,
    /// The images sent, complete and in order.
    images: Option<Vec<ImageContent>>,
    /// Whether the caller left the input source at its default.
    interactive_source: bool,
}

/// What a controlled run records.
#[derive(Default)]
struct Recorder {
    /// Every step, in the order the run took it.
    steps: RefCell<Vec<String>>,
    /// Standard output bytes.
    stdout: RefCell<String>,
    /// Standard error lines.
    stderr: RefCell<Vec<String>>,
    /// Each prompt the current session received.
    prompts: RefCell<Vec<SentPrompt>>,
    /// Each session the run has bound, in order.
    sessions: RefCell<Vec<Rc<TestSession>>>,
    /// The header the session holds.
    header: RefCell<Option<FileEntry>>,
    /// The final history entry.
    final_message: RefCell<Option<FinalMessage>>,
    /// The outcomes of the coming prompts; absent means success.
    prompt_results: RefCell<VecDeque<Option<String>>>,
    /// The outcomes of the coming bindings; absent means success.
    bind_results: RefCell<VecDeque<Option<String>>>,
    /// The rebind operation the runtime kept.
    rebind: RefCell<Option<PrintRebind>>,
    /// Each status the process was asked to exit with.
    exits: RefCell<Vec<i32>>,
    /// Whom to tell when the run asks the process to exit.
    exit_announced: RefCell<Option<oneshot::Sender<i32>>>,
    /// Whom to tell when a runtime disposal starts.
    dispose_announced: RefCell<Option<oneshot::Sender<()>>>,
    /// Each installed signal callback.
    signal_callbacks: RefCell<Listeners<Rc<SignalCallback>>>,
    /// The live event subscriptions, by identity.
    listeners: Rc<RefCell<Listeners<AgentSessionEventListener>>>,
    /// The pending settlement of a disposal; absent means it settles at once.
    dispose_settles: RefCell<Option<oneshot::Receiver<()>>>,
    /// The disposal failure.
    dispose_failure: RefCell<Option<String>>,
    /// Whether the flush fails.
    flush_failure: Cell<bool>,
    /// Whether the standard output write fails.
    stdout_failure: Cell<bool>,
    /// The bindings each session received.
    bindings: RefCell<Vec<ExtensionBindings>>,
    /// Whether a binding session announces an event of its own.
    announce_on_bind: Cell<bool>,
    /// The next prompt replaces the session.
    replace_on_prompt: Cell<bool>,
    /// The session a replacement installs.
    pending_replacement: RefCell<Option<Rc<TestSession>>>,
    /// Work a prompt runs before it returns.
    on_prompt: RefCell<Option<PromptHook>>,
    /// This recorder, for a session created without the test's own handle.
    me: RefCell<std::rc::Weak<Recorder>>,
}

impl Recorder {
    /// Records `step`.
    fn push(&self, step: &str) {
        self.steps.borrow_mut().push(step.to_owned());
    }

    /// Every step recorded so far.
    fn steps(&self) -> Vec<String> {
        self.steps.borrow().clone()
    }

    /// How often `step` was recorded.
    fn count(&self, step: &str) -> usize {
        self.steps
            .borrow()
            .iter()
            .filter(|entry| *entry == step)
            .count()
    }

    /// The position of `step` in the recorded order.
    fn position(&self, step: &str) -> usize {
        self.steps
            .borrow()
            .iter()
            .position(|entry| entry == step)
            .unwrap_or_else(|| panic!("missing step {step} in {:?}", self.steps()))
    }

    /// Delivers `event` to every live listener.
    fn emit(&self, event: &AgentSessionEvent) {
        let snapshot = self.listeners.borrow_mut().snapshot();
        for listener in snapshot {
            listener(event);
        }
    }

    /// Delivers `signal` to every installed signal callback.
    fn signal(&self, signal: TerminationSignal) {
        let snapshot = self.signal_callbacks.borrow_mut().snapshot();
        for callback in snapshot {
            callback(signal);
        }
    }

    /// Announces the process exit the run asks for.
    fn announce_exit(&self) -> oneshot::Receiver<i32> {
        let (sender, announced) = oneshot::channel();
        *self.exit_announced.borrow_mut() = Some(sender);
        announced
    }

    /// Announces the start of a runtime disposal.
    fn announce_disposal(&self) -> oneshot::Receiver<()> {
        let (sender, announced) = oneshot::channel();
        *self.dispose_announced.borrow_mut() = Some(sender);
        announced
    }

    /// The session now in effect.
    fn current(&self) -> Rc<TestSession> {
        Rc::clone(self.sessions.borrow().last().expect("a session"))
    }

    /// Installs the next session, which the coming prompt makes current.
    fn replacement(&self) -> Rc<dyn PrintSession> {
        let replacement = self
            .pending_replacement
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                Rc::new(TestSession {
                    recorder: self.me.borrow().upgrade().expect("the recorder"),
                    prompted: Cell::new(0),
                })
            });
        self.sessions.borrow_mut().push(Rc::clone(&replacement));
        replacement as Rc<dyn PrintSession>
    }
}

/// A session whose operations a test drives.
struct TestSession {
    /// The shared record.
    recorder: Rc<Recorder>,
    /// How many prompts this session received.
    prompted: Cell<usize>,
}

impl PrintSession for TestSession {
    fn bind_extensions(&self, bindings: ExtensionBindings) -> PrintFuture<()> {
        self.recorder.push("bind");
        let failure = self
            .recorder
            .bind_results
            .borrow_mut()
            .pop_front()
            .flatten();
        self.recorder.bindings.borrow_mut().push(bindings);
        if self.recorder.announce_on_bind.get() {
            self.recorder.emit(&AgentSessionEvent::QueueUpdate {
                steering: Vec::new(),
                follow_up: Vec::new(),
            });
        }
        Box::pin(std::future::ready(match failure {
            Some(message) => Err(DiagnosticErrorInfo::error(message)),
            None => Ok(()),
        }))
    }

    fn subscribe(&self, listener: AgentSessionEventListener) -> Box<dyn Fn()> {
        self.recorder.push("subscribe");
        let remove = Listeners::subscribe(
            &self.recorder.listeners,
            Rc::new(move |event| listener(event)),
        );
        let recorder = Rc::clone(&self.recorder);
        Box::new(move || {
            recorder.push("unsubscribe");
            remove();
        })
    }

    fn prompt(&self, text: String, options: PromptOptions) -> PrintFuture<()> {
        self.recorder.push("prompt");
        self.prompted.set(self.prompted.get() + 1);
        self.recorder.prompts.borrow_mut().push(SentPrompt {
            text,
            images: options.images,
            interactive_source: options.source.is_none(),
        });
        if self.recorder.replace_on_prompt.replace(false) {
            let rebind = self.recorder.rebind.borrow().clone().expect("a rebind");
            let replacement = self.recorder.replacement();
            return Box::pin(async move { rebind(replacement).await });
        }
        let hook = self.recorder.on_prompt.borrow().clone();
        let failure = self
            .recorder
            .prompt_results
            .borrow_mut()
            .pop_front()
            .flatten();
        Box::pin(async move {
            if let Some(hook) = hook {
                drop(hook().await);
            }
            match failure {
                Some(message) => Err(DiagnosticErrorInfo::error(message)),
                None => Ok(()),
            }
        })
    }

    fn header(&self) -> Option<FileEntry> {
        self.recorder.header.borrow().clone()
    }

    fn final_message(&self) -> Option<FinalMessage> {
        self.recorder
            .final_message
            .borrow()
            .as_ref()
            .map(FinalMessage::clone)
    }
}

/// A runtime whose operations a test drives.
struct TestRuntime {
    /// The shared record.
    recorder: Rc<Recorder>,
}

impl PrintRuntime for TestRuntime {
    fn session(&self) -> Rc<dyn PrintSession> {
        self.recorder.current() as Rc<dyn PrintSession>
    }

    fn command_actions(&self) -> ExtensionCommandContextActions {
        // The runtime supplies its own command operations; the run only binds them.
        let recorder = Rc::clone(&self.recorder);
        let uncancelled = || {
            Box::pin(std::future::ready(Ok(SessionChangeResult {
                cancelled: false,
            })))
        };
        ExtensionCommandContextActions::new(CommandContextOperations {
            wait_for_idle: Rc::new(|| Box::pin(std::future::ready(Ok(())))),
            new_session: Rc::new(move |_| uncancelled()),
            fork: Rc::new(move |_, _| uncancelled()),
            navigate_tree: Rc::new(move |_, _| uncancelled()),
            switch_session: Rc::new(move |_, _| uncancelled()),
            reload: Rc::new(move || {
                recorder.push("runtime_reload");
                Box::pin(std::future::ready(Ok(())))
            }),
        })
    }

    fn set_rebind(&self, rebind: Option<PrintRebind>) {
        self.recorder.push("set_rebind");
        *self.recorder.rebind.borrow_mut() = rebind;
    }

    fn dispose(&self) -> PrintFuture<()> {
        self.recorder.push("dispose");
        let started = self.recorder.dispose_announced.borrow_mut().take();
        if let Some(started) = started {
            let _ = started.send(());
        }
        let settlement = self.recorder.dispose_settles.borrow_mut().take();
        let failure = self.recorder.dispose_failure.borrow().clone();
        Box::pin(async move {
            if let Some(settlement) = settlement {
                drop(settlement.await);
            }
            match failure {
                Some(message) => Err(DiagnosticErrorInfo::error(message)),
                None => Ok(()),
            }
        })
    }
}

/// The signals a controlled run delivers.
struct TestSignals {
    /// The shared record.
    recorder: Rc<Recorder>,
}

impl SignalPort for TestSignals {
    fn register(&self, callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
        self.recorder.push("register_signals");
        self.recorder
            .signal_callbacks
            .borrow_mut()
            .add(Rc::new(callback));
        let recorder = Rc::clone(&self.recorder);
        Ok(Box::new(move || recorder.push("remove_signals")))
    }

    fn exit(&self, code: i32) {
        self.recorder.exits.borrow_mut().push(code);
        let announced = self.recorder.exit_announced.borrow_mut().take();
        if let Some(announced) = announced {
            let _ = announced.send(code);
        }
    }
}

/// The effects of a controlled run.
fn effects(recorder: &Rc<Recorder>) -> PrintEffects {
    PrintEffects {
        write_stdout: {
            let recorder = Rc::clone(recorder);
            Rc::new(move |text| {
                if recorder.stdout_failure.get() {
                    return Err(io::Error::other("write failed"));
                }
                recorder.stdout.borrow_mut().push_str(text);
                Ok(())
            })
        },
        write_stderr_line: {
            let recorder = Rc::clone(recorder);
            Rc::new(move |line| recorder.stderr.borrow_mut().push(line.to_owned()))
        },
        flush_stdout: {
            let recorder = Rc::clone(recorder);
            Rc::new(move || {
                recorder.push("flush");
                if recorder.flush_failure.get() {
                    return Err(io::Error::other("flush failed"));
                }
                Ok(())
            })
        },
        kill_children: {
            let recorder = Rc::clone(recorder);
            Rc::new(move || recorder.push("kill_children"))
        },
        signals: Rc::new(TestSignals {
            recorder: Rc::clone(recorder),
        }),
    }
}

/// A recorder holding one controlled session.
fn recorder() -> Rc<Recorder> {
    let recorder = Rc::new(Recorder::default());
    *recorder.me.borrow_mut() = Rc::downgrade(&recorder);
    recorder.sessions.borrow_mut().push(Rc::new(TestSession {
        recorder: Rc::clone(&recorder),
        prompted: Cell::new(0),
    }));
    recorder
}

/// Runs `options` over a controlled runtime and returns its status.
async fn run(
    recorder: &Rc<Recorder>,
    options: PrintModeOptions,
) -> Result<i32, DiagnosticErrorInfo> {
    let runtime = TestRuntime {
        recorder: Rc::clone(recorder),
    };
    run_print_mode_with(Rc::new(runtime), options, effects(recorder)).await
}

/// The options of a text run.
fn text(prompts: &[&str]) -> PrintModeOptions {
    PrintModeOptions {
        mode: PrintMode::Text,
        messages: prompts.iter().map(|prompt| (*prompt).to_owned()).collect(),
        initial_message: None,
        initial_images: None,
    }
}

/// The options of a JSON run.
fn json(prompts: &[&str]) -> PrintModeOptions {
    PrintModeOptions {
        mode: PrintMode::Json,
        ..text(prompts)
    }
}

/// An assistant response with `stop` and `blocks`.
fn assistant(stop: StopReason, error: Option<&str>, blocks: &[&str]) -> FinalMessage {
    FinalMessage::Assistant(Arc::new(RwLock::new(AssistantMessage {
        content: blocks
            .iter()
            .map(|block| AssistantContent::Text(TextContent::new((*block).to_owned())))
            .collect(),
        api: "faux".into(),
        provider: "faux".into(),
        model: "faux-1".to_owned(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: stop,
        error_message: error.map(str::to_owned),
        timestamp: 0.0,
    })))
}

/// Runs `future` on a task set, as the application does.
fn on_task_set<F: Future>(future: F) -> F::Output {
    let local = tokio::task::LocalSet::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime");
    runtime.block_on(local.run_until(future))
}

/// The options of a font image the initial prompt forwards.
fn image(data: &str) -> ImageContent {
    ImageContent {
        mime_type: "image/png".to_owned(),
        data: data.to_owned(),
    }
}

#[test]
fn initial_images_reach_input_and_quit_once() {
    on_task_set(async {
        let recorder = recorder();
        let options = PrintModeOptions {
            initial_message: Some("look".to_owned()),
            initial_images: Some(vec![image("first"), image("second")]),
            ..json(&[])
        };
        assert_eq!(run(&recorder, options).await, Ok(0));
        assert_eq!(
            *recorder.prompts.borrow(),
            vec![SentPrompt {
                text: "look".to_owned(),
                images: Some(vec![image("first"), image("second")]),
                interactive_source: true,
            }]
        );
        assert_eq!(recorder.count("dispose"), 1);
    });
}

#[test]
fn json_followups_keep_default_prompt_options_and_quit() {
    on_task_set(async {
        let recorder = recorder();
        assert_eq!(run(&recorder, json(&["second"])).await, Ok(0));
        assert_eq!(
            *recorder.prompts.borrow(),
            vec![SentPrompt {
                text: "second".to_owned(),
                images: None,
                interactive_source: true,
            }]
        );
        assert_eq!(recorder.count("dispose"), 1);
    });
}

#[test]
fn failed_assistant_reports_provider_error_and_quits() {
    on_task_set(async {
        let recorder = recorder();
        *recorder.final_message.borrow_mut() = Some(assistant(
            StopReason::Error,
            Some("provider failure"),
            &["ignored"],
        ));
        assert_eq!(run(&recorder, text(&[])).await, Ok(1));
        assert_eq!(*recorder.stderr.borrow(), vec!["provider failure"]);
        assert_eq!(recorder.stdout.borrow().as_str(), "");
        assert_eq!(recorder.count("dispose"), 1);
    });
}

#[test]
fn initial_message_presence_is_not_trimmed() {
    on_task_set(async {
        let skipped = recorder();
        assert_eq!(run(&skipped, text(&[])).await, Ok(0));
        assert!(skipped.prompts.borrow().is_empty());

        let empty = recorder();
        let options = PrintModeOptions {
            initial_message: Some(String::new()),
            initial_images: Some(Vec::new()),
            ..text(&[])
        };
        assert_eq!(run(&empty, options).await, Ok(0));
        assert!(empty.prompts.borrow().is_empty());

        let kept = recorder();
        let options = PrintModeOptions {
            initial_message: Some("\u{feff} spaced\tkeep\u{85}".to_owned()),
            initial_images: Some(Vec::new()),
            ..text(&[])
        };
        assert_eq!(run(&kept, options).await, Ok(0));
        assert_eq!(
            *kept.prompts.borrow(),
            vec![SentPrompt {
                text: "\u{feff} spaced\tkeep\u{85}".to_owned(),
                images: Some(Vec::new()),
                interactive_source: true,
            }]
        );
    });
}

#[test]
fn prompts_run_serially_and_stop_after_rejection() {
    on_task_set(async {
        let recorder = recorder();
        recorder.prompt_results.borrow_mut().push_back(None);
        recorder
            .prompt_results
            .borrow_mut()
            .push_back(Some("second failed".to_owned()));
        let options = PrintModeOptions {
            initial_message: Some("first".to_owned()),
            ..text(&["second", "third"])
        };
        assert_eq!(run(&recorder, options).await, Ok(1));
        let sent: Vec<String> = recorder
            .prompts
            .borrow()
            .iter()
            .map(|prompt| prompt.text.clone())
            .collect();
        assert_eq!(sent, vec!["first".to_owned(), "second".to_owned()]);
        assert_eq!(*recorder.stderr.borrow(), vec!["second failed"]);
        assert_eq!(recorder.count("dispose"), 1);
        assert_eq!(recorder.count("flush"), 1);
        let steps = recorder.steps();
        let first = steps
            .iter()
            .position(|step| step == "prompt")
            .expect("prompt");
        assert_eq!(steps[first + 1], "prompt");
    });
}

#[test]
fn text_uses_only_last_history_entry() {
    on_task_set(async {
        let none = recorder();
        assert_eq!(run(&none, text(&[])).await, Ok(0));
        assert_eq!(none.stdout.borrow().as_str(), "");

        let other = recorder();
        *other.final_message.borrow_mut() = Some(FinalMessage::Other);
        assert_eq!(run(&other, text(&[])).await, Ok(0));
        assert_eq!(other.stdout.borrow().as_str(), "");

        for stop in [StopReason::Stop, StopReason::Length, StopReason::ToolUse] {
            let recorder = recorder();
            *recorder.final_message.borrow_mut() = Some(assistant(stop, None, &["answer"]));
            assert_eq!(run(&recorder, text(&[])).await, Ok(0));
            assert_eq!(recorder.stdout.borrow().as_str(), "answer\n");
        }
    });
}

#[test]
fn text_blocks_preserve_bytes_and_omit_other_content() {
    on_task_set(async {
        let recorder = recorder();
        *recorder.final_message.borrow_mut() = Some(assistant(
            StopReason::Stop,
            None,
            &["", "line\nnext", " tab\tand\u{1b}[31mred\u{1b}[0m "],
        ));
        assert_eq!(run(&recorder, text(&[])).await, Ok(0));
        assert_eq!(
            recorder.stdout.borrow().as_str(),
            "\nline\nnext\n tab\tand\u{1b}[31mred\u{1b}[0m \n"
        );
    });
}

#[test]
fn terminal_failures_choose_nonempty_message_or_reason() {
    on_task_set(async {
        let cases = [
            (StopReason::Error, None, "Request error"),
            (StopReason::Error, Some(""), "Request error"),
            (StopReason::Error, Some("  "), "  "),
            (StopReason::Aborted, None, "Request aborted"),
            (StopReason::Aborted, Some(""), "Request aborted"),
            (StopReason::Aborted, Some("asked"), "asked"),
        ];
        for (stop, message, expected) in cases {
            let recorder = recorder();
            *recorder.final_message.borrow_mut() = Some(assistant(stop, message, &[]));
            assert_eq!(run(&recorder, text(&[])).await, Ok(1));
            assert_eq!(*recorder.stderr.borrow(), vec![expected.to_owned()]);
            assert_eq!(recorder.stdout.borrow().as_str(), "");
        }
    });
}

#[test]
fn json_assistant_failures_keep_zero_status() {
    on_task_set(async {
        for stop in [StopReason::Error, StopReason::Aborted] {
            let recorder = recorder();
            *recorder.final_message.borrow_mut() = Some(assistant(stop, Some("failed"), &[]));
            assert_eq!(run(&recorder, json(&[])).await, Ok(0));
            assert_eq!(*recorder.stderr.borrow(), Vec::<String>::new());
        }
    });
}

#[test]
fn json_header_precedes_binding_and_raw_events() {
    on_task_set(async {
        let present = recorder();
        *present.header.borrow_mut() = Some(json!({"type": "session", "id": "s1"}));
        present.announce_on_bind.set(true);
        assert_eq!(run(&present, json(&[])).await, Ok(0));
        assert!(present.position("bind") > present.position("set_rebind"));
        assert_eq!(
            present.stdout.borrow().as_str(),
            "{\"type\":\"session\",\"id\":\"s1\"}\n"
        );

        let absent = recorder();
        absent.announce_on_bind.set(true);
        assert_eq!(run(&absent, json(&[])).await, Ok(0));
        assert_eq!(absent.stdout.borrow().as_str(), "");

        let text_mode = recorder();
        *text_mode.header.borrow_mut() = Some(json!({"type": "session"}));
        assert_eq!(
            run(&text_mode, text(&[])).await,
            Ok(0),
            "text mode writes no header"
        );
        assert_eq!(text_mode.stdout.borrow().as_str(), "");
    });
}

#[test]
fn json_keeps_subscribed_session_event_records() {
    on_task_set(async {
        let recorder = recorder();
        let events = [
            AgentSessionEvent::QueueUpdate {
                steering: vec!["/hello now".to_owned()],
                follow_up: Vec::new(),
            },
            AgentSessionEvent::SessionInfoChanged {
                name: Some("quoted \"name\"\n".to_owned()),
            },
        ];
        let hook: PromptHook = {
            let recorder = Rc::clone(&recorder);
            Rc::new(move || {
                announce(&recorder, &events);
                Box::pin(std::future::ready(Ok(())))
            })
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        assert_eq!(run(&recorder, json(&["report"])).await, Ok(0));
        assert_eq!(
            recorder.stdout.borrow().as_str(),
            concat!(
                "{\"type\":\"queue_update\",\"steering\":[\"/hello now\"],\"followUp\":[]}\n",
                "{\"type\":\"session_info_changed\",\"name\":\"quoted \\\"name\\\"\\n\"}\n"
            ),
            "each subscribed event crosses unchanged as one compact record"
        );
        assert_eq!(*recorder.stderr.borrow(), Vec::<String>::new());
    });
}

#[test]
fn event_write_failure_reaches_the_error_path() {
    on_task_set(async {
        let recorder = recorder();
        let hook = {
            let recorder = Rc::clone(&recorder);
            Rc::new(move || -> PrintFuture<()> {
                recorder.stdout_failure.set(true);
                recorder.emit(&AgentSessionEvent::SessionInfoChanged { name: None });
                recorder.stdout_failure.set(false);
                Box::pin(std::future::ready(Ok(())))
            })
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        assert_eq!(
            run(&recorder, json(&["report"])).await,
            Ok(1),
            "a truncated event stream is not reported as success"
        );
        assert_eq!(*recorder.stderr.borrow(), vec!["write failed".to_owned()]);
        assert_eq!(recorder.count("dispose"), 1);
        assert_eq!(recorder.count("flush"), 1);
    });
}

#[test]
fn a_rejected_prompt_keeps_the_retained_event_failure() {
    on_task_set(async {
        let recorder = recorder();
        let hook = {
            let recorder = Rc::clone(&recorder);
            Rc::new(move || -> PrintFuture<()> {
                recorder.stdout_failure.set(true);
                recorder.emit(&AgentSessionEvent::SessionInfoChanged { name: None });
                recorder.stdout_failure.set(false);
                Box::pin(std::future::ready(Ok(())))
            })
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        recorder
            .prompt_results
            .borrow_mut()
            .push_back(Some("prompt failed".to_owned()));
        assert_eq!(
            run(&recorder, json(&["report"])).await,
            Ok(1),
            "a rejected prompt does not hide the retained event failure"
        );
        assert_eq!(
            *recorder.stderr.borrow(),
            vec!["write failed".to_owned(), "prompt failed".to_owned()],
            "both retained failures reach standard error"
        );
        assert_eq!(recorder.count("dispose"), 1);
        assert_eq!(recorder.count("flush"), 1);
    });
}

#[test]
fn replacement_rebinds_before_subscribing() {
    on_task_set(async {
        let recorder = recorder();
        let replacement = Rc::new(TestSession {
            recorder: Rc::clone(&recorder),
            prompted: Cell::new(0),
        });
        *recorder.pending_replacement.borrow_mut() = Some(Rc::clone(&replacement));
        recorder.replace_on_prompt.set(true);
        assert_eq!(run(&recorder, text(&["replace", "after"])).await, Ok(0));
        let steps = recorder.steps();
        let binds: Vec<usize> = steps
            .iter()
            .enumerate()
            .filter(|(_, step)| *step == "bind")
            .map(|(index, _)| index)
            .collect();
        let subscribes: Vec<usize> = steps
            .iter()
            .enumerate()
            .filter(|(_, step)| *step == "subscribe")
            .map(|(index, _)| index)
            .collect();
        let unsubscribe = steps
            .iter()
            .position(|step| step == "unsubscribe")
            .expect("unsubscribe");
        assert_eq!(
            binds.len(),
            2,
            "the run binds the startup and the replacement session"
        );
        assert!(
            binds[0] < subscribes[0],
            "the first binding precedes its subscription"
        );
        assert!(
            binds[1] < unsubscribe,
            "the replacement binds before the old one stops"
        );
        assert!(
            unsubscribe < subscribes[1],
            "the old subscription stops before the new one"
        );
        assert_eq!(
            replacement.prompted.get(),
            1,
            "the next prompt reaches the replacement"
        );
        let prompts: Vec<String> = recorder
            .prompts
            .borrow()
            .iter()
            .map(|prompt| prompt.text.clone())
            .collect();
        assert_eq!(prompts, vec!["replace".to_owned(), "after".to_owned()]);
    });
}

#[test]
fn failed_rebinding_keeps_previous_subscription_until_disposal() {
    on_task_set(async {
        let recorder = recorder();
        recorder.bind_results.borrow_mut().push_back(None);
        recorder
            .bind_results
            .borrow_mut()
            .push_back(Some("rebind failed".to_owned()));
        recorder.replace_on_prompt.set(true);
        assert_eq!(run(&recorder, text(&["replace"])).await, Ok(1));
        assert_eq!(*recorder.stderr.borrow(), vec!["rebind failed".to_owned()]);
        assert_eq!(
            recorder.count("subscribe"),
            1,
            "the failed replacement never subscribes"
        );
        assert_eq!(
            recorder.count("unsubscribe"),
            1,
            "only disposal removes the old listener"
        );
        assert!(recorder.position("bind") < recorder.position("unsubscribe"));
        assert!(recorder.position("unsubscribe") < recorder.position("dispose"));
    });
}

/// Waits for the exit status the run asked the process to take.
async fn exit_status(exited: &Rc<RefCell<Option<oneshot::Receiver<i32>>>>) -> i32 {
    let announced = exited.borrow_mut().take().expect("an exit");
    announced.await.expect("the exit status")
}

/// Signals termination, settles the pending disposal, then checks the exit it reports.
async fn terminated_exit(
    recorder: Rc<Recorder>,
    settlement: Rc<RefCell<Option<oneshot::Sender<()>>>>,
    exited: Rc<RefCell<Option<oneshot::Receiver<i32>>>>,
) -> Result<(), DiagnosticErrorInfo> {
    recorder.signal(TerminationSignal::Terminate);
    assert!(recorder.exits.borrow().is_empty(), "disposal is pending");
    drop(settlement.borrow_mut().take());
    assert_eq!(exit_status(&exited).await, 143, "the termination status");
    Ok(())
}

/// Signals termination and hangup while the first disposal is still pending.
async fn repeated_signal_exit(
    recorder: Rc<Recorder>,
    entered: Rc<RefCell<Option<oneshot::Receiver<()>>>>,
    exited: Rc<RefCell<Option<oneshot::Receiver<i32>>>>,
) -> Result<(), DiagnosticErrorInfo> {
    recorder.signal(TerminationSignal::Terminate);
    let started = entered.borrow_mut().take().expect("a disposal");
    assert_eq!(started.await, Ok(()), "the first disposal started");
    assert_eq!(recorder.count("dispose"), 1, "the first disposal started");
    assert!(recorder.exits.borrow().is_empty(), "and is still pending");
    recorder.signal(TerminationSignal::Hangup);
    assert_eq!(exit_status(&exited).await, 129, "the hangup status");
    Ok(())
}

/// Signals termination once and checks the exit a rejected disposal still reports.
async fn rejected_disposal_exit(
    recorder: Rc<Recorder>,
    exited: Rc<RefCell<Option<oneshot::Receiver<i32>>>>,
) -> Result<(), DiagnosticErrorInfo> {
    recorder.signal(TerminationSignal::Terminate);
    assert_eq!(
        exit_status(&exited).await,
        143,
        "a rejected disposal still exits"
    );
    Ok(())
}

/// Announces every event to the live subscriptions.
fn announce(recorder: &Recorder, events: &[AgentSessionEvent]) {
    for event in events {
        recorder.emit(event);
    }
}

/// Runs the reload the session's bound command operations carry.
async fn reload_bound_actions(recorder: Rc<Recorder>) -> Result<(), DiagnosticErrorInfo> {
    let actions = {
        let bindings = recorder.bindings.borrow();
        assert!(bindings[0].ui_context.is_none());
        bindings[0]
            .command_context_actions
            .clone()
            .expect("the runtime operations")
    };
    checked((actions.operations().reload)().await)
}

/// The diagnostic a command operation's failure becomes on this side of the seam.
fn checked<T>(
    result: Result<T, Box<dyn std::error::Error + Send + Sync>>,
) -> Result<T, DiagnosticErrorInfo> {
    result.map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}

#[test]
fn extension_failures_use_exact_stderr_line() {
    on_task_set(async {
        let recorder = recorder();
        let hook = {
            let recorder = Rc::clone(&recorder);
            Rc::new(move || -> PrintFuture<()> {
                let bindings = recorder.bindings.borrow();
                let on_error = bindings[0].on_error.as_ref().expect("an error listener");
                on_error(&maestro_app::ExtensionError {
                    extension_path: "/ext/one.ts".to_owned(),
                    event: "tool_call".to_owned(),
                    error: "boom".to_owned(),
                    stack: None,
                })
                .expect("a recorded failure");
                Box::pin(std::future::ready(Ok(())))
            })
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        assert_eq!(run(&recorder, text(&["report"])).await, Ok(0));
        assert_eq!(
            *recorder.stderr.borrow(),
            vec!["Extension error (/ext/one.ts): boom".to_owned()]
        );
        assert_eq!(recorder.stdout.borrow().as_str(), "");
    });
}

#[test]
fn thrown_operations_return_one_after_cleanup() {
    on_task_set(async {
        let binding = recorder();
        binding
            .bind_results
            .borrow_mut()
            .push_back(Some("bind failed".to_owned()));
        assert_eq!(run(&binding, text(&[])).await, Ok(1));
        assert_eq!(*binding.stderr.borrow(), vec!["bind failed".to_owned()]);
        assert_eq!(binding.count("dispose"), 1);
        assert_eq!(binding.count("flush"), 1);
        assert_eq!(binding.count("remove_signals"), 1);

        let prompting = recorder();
        prompting
            .prompt_results
            .borrow_mut()
            .push_back(Some("prompt failed".to_owned()));
        assert_eq!(run(&prompting, text(&["first"])).await, Ok(1));
        assert_eq!(*prompting.stderr.borrow(), vec!["prompt failed".to_owned()]);
        assert_eq!(prompting.count("dispose"), 1);
    });
}

#[test]
fn disposal_precedes_flush_and_runs_once() {
    on_task_set(async {
        let recorder = recorder();
        assert_eq!(run(&recorder, text(&[])).await, Ok(0));
        assert_eq!(recorder.count("dispose"), 1);
        assert_eq!(recorder.count("unsubscribe"), 1);
        assert!(recorder.position("unsubscribe") < recorder.position("dispose"));
        assert!(recorder.position("dispose") < recorder.position("flush"));
        assert!(recorder.exits.borrow().is_empty());
    });
}

#[test]
fn cleanup_failures_reject_instead_of_becoming_status() {
    on_task_set(async {
        let disposing = recorder();
        *disposing.dispose_failure.borrow_mut() = Some("disposal failed".to_owned());
        let error = run(&disposing, text(&[]))
            .await
            .expect_err("a disposal failure rejects");
        assert_eq!(error.message, "disposal failed");
        assert_eq!(disposing.count("flush"), 0);

        let flushing = recorder();
        flushing.flush_failure.set(true);
        let error = run(&flushing, text(&[]))
            .await
            .expect_err("a flush failure rejects");
        assert_eq!(error.message, "flush failed");

        let both = recorder();
        *both.dispose_failure.borrow_mut() = Some("disposal failed".to_owned());
        both.flush_failure.set(true);
        let error = run(&both, text(&[]))
            .await
            .expect_err("a disposal failure rejects before flushing");
        assert_eq!(error.message, "disposal failed");
        assert_eq!(both.count("flush"), 0);
    });
}

#[test]
fn native_signals_kill_children_before_disposal() {
    on_task_set(async {
        let terminating = recorder();
        let exited = Rc::new(RefCell::new(Some(terminating.announce_exit())));
        let (settle_disposal, disposal_pending) = oneshot::channel();
        *terminating.dispose_settles.borrow_mut() = Some(disposal_pending);
        let settle_disposal = Rc::new(RefCell::new(Some(settle_disposal)));
        let hook: PromptHook = {
            let (recorder, exited, settlement) = (
                Rc::clone(&terminating),
                Rc::clone(&exited),
                Rc::clone(&settle_disposal),
            );
            Rc::new(move || {
                Box::pin(terminated_exit(
                    Rc::clone(&recorder),
                    Rc::clone(&settlement),
                    Rc::clone(&exited),
                ))
            })
        };
        *terminating.on_prompt.borrow_mut() = Some(hook);
        assert_eq!(run(&terminating, text(&["signal"])).await, Ok(0));
        assert_eq!(terminating.count("kill_children"), 1);
        assert!(terminating.position("kill_children") < terminating.position("dispose"));
        assert_eq!(terminating.count("dispose"), 1);
        assert_eq!(*terminating.exits.borrow(), vec![143]);
    });
}

#[test]
fn repeated_signal_does_not_wait_for_first_disposal() {
    on_task_set(async {
        let recorder = recorder();
        let entered = Rc::new(RefCell::new(Some(recorder.announce_disposal())));
        let exited = Rc::new(RefCell::new(Some(recorder.announce_exit())));
        let (settle_disposal, disposal_pending) = oneshot::channel();
        *recorder.dispose_settles.borrow_mut() = Some(disposal_pending);
        let hook: PromptHook = {
            let (recorder, entered, exited) = (
                Rc::clone(&recorder),
                Rc::clone(&entered),
                Rc::clone(&exited),
            );
            Rc::new(move || {
                Box::pin(repeated_signal_exit(
                    Rc::clone(&recorder),
                    Rc::clone(&entered),
                    Rc::clone(&exited),
                ))
            })
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        assert_eq!(run(&recorder, text(&["signal"])).await, Ok(0));
        assert_eq!(
            recorder.count("dispose"),
            1,
            "the second signal disposes nothing twice"
        );
        assert_eq!(*recorder.exits.borrow(), vec![129]);
        drop(settle_disposal);
    });
}

#[test]
fn signal_exit_skips_normal_flush() {
    on_task_set(async {
        let recorder = recorder();
        let exited = Rc::new(RefCell::new(Some(recorder.announce_exit())));
        *recorder.dispose_failure.borrow_mut() = Some("disposal failed".to_owned());
        let hook: PromptHook = {
            let (recorder, exited) = (Rc::clone(&recorder), Rc::clone(&exited));
            Rc::new(move || {
                Box::pin(rejected_disposal_exit(
                    Rc::clone(&recorder),
                    Rc::clone(&exited),
                ))
            })
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        // A rejected signal disposal still exits with the signal status, and the signal
        // continuation never flushes: only the run's own completion does.
        assert_eq!(run(&recorder, text(&["signal"])).await, Ok(0));
        assert_eq!(*recorder.exits.borrow(), vec![143]);
        assert_eq!(
            recorder.count("flush"),
            1,
            "only the run's own completion flushes"
        );
    });
}

#[test]
fn handler_registration_and_removal_match_supported_signals() {
    on_task_set(async {
        let recorder = recorder();
        assert_eq!(run(&recorder, text(&[])).await, Ok(0));
        assert_eq!(recorder.count("register_signals"), 1);
        assert_eq!(recorder.count("remove_signals"), 1);
        assert!(recorder.position("remove_signals") < recorder.position("dispose"));
    });
    let expected = if cfg!(unix) {
        vec![TerminationSignal::Terminate, TerminationSignal::Hangup]
    } else {
        vec![TerminationSignal::Terminate]
    };
    assert_eq!(
        maestro_app::signals_for_host(),
        expected,
        "termination everywhere and a hangup off Windows"
    );
}

#[test]
fn retained_rebind_releases_the_ended_run() {
    on_task_set(async {
        let recorder = recorder();
        assert_eq!(run(&recorder, text(&["hold"])).await, Ok(0));
        let rebind = recorder.rebind.borrow().clone().expect("a kept rebind");
        let bindings = recorder.count("bind");
        let subscribed = recorder.count("subscribe");
        let replacement = recorder.replacement();
        assert_eq!(
            rebind(replacement).await,
            Ok(()),
            "the kept rebind still answers"
        );
        assert_eq!(
            recorder.count("bind"),
            bindings,
            "the ended run binds nothing"
        );
        assert_eq!(recorder.count("subscribe"), subscribed);
        assert_eq!(recorder.count("dispose"), 1);
    });
}

#[test]
fn bindings_carry_the_runtime_command_actions() {
    on_task_set(async {
        let recorder = recorder();
        let hook: PromptHook = {
            let recorder = Rc::clone(&recorder);
            Rc::new(move || Box::pin(reload_bound_actions(Rc::clone(&recorder))))
        };
        *recorder.on_prompt.borrow_mut() = Some(hook);
        assert_eq!(run(&recorder, text(&["bind"])).await, Ok(0));
        assert_eq!(
            recorder.count("runtime_reload"),
            1,
            "the bound operations are the runtime's own"
        );
    });
}

#[test]
fn raw_output_uses_application_guard() {
    let effects = PrintEffects::native();
    output_guard::take_over_stdout();
    assert!(output_guard::is_stdout_taken_over());
    (effects.write_stdout)("").expect("a raw write under takeover");
    (effects.flush_stdout)().expect("a raw flush under takeover");
    output_guard::restore_stdout();
    assert!(!output_guard::is_stdout_taken_over());
}

#[test]
fn prompt_and_text_traversals_stay_linear() {
    on_task_set(async {
        for size in [8, 16, 32] {
            let prompts_recorder = recorder();
            let prompts: Vec<String> = (0..size).map(|index| format!("prompt {index}")).collect();
            let options = text(&prompts.iter().map(String::as_str).collect::<Vec<_>>());
            assert_eq!(run(&prompts_recorder, options).await, Ok(0));
            assert_eq!(prompts_recorder.prompts.borrow().len(), size);
            assert_eq!(
                prompts_recorder.count("prompt"),
                size,
                "every prompt is visited once"
            );

            let blocks: Vec<String> = (0..size).map(|index| format!("block {index}")).collect();
            let text_recorder = recorder();
            *text_recorder.final_message.borrow_mut() = Some(assistant(
                StopReason::Stop,
                None,
                &blocks.iter().map(String::as_str).collect::<Vec<_>>(),
            ));
            assert_eq!(run(&text_recorder, text(&[])).await, Ok(0));
            let written = text_recorder.stdout.borrow().clone();
            let expected: String = blocks.iter().fold(String::new(), |mut text, block| {
                text.push_str(block);
                text.push('\n');
                text
            });
            assert_eq!(
                written, expected,
                "each text block is written once, in order"
            );
        }
    });
}

#[test]
fn print_session_fixture_keeps_subscription_semantics() {
    let recorder = recorder();
    let session = recorder.current();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let listener: AgentSessionEventListener = {
        let seen = Rc::clone(&seen);
        Rc::new(move |_| seen.borrow_mut().push(1))
    };
    let remove = session.subscribe(Rc::clone(&listener));
    let sibling = session.subscribe(listener);
    let event = AgentSessionEvent::SessionInfoChanged { name: None };
    recorder.emit(&event);
    assert_eq!(*seen.borrow(), [1, 1]);
    remove();
    remove();
    seen.borrow_mut().clear();
    recorder.emit(&event);
    assert_eq!(*seen.borrow(), [1]);
    sibling();
    print_fixture_snapshot_edits(&recorder, &seen, &event);
    print_fixture_signal_snapshot(&recorder, &seen);
}

/// Exercises edits through the session subscription route while a snapshot runs.
fn print_fixture_snapshot_edits(
    recorder: &Rc<Recorder>,
    seen: &Rc<RefCell<Vec<usize>>>,
    event: &AgentSessionEvent,
) {
    let remove_next = Rc::new(RefCell::new(None::<Box<dyn Fn()>>));
    let first: AgentSessionEventListener = {
        let (weak, seen, remove_next) = (
            Rc::downgrade(recorder),
            Rc::clone(seen),
            Rc::clone(&remove_next),
        );
        let once = Cell::new(false);
        Rc::new(move |_| {
            seen.borrow_mut().push(2);
            if !once.replace(true) {
                remove_next.borrow().as_ref().unwrap()();
                let seen = Rc::clone(&seen);
                drop(
                    weak.upgrade()
                        .unwrap()
                        .current()
                        .subscribe(Rc::new(move |_| seen.borrow_mut().push(4))),
                );
            }
        })
    };
    let remove_first = recorder.current().subscribe(first);
    let last: AgentSessionEventListener = {
        let seen = Rc::clone(seen);
        Rc::new(move |_| seen.borrow_mut().push(3))
    };
    *remove_next.borrow_mut() = Some(recorder.current().subscribe(last));
    seen.borrow_mut().clear();
    recorder.emit(event);
    assert_eq!(*seen.borrow(), [2, 3]);
    seen.borrow_mut().clear();
    recorder.emit(event);
    assert_eq!(*seen.borrow(), [2, 4]);
    remove_first();
}

/// A signal registered during delivery waits for the next snapshot; cleanup logs only.
fn print_fixture_signal_snapshot(recorder: &Rc<Recorder>, seen: &Rc<RefCell<Vec<usize>>>) {
    let signals = TestSignals {
        recorder: Rc::clone(recorder),
    };
    let callback: SignalCallback = {
        let (weak, seen) = (Rc::downgrade(recorder), Rc::clone(seen));
        let once = Cell::new(false);
        Rc::new(move |_| {
            seen.borrow_mut().push(5);
            if !once.replace(true) {
                let signals = TestSignals {
                    recorder: weak.upgrade().unwrap(),
                };
                let seen = Rc::clone(&seen);
                drop(
                    signals
                        .register(Rc::new(move |_| seen.borrow_mut().push(6)))
                        .unwrap(),
                );
            }
        })
    };
    let cleanup = signals.register(Rc::clone(&callback)).unwrap();
    let other = signals.register(callback).unwrap();
    seen.borrow_mut().clear();
    recorder.signal(TerminationSignal::Terminate);
    assert_eq!(*seen.borrow(), [5, 5]);
    cleanup();
    other();
    assert_eq!(recorder.count("remove_signals"), 2);
    seen.borrow_mut().clear();
    recorder.signal(TerminationSignal::Terminate);
    assert_eq!(*seen.borrow(), [5, 5, 6]);
}
