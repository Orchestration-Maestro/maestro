mod support;
use maestro_models::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use support::{block_on, conformance::*};

#[test]
fn pre_dispatch_cancellation_aborts_without_consuming_a_script() {
    let (models, fake) = fixture(vec![done()]);
    let options = support::auth::local();
    options.cancellation.cancel();
    let events = collect(
        models.stream(model(), context(), options.clone()),
        &model(),
        73,
    );
    assert_eq!(trace(&events), vec![("error", None)]);
    assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Aborted));
    let result = block_on(models.complete(model(), context(), options));
    assert_eq!(result, *terminal(&events));
    assert_eq!(fake.calls().len(), 0);
    assert_eq!(fake.pending(), 1);
    let result = block_on(models.complete(model(), context(), support::auth::local()));
    assert_eq!(result.stop_reason, Some(StopReason::Stop));
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn capability_check_cancellation_aborts_without_dispatching_or_consuming_a_script() {
    struct CancellingProvider {
        cancellation: Cancellation,
        scripted: Arc<ScriptedProvider>,
    }

    impl Provider for CancellingProvider {
        fn supports(&self, operation: &str) -> bool {
            self.cancellation.cancel();
            self.scripted.supports(operation)
        }

        fn stream(
            &self,
            model: Model,
            context: Context,
            options: ProviderOptions,
        ) -> Result<Box<dyn ProviderStream>, Failure> {
            self.scripted.stream(model, context, options)
        }
    }

    let options = support::auth::local();
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let models = registry(Arc::new(CancellingProvider {
        cancellation: options.cancellation.clone(),
        scripted: fake.clone(),
    }));
    let events = collect(models.stream(model(), context(), options), &model(), 73);
    assert_eq!(trace(&events), vec![("error", None)]);
    assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Aborted));
    assert_eq!(fake.calls().len(), 0);
    assert_eq!(fake.pending(), 1);
}

#[test]
fn blocked_read_cancellation_wakes_and_drops_without_retry() {
    for partial in [
        vec![],
        vec![
            ProviderUpdate::TextStart { content_index: 0 },
            ProviderUpdate::TextDelta {
                content_index: 0,
                delta: "partial".into(),
            },
        ],
        vec![
            ProviderUpdate::ThinkingStart {
                content_index: 0,
                signature: None,
            },
            ProviderUpdate::ThinkingDelta {
                content_index: 0,
                delta: "reason".into(),
            },
        ],
        vec![
            tool_start(0),
            ProviderUpdate::ToolCallDelta {
                content_index: 0,
                delta: "{\"x\":".into(),
            },
        ],
    ] {
        let gate = Gate::default();
        let mut actions = vec![ScriptStep::Update(ProviderUpdate::Usage {
            usage: accounting_report(),
        })];
        actions.extend(partial.iter().cloned().map(ScriptStep::Update));
        actions.push(ScriptStep::Wait(gate.wait()));
        actions.push(ScriptStep::Update(done()));
        let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(actions)]));
        let models = priced_registry(fake.clone());
        let options = support::auth::local();
        let mut stream = models.stream(model(), context(), options.clone());
        let mut events = Vec::new();
        if !partial.is_empty() {
            for _ in 0..3 {
                events.push(block_on(stream.next()).unwrap());
            }
        }
        let before = events
            .last()
            .map(|event| snapshot(event).content.clone())
            .unwrap_or_default();
        let wakes = Arc::new(WakeCounter::default());
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
        options.cancellation.cancel();
        assert!(wakes.0.load(Ordering::SeqCst) > 0);
        let std::task::Poll::Ready(Some(event)) = poll(next.as_mut(), &wakes) else {
            panic!("cancellation did not unblock read")
        };
        drop(next);
        events.push(event);
        assert_contract(&events, &model(), 73);
        assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
        assert_eq!(terminal(&events).content, before);
        assert_accounting(&terminal(&events).usage, &expected_accounting());
        if let Some(AssistantContent::ToolCall(call)) = before.first() {
            assert!(call.arguments().is_none());
        }
        assert_eq!(gate.1.load(Ordering::SeqCst), 1);
        assert_eq!(fake.calls().len(), 1);
        assert_eq!(block_on(stream.next()), None);
    }
    let gate = Gate::default();
    let wait = gate.wait();
    let count = Arc::new(AtomicUsize::new(0));
    let inside = count.clone();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        move |_| {
            inside.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                wait.await;
                Ok(vec![ScriptStep::Update(done())])
            })
        },
    ))]));
    let models = registry(fake.clone());
    let options = support::auth::local();
    let mut stream = models.stream(model(), context(), options.clone());
    let wakes = Arc::new(WakeCounter::default());
    {
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
    }
    let mut next = Box::pin(stream.next());
    assert!(poll(next.as_mut(), &wakes).is_pending());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    options.cancellation.cancel();
    assert!(wakes.0.load(Ordering::SeqCst) > 0);
    let std::task::Poll::Ready(Some(event)) = poll(next.as_mut(), &wakes) else {
        panic!("factory remained pending")
    };
    drop(next);
    assert_eq!(trace(&[event]), vec![("error", None)]);
    assert_eq!(gate.1.load(Ordering::SeqCst), 1);
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn cancellation_wins_ready_read_and_pending_terminal_races() {
    for first in [done(), ProviderUpdate::TextStart { content_index: 0 }] {
        let options = support::auth::local();
        let mut direct = DirectProvider::new(vec![first]);
        direct.cancel_on_read = Some(options.cancellation.clone());
        let direct = Arc::new(direct);
        let models = registry(direct.clone());
        let events = collect(models.stream(model(), context(), options), &model(), 73);
        assert_eq!(trace(&events), vec![("error", None)]);
        assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
        assert_eq!(direct.polls.load(Ordering::SeqCst), 1);
        assert_eq!(direct.drops.load(Ordering::SeqCst), 1);
    }
    for updates in [
        vec![done()],
        vec![
            ProviderUpdate::RedactedThinking {
                content_index: 0,
                data: "opaque".into(),
            },
            done(),
        ],
    ] {
        let (models, _) = fixture(updates.clone());
        let options = support::auth::local();
        let mut stream = models.stream(model(), context(), options.clone());
        let mut events = vec![block_on(stream.next()).unwrap()];
        if updates.len() == 2 {
            events.push(block_on(stream.next()).unwrap());
        }
        options.cancellation.cancel();
        events.push(block_on(stream.next()).unwrap());
        assert_contract(&events, &model(), 73);
        assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
        assert_eq!(block_on(stream.next()), None);
        if updates.len() == 2 {
            assert_eq!(
                trace(&events),
                vec![
                    ("start", None),
                    ("thinking_start", Some(0)),
                    ("error", None)
                ]
            );
        }
    }
    let (models, _) = fixture(vec![done()]);
    let options = support::auth::local();
    let mut stream = models.stream(model(), context(), options.clone());
    assert!(matches!(
        block_on(stream.next()),
        Some(ModelEvent::Start { .. })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(ModelEvent::Done { .. })
    ));
    options.cancellation.cancel();
    assert_eq!(block_on(stream.next()), None);
    let gate = Gate::default();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(vec![
        ScriptStep::Wait(gate.wait()),
        ScriptStep::Update(done()),
    ])]));
    let models = registry(fake);
    let options = support::auth::local();
    let mut stream = models.stream(model(), context(), options.clone());
    let wakes = Arc::new(WakeCounter::default());
    let mut next = Box::pin(stream.next());
    assert!(poll(next.as_mut(), &wakes).is_pending());
    gate.release();
    options.cancellation.cancel();
    assert!(matches!(
        poll(next.as_mut(), &wakes),
        std::task::Poll::Ready(Some(ModelEvent::Error {
            reason: StopReason::Aborted,
            ..
        }))
    ));
}

#[test]
fn cancellation_waiters_are_idempotent_and_request_local() {
    let cancellation = Cancellation::new();
    let clone = cancellation.clone();
    let first = Arc::new(WakeCounter::default());
    let second = Arc::new(WakeCounter::default());
    let stale = Arc::new(WakeCounter::default());
    let dropped = Arc::new(WakeCounter::default());
    let mut one = Box::pin(cancellation.cancelled());
    let mut two = Box::pin(clone.cancelled());
    assert!(poll(one.as_mut(), &stale).is_pending());
    assert!(poll(one.as_mut(), &first).is_pending());
    assert!(poll(one.as_mut(), &first).is_pending());
    assert!(poll(two.as_mut(), &second).is_pending());
    {
        let mut abandoned = Box::pin(cancellation.cancelled());
        assert!(poll(abandoned.as_mut(), &dropped).is_pending());
    }
    clone.cancel();
    clone.cancel();
    cancellation.cancel();
    assert_eq!(first.0.load(Ordering::SeqCst), 1);
    assert_eq!(second.0.load(Ordering::SeqCst), 1);
    assert_eq!(stale.0.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.0.load(Ordering::SeqCst), 0);
    assert!(poll(one.as_mut(), &first).is_ready());
    assert!(poll(two.as_mut(), &second).is_ready());
    let mut after = Box::pin(cancellation.cancelled());
    assert!(poll(after.as_mut(), &first).is_ready());
    assert!(cancellation.is_cancelled());
    let options = StreamOptions::default();
    let independent = StreamOptions::default();
    let shared = options.clone();
    shared.cancellation.cancel();
    assert!(options.cancellation.is_cancelled());
    assert!(!independent.cancellation.is_cancelled());
    assert!(!Cancellation::default().is_cancelled());
}
