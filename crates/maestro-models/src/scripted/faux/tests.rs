use super::*;
use host::{Host, Work};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{
        Weak,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context as TaskContext, Poll, Wake, Waker},
};
#[derive(Default)]
struct Controlled {
    queue: Mutex<VecDeque<Arc<Task>>>,
    random: Mutex<VecDeque<f64>>,
    delays: Mutex<Vec<f64>>,
    raw_delays: Mutex<Vec<f64>>,
    stderr: Mutex<String>,
    clock: Mutex<f64>,
    owner: std::sync::OnceLock<Weak<Controlled>>,
}
struct Task {
    work: Mutex<Option<Work>>,
    host: Weak<Controlled>,
    queued: AtomicBool,
}
impl Wake for Task {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if !self.queued.swap(true, Ordering::SeqCst) {
            self.host
                .upgrade()
                .unwrap()
                .queue
                .lock()
                .unwrap()
                .push_back(self.clone());
        }
    }
}
struct Yield(bool);
impl Future for Yield {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
impl Controlled {
    fn new() -> Arc<Self> {
        let h = Arc::new(Self::default());
        h.owner.set(Arc::downgrade(&h)).unwrap();
        *h.clock.lock().unwrap() = 1234.0;
        h
    }
    fn step(&self) -> bool {
        let Some(task) = self.queue.lock().unwrap().pop_front() else {
            return false;
        };
        task.queued.store(false, Ordering::SeqCst);
        let mut work = task.work.lock().unwrap().take().unwrap();
        let waker = Waker::from(task.clone());
        let mut cx = TaskContext::from_waker(&waker);
        if work.as_mut().poll(&mut cx).is_pending() {
            *task.work.lock().unwrap() = Some(work);
        }
        true
    }
    fn drain(&self) {
        while self.step() {}
    }
    fn register(
        self: &Arc<Self>,
        options: RegisterFauxProviderOptions,
    ) -> FauxProviderRegistration {
        static ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = ID.fetch_add(1, Ordering::Relaxed);
        self.random
            .lock()
            .unwrap()
            .push_front((id + 1) as f64 / 100000.0);
        register_with_host(
            RegisterFauxProviderOptions {
                api: Some(format!("controlled-faux-{id}")),
                ..options
            },
            self.clone(),
        )
    }
}
impl Host for Controlled {
    fn clock(&self) -> f64 {
        *self.clock.lock().unwrap()
    }
    fn random(&self) -> f64 {
        self.random.lock().unwrap().pop_front().unwrap_or(0.0)
    }
    fn spawn(&self, work: Work) {
        self.queue.lock().unwrap().push_back(Arc::new(Task {
            work: Mutex::new(Some(work)),
            host: self.owner.get().unwrap().clone(),
            queued: AtomicBool::new(true),
        }));
    }
    fn microtask(&self) -> Work {
        Box::pin(Yield(false))
    }
    fn timer(&self, delay: f64) -> Work {
        self.raw_delays.lock().unwrap().push(delay);
        let delay = host::normalize_native_delay(delay, self);
        self.delays.lock().unwrap().push(delay);
        Box::pin(Yield(false))
    }
    fn stderr(&self, text: &str) {
        self.stderr.lock().unwrap().push_str(text);
    }
}
fn ctx() -> Context {
    Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    }
}
fn msg(text: &str) -> AssistantMessage {
    faux_assistant_message(FauxAssistantContent::Text(text.into()), Default::default())
}
fn poll<T>(future: impl Future<Output = T>) -> Poll<T> {
    Box::pin(future)
        .as_mut()
        .poll(&mut TaskContext::from_waker(Waker::noop()))
}
fn ready<T>(future: impl Future<Output = T>) -> T {
    match poll(future) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("observation pending"),
    }
}
fn events(stream: &AssistantMessageEventStream) -> Vec<AssistantMessageEvent> {
    let mut iter = stream.iter();
    let mut events = vec![];
    while let Some(e) = ready(iter.next()) {
        events.push(e)
    }
    events
}
#[test]
fn faux_queue_is_reserved_before_scheduling() {
    let h = Controlled::new();
    let r = h.register(Default::default());
    let counts = Arc::new(Mutex::new(vec![]));
    let seen = counts.clone();
    r.set_responses(vec![
        FauxResponseStep::Factory(Arc::new(move |_, _, state, _| {
            seen.lock().unwrap().push(state.read().unwrap().call_count);
            Box::pin(async { Ok(msg("first")) })
        })),
        FauxResponseStep::Message(msg("second")),
    ]);
    let first = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    let second = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    assert_eq!(r.state.read().unwrap().call_count, 2.0);
    assert_eq!(r.get_pending_response_count(), 0);
    assert!(counts.lock().unwrap().is_empty());
    r.set_responses(vec![FauxResponseStep::Message(msg("new"))]);
    r.append_responses(vec![FauxResponseStep::Message(msg("appended"))]);
    assert_eq!(r.get_pending_response_count(), 2);
    r.unregister();
    h.drain();
    assert_eq!(*counts.lock().unwrap(), vec![2.0]);
    assert_eq!(
        ready(first.result()).read().unwrap().content,
        vec![AssistantContent::Text(faux_text("first".into()))]
    );
    assert_eq!(
        ready(second.result()).read().unwrap().content,
        vec![AssistantContent::Text(faux_text("second".into()))]
    );
    h.random.lock().unwrap().push_back(0.5);
    let replacement = register_with_host(
        RegisterFauxProviderOptions {
            api: Some(r.api.clone()),
            ..Default::default()
        },
        h.clone(),
    );
    r.unregister();
    assert!(get_api_provider(&r.api).is_some());
    replacement.unregister();
}
#[test]
fn faux_uncaught_conversion_reports_without_settlement() {
    let h = Controlled::new();
    let r = h.register(Default::default());
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(|_, _, _, _| {
        Box::pin(async {
            let conversion = ThrownValue::StringCoercion(Arc::new(|| {
                Err(ThrownValue::Error(Box::new(Error {
                    name: "Error".into(),
                    message: "coercion failed".into(),
                    stack: None,
                    code: None,
                })))
            }));
            Err(conversion)
        })
    }))]);
    let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    h.drain();
    assert_eq!(&*h.stderr.lock().unwrap(), "Error: coercion failed\n");
    assert!(poll(s.iter().next()).is_pending());
    assert!(poll(s.result()).is_pending());
    r.append_responses(vec![FauxResponseStep::Message(msg("continues"))]);
    let next = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    h.drain();
    assert_eq!(
        ready(next.result()).read().unwrap().stop_reason,
        StopReason::Stop
    );
    r.unregister();
}
#[test]
fn faux_utf16_slicing_preserves_boundaries() {
    let h = Controlled::new();
    let r = h.register(RegisterFauxProviderOptions {
        token_size: Some(FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        ..Default::default()
    });
    let text = "abc😀def\u{feff}\u{85}";
    r.set_responses(vec![FauxResponseStep::Message(msg(text))]);
    let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    h.drain();
    let out = events(&s);
    let deltas = out
        .iter()
        .filter_map(|e| {
            if let AssistantMessageEvent::TextDelta { delta, partial, .. } = e {
                Some((delta.clone(), partial.read().unwrap().content.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(deltas[0].0, "abc�");
    assert_eq!(deltas[1].0, "�def");
    let AssistantContent::Text(t) = &deltas[1].1[0] else {
        panic!()
    };
    assert_eq!(t.text, "abc😀def");
    assert_eq!(
        ready(s.result()).read().unwrap().content,
        vec![AssistantContent::Text(faux_text(text.into()))]
    );
    assert_eq!(estimate_tokens("a😀b"), 1.0);
    assert_eq!(
        content_to_text(&[InputContent::Image(ImageContent {
            data: "a😀b".into(),
            mime_type: "image/png".into()
        })]),
        "[image:image/png:4]"
    );
    r.unregister();
}
#[test]
fn faux_chunk_range_normalizes_like_numbers() {
    let h = Controlled::new();
    let text = "abcdefghijklmnopqrstuvwxyz";
    for (min, max, random, lengths) in [
        (None, None, 0.0, vec![12, 12, 2]),
        (None, None, 1.0 - f64::EPSILON / 2.0, vec![20, 6]),
        (Some(1.1), Some(1.1), 0.0, vec![4, 4, 5, 4, 5, 4]),
        (Some(8.0), Some(1.0), 0.0, vec![4, 4, 4, 4, 4, 4, 2]),
        (Some(-1.0), Some(0.0), 0.0, vec![4, 4, 4, 4, 4, 4, 2]),
        (Some(f64::NAN), None, 0.0, vec![0]),
        (Some(f64::INFINITY), Some(f64::INFINITY), 0.0, vec![0]),
        (
            Some(f64::NEG_INFINITY),
            None,
            0.0,
            vec![4, 4, 4, 4, 4, 4, 2],
        ),
        (None, Some(1.0), 0.0, vec![4, 4, 4, 4, 4, 4, 2]),
        (Some(6.0), None, 0.0, vec![20, 6]),
    ] {
        h.random
            .lock()
            .unwrap()
            .extend(std::iter::repeat_n(random, 20));
        let r = h.register(RegisterFauxProviderOptions {
            token_size: Some(FauxTokenSize { min, max }),
            ..Default::default()
        });
        r.set_responses(vec![FauxResponseStep::Message(msg(text))]);
        let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
        h.drain();
        let out = events(&s);
        assert_eq!(
            out.iter()
                .filter_map(
                    |e| if let AssistantMessageEvent::TextDelta { delta, .. } = e {
                        Some(delta.encode_utf16().count())
                    } else {
                        None
                    }
                )
                .collect::<Vec<_>>(),
            lengths,
            "min={min:?} max={max:?}"
        );
        assert_eq!(
            ready(s.result()).read().unwrap().content,
            vec![AssistantContent::Text(faux_text(text.into()))]
        );
        r.unregister();
        h.random.lock().unwrap().clear();
    }
    let r = h.register(Default::default());
    r.set_responses(vec![FauxResponseStep::Message(msg(""))]);
    let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    h.drain();
    assert_eq!(
        events(&s)
            .iter()
            .filter(|e| matches!(e,AssistantMessageEvent::TextDelta {delta,..} if delta.is_empty()))
            .count(),
        1
    );
    r.unregister();
}
#[test]
fn faux_ids_use_clock_and_radix36() {
    let h = Controlled::new();
    h.random
        .lock()
        .unwrap()
        .extend([0.0, 0.5, 0.1, 1.0 - f64::EPSILON / 2.0]);
    for expected in [
        "tool:1234:",
        "tool:1234:i",
        "tool:1234:3lllllllllm",
        "tool:1234:zzzzzzzzzza",
    ] {
        assert_eq!(random_id("tool", h.as_ref()), expected);
    }
    h.random.lock().unwrap().extend([0.5, 0.1]);
    let r = register_with_host(Default::default(), h.clone());
    assert_eq!(r.api, "faux:1234:i");
    assert_eq!(r.source, "faux-provider:1234:3lllllllllm");
    r.unregister();
    h.random.lock().unwrap().extend([0.5]);
    let r = register_with_host(
        RegisterFauxProviderOptions {
            api: Some("".into()),
            ..Default::default()
        },
        h.clone(),
    );
    assert_eq!(r.api, "");
    assert_eq!(r.source, "faux-provider:1234:i");
    r.unregister();
}
#[test]
fn faux_scheduler_starts_without_observation() {
    let h = Controlled::new();
    let r = h.register(Default::default());
    let called = Arc::new(AtomicBool::new(false));
    let seen = called.clone();
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, _, _, _| {
            seen.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(msg("a")) })
        },
    ))]);
    let response = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
    assert!(!called.load(Ordering::SeqCst));
    h.drain();
    assert!(called.load(Ordering::SeqCst));
    assert!(poll(response.result()).is_ready());
    r.unregister();
    for rate in [
        None,
        Some(0.0),
        Some(-1.0),
        Some(f64::NAN),
        Some(f64::NEG_INFINITY),
    ] {
        let r = h.register(RegisterFauxProviderOptions {
            tokens_per_second: rate,
            ..Default::default()
        });
        r.set_responses(vec![FauxResponseStep::Message(msg(""))]);
        let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
        h.drain();
        assert!(poll(s.result()).is_ready());
        assert!(h.delays.lock().unwrap().is_empty());
        r.unregister();
    }
    let r = h.register(Default::default());
    let (send, receive) = tokio::sync::oneshot::channel();
    let receive = Arc::new(Mutex::new(Some(receive)));
    let called = Arc::new(AtomicBool::new(false));
    let seen = called.clone();
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, _, _, _| {
            seen.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(msg("pending hook")) })
        },
    ))]);
    let options = ProviderStreamOptions {
        base: StreamOptions {
            on_response: Some(Arc::new(move |_, _| {
                let receive = receive.lock().unwrap().take().unwrap();
                Box::pin(async move {
                    receive.await.unwrap();
                    Ok(())
                })
            })),
            ..Default::default()
        },
        ..Default::default()
    };
    let s = stream(r.get_model(None).unwrap(), ctx(), Some(options)).unwrap();
    h.drain();
    assert!(!called.load(Ordering::SeqCst));
    send.send(()).unwrap();
    assert!(h.step());
    assert!(called.load(Ordering::SeqCst), "factory waits for the hook");
    h.drain();
    assert!(poll(s.result()).is_ready());
    r.unregister();
    let native = || {
        let r = register_faux_provider(Default::default());
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
            move |_, _, _, _| {
                tx.send(()).unwrap();
                Box::pin(async { Ok(msg("native")) })
            },
        ))]);
        let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
        rx.recv().unwrap();
        assert_eq!(
            blocking(s.result()).read().unwrap().stop_reason,
            StopReason::Stop
        );
        r.unregister();
    };
    native();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let r = register_faux_provider(Default::default());
        r.set_responses(vec![FauxResponseStep::Message(msg("ambient"))]);
        let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
        tokio::task::yield_now().await;
        assert_eq!(
            s.result().await.read().unwrap().stop_reason,
            StopReason::Stop
        );
        r.unregister();
    });
}
fn blocking<T>(f: impl Future<Output = T>) -> T {
    struct Thread(std::thread::Thread);
    impl Wake for Thread {
        fn wake(self: Arc<Self>) {
            self.0.unpark()
        }
    }
    let w = Waker::from(Arc::new(Thread(std::thread::current())));
    let mut cx = TaskContext::from_waker(&w);
    let mut f = Box::pin(f);
    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}
#[test]
fn faux_pacing_uses_estimated_token_delays() {
    let h = Controlled::new();
    for (text, rate, delay) in [
        ("abcdefghijkl", 100.0, 30.0),
        ("", 50.0, 1.0),
        ("abcd", f64::INFINITY, 1.0),
        ("abcd", 300.0, 3.0),
    ] {
        let r = h.register(RegisterFauxProviderOptions {
            tokens_per_second: Some(rate),
            token_size: Some(FauxTokenSize {
                min: Some(3.0),
                max: Some(3.0),
            }),
            ..Default::default()
        });
        r.set_responses(vec![FauxResponseStep::Message(msg(text))]);
        let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
        h.drain();
        assert_eq!(
            h.raw_delays.lock().unwrap().pop(),
            Some(match rate {
                f64::INFINITY => 0.0,
                300.0 => 1000.0 / 300.0,
                _ =>
                    if text.is_empty() {
                        0.0
                    } else {
                        30.0
                    },
            })
        );
        assert_eq!(h.delays.lock().unwrap().pop(), Some(delay));
        assert!(h.stderr.lock().unwrap().is_empty());
        assert!(poll(s.result()).is_ready());
        r.unregister();
    }
}
#[test]
fn faux_timer_overflow_reports_exact_warning() {
    let h = Controlled::new();
    for (rate, expected) in [
        (1000.0 / 2147483648.0, "2147483648"),
        (f64::MIN_POSITIVE, "Infinity"),
    ] {
        let r = h.register(RegisterFauxProviderOptions {
            tokens_per_second: Some(rate),
            ..Default::default()
        });
        r.set_responses(vec![FauxResponseStep::Message(msg("a"))]);
        let s = stream(r.get_model(None).unwrap(), ctx(), None).unwrap();
        h.drain();
        assert_eq!(
            std::mem::take(&mut *h.stderr.lock().unwrap()),
            format!(
                "{expected} does not fit into a 32-bit signed integer.\nTimeout duration was set to 1.\n"
            )
        );
        assert_eq!(h.delays.lock().unwrap().pop(), Some(1.0));
        assert!(poll(s.result()).is_ready());
        r.unregister();
    }
}
#[test]
fn faux_abort_checks_do_not_expand() {
    let h = Controlled::new();
    let r = h.register(RegisterFauxProviderOptions {
        token_size: Some(FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        tokens_per_second: Some(50.0),
        ..Default::default()
    });
    let signal = Cancellation::new();
    signal.cancel();
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            signal: Some(signal.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let exhausted = stream(r.get_model(None).unwrap(), ctx(), Some(opts.clone())).unwrap();
    h.drain();
    let out = ready(exhausted.result());
    assert_eq!(out.read().unwrap().stop_reason, StopReason::Error);
    assert_eq!(out.read().unwrap().timestamp, 1234.0);
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(|_, _, _, _| {
        Box::pin(async { Err(ThrownValue::Undefined) })
    }))]);
    let failed = stream(r.get_model(None).unwrap(), ctx(), Some(opts)).unwrap();
    h.drain();
    assert_eq!(
        ready(failed.result()).read().unwrap().stop_reason,
        StopReason::Error
    );
    let signal = Cancellation::new();
    let (send, receive) = tokio::sync::oneshot::channel();
    let receive = Arc::new(Mutex::new(Some(receive)));
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, _, _, _| {
            let receive = receive.lock().unwrap().take().unwrap();
            Box::pin(async move {
                receive.await.unwrap();
                Ok(msg("abcdefgh"))
            })
        },
    ))]);
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            signal: Some(signal.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let s = stream(r.get_model(None).unwrap(), ctx(), Some(opts)).unwrap();
    h.drain();
    signal.cancel();
    assert!(poll(s.result()).is_pending());
    assert!(poll(s.iter().next()).is_pending());
    send.send(()).unwrap();
    h.drain();
    let result = ready(s.result());
    assert_eq!(result.read().unwrap().stop_reason, StopReason::Aborted);
    assert_eq!(result.read().unwrap().usage.output, 2.0);
    assert_eq!(result.read().unwrap().timestamp, 1234.0);
    let signal = Cancellation::new();
    r.set_responses(vec![FauxResponseStep::Message(msg("abcdefgh"))]);
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            signal: Some(signal.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let s = stream(r.get_model(None).unwrap(), ctx(), Some(opts)).unwrap();
    h.step();
    signal.cancel();
    assert!(poll(s.result()).is_pending());
    h.drain();
    let out = events(&s);
    assert_eq!(out.len(), 3);
    assert!(matches!(out[0], AssistantMessageEvent::Start { .. }));
    assert!(matches!(out[1], AssistantMessageEvent::TextStart { .. }));
    assert!(matches!(
        out[2],
        AssistantMessageEvent::Error {
            reason: StopReason::Aborted,
            ..
        }
    ));
    let signal = Cancellation::new();
    r.set_responses(vec![FauxResponseStep::Message(msg("abcd"))]);
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            signal: Some(signal.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let s = stream(r.get_model(None).unwrap(), ctx(), Some(opts)).unwrap();
    h.drain();
    signal.cancel();
    assert_eq!(
        ready(s.result()).read().unwrap().stop_reason,
        StopReason::Stop
    );
    r.unregister();
}
#[test]
fn faux_documentation_paragraphs_are_current() {
    let doc = include_str!("../../../../../docs/models/faux.md");
    assert!(doc.contains("### Faux provider for tests"));
    assert!(doc.contains("`register_faux_provider()` registers a temporary in-memory provider for tests and demos. It is opt-in and not part of the built-in provider set."));
    assert!(doc.contains("Notes:"));
    for paragraph in [
        "Responses are consumed from a queue in request start order.",
        "If the queue is empty, the faux provider returns an assistant error message with `error_message: \"No more faux responses queued\"`.",
        "Use `registration.set_responses([...])` to replace the remaining queue and `registration.append_responses([...])` to add more responses.",
        "`registration.models` exposes all registered faux models. `registration.get_model(None)` returns the first one, and `registration.get_model(Some(id))` returns a specific one.",
        "Use `faux_assistant_message(...)` for scripted assistant replies. Use `faux_text(...)`, `faux_thinking(...)`, and `faux_tool_call(...)` to build content blocks without filling in low-level fields manually.",
        "`registration.unregister()` removes the temporary provider from the global API registry.",
        "Usage is estimated at roughly 1 token per 4 characters. When `session_id` is present and `cache_retention` is not `None`, prompt cache reads and writes are simulated automatically.",
        "Tool call arguments stream incrementally via `toolcall_delta` chunks.",
        "By default, each streamed chunk yields without a pacing delay. Set `tokens_per_second` to pace chunk delivery in real time.",
        "The intended use is one deterministic scripted flow per registration. If you need independent concurrent flows, register separate faux providers.",
    ] {
        assert!(doc.contains(paragraph), "missing {paragraph}");
    }
    let h = Controlled::new();
    let registration = h.register(RegisterFauxProviderOptions {
        tokens_per_second: Some(50.0),
        ..Default::default()
    });
    let model = registration.get_model(None).unwrap();
    let mut context = Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            content: UserContent::Text("Summarize package.json and then call echo".into()),
            timestamp: 1234.0,
        })],
        tools: None,
    };
    let tool = faux_tool_call(
        "echo".into(),
        serde_json::json!({"text":"package.json"})
            .as_object()
            .unwrap()
            .clone(),
        Default::default(),
    );
    let tool_id = tool.id.clone();
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        FauxAssistantContent::Blocks(vec![
            AssistantContent::Thinking(faux_thinking(
                "Need to inspect package metadata first.".into(),
            )),
            AssistantContent::ToolCall(Arc::new(RwLock::new(tool))),
        ]),
        FauxAssistantMessageOptions {
            stop_reason: Some(StopReason::ToolUse),
            ..Default::default()
        },
    ))]);
    let first = complete(
        model.clone(),
        context.clone(),
        Some(ProviderStreamOptions {
            base: StreamOptions {
                session_id: Some("session-1".into()),
                cache_retention: Some(CacheRetention::Short),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    h.drain();
    context.messages.push(Message::Assistant(
        ready(first).unwrap().read().unwrap().clone(),
    ));
    context
        .messages
        .push(Message::ToolResult(ToolResultMessage {
            tool_call_id: tool_id,
            tool_name: "echo".into(),
            content: vec![InputContent::Text(faux_text(
                "package.json contents here".into(),
            ))],
            details: None,
            is_error: false,
            timestamp: 1234.0,
        }));
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        FauxAssistantContent::Blocks(vec![
            AssistantContent::Thinking(faux_thinking(
                "Now I can summarize the tool output.".into(),
            )),
            AssistantContent::Text(faux_text("Here is the summary.".into())),
        ]),
        Default::default(),
    ))]);
    let s = stream(model, context, None).unwrap();
    h.drain();
    let mut stdout = String::new();
    for event in events(&s) {
        stdout.push_str(
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap(),
        );
        stdout.push('\n');
    }
    let multi = h.register(RegisterFauxProviderOptions {
        models: Some(vec![
            FauxModelDefinition {
                id: "faux-fast".into(),
                name: None,
                reasoning: Some(false),
                input: None,
                cost: None,
                context_window: None,
                max_tokens: None,
            },
            FauxModelDefinition {
                id: "faux-thinker".into(),
                name: None,
                reasoning: Some(true),
                input: None,
                cost: None,
                context_window: None,
                max_tokens: None,
            },
        ]),
        ..Default::default()
    });
    stdout.push_str(&format!(
        "{}\n{}\n{}\n",
        multi.get_model(Some("faux-thinker")).unwrap().reasoning,
        registration.get_pending_response_count(),
        registration.state.read().unwrap().call_count
    ));
    assert_eq!(
        stdout,
        "start\nthinking_start\nthinking_delta\nthinking_delta\nthinking_delta\nthinking_end\ntext_start\ntext_delta\ntext_delta\ntext_end\ndone\ntrue\n0\n2\n"
    );
    registration.unregister();
    multi.unregister();
}
