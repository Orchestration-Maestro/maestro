mod support;
use maestro_models::*;
use std::sync::{Arc, atomic::Ordering};
use support::{auth::*, block_on, conformance::*};

#[test]
fn pre_cancelled_auth_never_resolves_or_dispatches() {
    let (models, fake) = fixture(vec![done()]);
    let resolver = Arc::new(Resolver::new(vec![]));
    let options = resolving(resolver.clone());
    options.cancellation.cancel();
    let events = collect(
        models.stream(model(), context(), options.clone()),
        &model(),
        73,
    );
    assert_eq!(trace(&events), vec![("error", None)]);
    assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Aborted));
    assert_eq!(
        block_on(models.complete(model(), context(), options)),
        *terminal(&events)
    );
    assert_eq!(resolver.count(), 0);
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
}

#[test]
fn blocked_auth_cancellation_wakes_drops_and_never_retries() {
    for complete in [false, true] {
        let (models, fake) = fixture(vec![done()]);
        let gate = Gate::default();
        let mut resolver = Resolver::new(vec![Ok(secret("blocked", None))]);
        resolver.gate = Some(gate.clone());
        let resolver = Arc::new(resolver);
        let options = resolving(resolver.clone());
        let wakes = Arc::new(WakeCounter::default());
        let result = if complete {
            let mut completion = Box::pin(models.complete(model(), context(), options.clone()));
            assert!(poll(completion.as_mut(), &wakes).is_pending());
            assert_eq!(resolver.count(), 1);
            options.cancellation.cancel();
            assert!(wakes.0.load(Ordering::SeqCst) > 0);
            let std::task::Poll::Ready(result) = poll(completion.as_mut(), &wakes) else {
                panic!("completion blocked")
            };
            result
        } else {
            let mut stream = models.stream(model(), context(), options.clone());
            let mut next = Box::pin(stream.next());
            assert!(poll(next.as_mut(), &wakes).is_pending());
            assert_eq!(resolver.count(), 1);
            options.cancellation.cancel();
            assert!(wakes.0.load(Ordering::SeqCst) > 0);
            let std::task::Poll::Ready(Some(event)) = poll(next.as_mut(), &wakes) else {
                panic!("read blocked")
            };
            drop(next);
            assert_eq!(trace(std::slice::from_ref(&event)), vec![("error", None)]);
            assert_eq!(block_on(stream.next()), None);
            gate.release();
            assert_eq!(block_on(stream.next()), None);
            snapshot(&event).clone()
        };
        assert_eq!(result.failure, Some(Failure::Cancelled));
        assert_eq!(result.stop_reason, Some(StopReason::Aborted));
        assert_eq!(gate.1.load(Ordering::SeqCst), 1);
        assert_eq!(resolver.count(), 1);
        assert!(fake.calls().is_empty());
        assert_eq!(fake.pending(), 1);
        gate.release();
        assert_eq!(resolver.count(), 1);
    }
}

#[test]
fn auth_readiness_cancellation_races_prevent_dispatch() {
    for failed in [false, true] {
        for creation in [false, true] {
            let (models, fake) = fixture(vec![done()]);
            let gate = Gate::default();
            let mut resolver = Resolver::new(vec![if failed {
                Err(Failure::AuthenticationFailed)
            } else {
                Ok(secret("ready", None))
            }]);
            if creation {
                resolver.cancel_on_create = true;
            } else {
                resolver.cancel_on_poll = true;
            }
            resolver.gate = Some(gate.clone());
            gate.release();
            let resolver = Arc::new(resolver);
            let events = collect(
                models.stream(model(), context(), resolving(resolver.clone())),
                &model(),
                73,
            );
            assert_eq!(trace(&events), vec![("error", None)]);
            assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
            assert!(fake.calls().is_empty());
            assert_eq!(fake.pending(), 1);
            assert_eq!(gate.1.load(Ordering::SeqCst), 1);
        }
        let (models, fake) = fixture(vec![done()]);
        let gate = Gate::default();
        let mut resolver = Resolver::new(vec![if failed {
            Err(Failure::AuthenticationFailed)
        } else {
            Ok(secret("tie", None))
        }]);
        resolver.gate = Some(gate.clone());
        let resolver = Arc::new(resolver);
        let options = resolving(resolver.clone());
        let wakes = Arc::new(WakeCounter::default());
        let mut stream = models.stream(model(), context(), options.clone());
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
        drop(next);
        assert_eq!(block_on(stream.next()), None);
        assert!(fake.calls().is_empty());
        assert_eq!(gate.1.load(Ordering::SeqCst), 1);
    }
    for capability in [false, true] {
        for resolved in [false, true] {
            let resolver = Arc::new(Resolver::new(vec![]));
            let options = if resolved {
                resolving(resolver.clone())
            } else {
                local()
            };
            let mut adapter = Adapter::default();
            if capability {
                adapter.cancel_capability = Some(options.cancellation.clone());
            } else {
                adapter.cancel_description = Some(options.cancellation.clone());
            }
            let adapter = Arc::new(adapter);
            let models = registry(adapter.clone());
            let events = collect(
                models.stream(model(), context(), options.clone()),
                &model(),
                73,
            );
            assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
            assert_eq!(trace(&events), vec![("error", None)]);
            assert!(adapter.calls.lock().unwrap().is_empty());
            assert_eq!(resolver.count(), 0);
        }
    }
    let (models, _) = fixture(vec![done()]);
    let options = local();
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
}

#[test]
fn adapter_setup_cancellation_aborts_without_reading_source() {
    use std::{future::Future, pin::Pin, sync::atomic::AtomicUsize};

    struct CancellingProvider(Arc<AtomicUsize>);
    impl Provider for CancellingProvider {
        fn supports(&self, operation: &str) -> bool {
            operation == "chat"
        }
        fn stream(
            &self,
            _: Model,
            _: Context,
            options: ProviderOptions,
        ) -> Result<Box<dyn ProviderStream>, Failure> {
            options.cancellation.cancel();
            Ok(Box::new(CountingSource(self.0.clone())))
        }
    }
    struct CountingSource(Arc<AtomicUsize>);
    impl ProviderStream for CountingSource {
        fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Some(done()) })
        }
    }

    let reads = Arc::new(AtomicUsize::new(0));
    let models = registry(Arc::new(CancellingProvider(reads.clone())));
    let resolver = Arc::new(Resolver::new(vec![Ok(secret("ready", None))]));
    let events = collect(
        models.stream(model(), context(), resolving(resolver)),
        &model(),
        73,
    );
    assert_eq!(trace(&events), vec![("error", None)]);
    assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Aborted));
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn dropping_pending_read_keeps_one_resolution_and_dropping_stream_releases_it() {
    let (models, fake) = fixture(vec![done()]);
    let gate = Gate::default();
    let mut resolver = Resolver::new(vec![Ok(secret("once", None))]);
    resolver.gate = Some(gate.clone());
    let resolver = Arc::new(resolver);
    let mut stream = models.stream(model(), context(), resolving(resolver.clone()));
    let wakes = Arc::new(WakeCounter::default());
    assert_eq!(resolver.count(), 0);
    for _ in 0..2 {
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
    }
    assert_eq!(resolver.count(), 1);
    assert!(fake.calls().is_empty());
    assert_eq!(gate.1.load(Ordering::SeqCst), 0);
    gate.release();
    let mut events = vec![block_on(stream.next()).unwrap()];
    assert_eq!(Arc::strong_count(&resolver), 1);
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    assert_eq!(trace(&events), vec![("start", None), ("done", None)]);
    assert_contract(&events, &model(), 73);
    assert_eq!(fake.calls().len(), 1);
    assert_eq!(resolver.count(), 1);
    assert_eq!(gate.1.load(Ordering::SeqCst), 1);
    let (models, fake) = fixture(vec![done()]);
    let gate = Gate::default();
    let mut resolver = Resolver::new(vec![Ok(secret("drop", None))]);
    resolver.gate = Some(gate.clone());
    let resolver = Arc::new(resolver);
    let weak = Arc::downgrade(&resolver);
    let mut stream = models.stream(model(), context(), resolving(resolver.clone()));
    {
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
    }
    assert_eq!(resolver.count(), 1);
    drop(resolver);
    assert!(weak.upgrade().is_some());
    drop(stream);
    assert_eq!(gate.1.load(Ordering::SeqCst), 1);
    assert!(weak.upgrade().is_none());
    gate.release();
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
}
