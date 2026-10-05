mod support;
use maestro_models::*;
use std::sync::Arc;
use support::{block_on, conformance::*};

#[test]
fn queued_scripts_are_fifo_and_exhaustion_never_replays() {
    let fake = Arc::new(ScriptedProvider::new(vec![
        steps({
            let mut u = text(0, "first");
            u.push(done());
            u
        }),
        steps({
            let mut u = text(0, "second");
            u.push(done());
            u
        }),
        Script::SetupFailure(Failure::AdapterFailed),
    ]));
    let models = registry(fake.clone());
    assert_eq!(fake.pending(), 3);
    let mut request = context();
    let first = models.stream(model(), request.clone(), support::auth::local());
    assert_eq!(fake.pending(), 2);
    if let Message::User(user) = &mut request.messages[0] {
        user.content = vec![InputContent::Text(TextContent {
            text: "changed".into(),
            replay_metadata: None,
        })];
    }
    let mut calls = fake.calls();
    if let Message::User(user) = &mut calls[0].context.messages[0] {
        user.content = vec![InputContent::Text(TextContent {
            text: "clone changed".into(),
            replay_metadata: None,
        })];
    }
    assert_eq!(fake.calls()[0].context, context());
    assert_eq!(
        terminal(&collect(first, &model(), 73)).content[0],
        AssistantContent::Text(TextContent {
            text: "first".into(),
            replay_metadata: None
        })
    );
    let second = block_on(models.complete(model(), context(), support::auth::local()));
    assert_eq!(
        second.content[0],
        AssistantContent::Text(TextContent {
            text: "second".into(),
            replay_metadata: None
        })
    );
    assert_eq!(fake.pending(), 1);
    for failure in [
        Failure::AdapterFailed,
        Failure::ScriptExhausted,
        Failure::ScriptExhausted,
    ] {
        let events = collect(
            models.stream(model(), context(), support::auth::local()),
            &model(),
            73,
        );
        assert_eq!(trace(&events), vec![("error", None)]);
        assert_eq!(terminal(&events).failure, Some(failure));
        assert!(terminal(&events).content.is_empty());
    }
    assert_eq!(fake.pending(), 0);
    assert_eq!(fake.calls().len(), 5);
    assert_eq!(
        fake.calls()
            .iter()
            .map(|call| call.call_index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn factories_inspect_owned_requests_without_holding_queue_locks() {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let holder = Arc::new(Mutex::new(None::<std::sync::Weak<ScriptedProvider>>));
    let seen = Arc::new(AtomicUsize::new(0));
    let (inside, count) = (holder.clone(), seen.clone());
    let options = support::auth::local();
    let cancellation = options.cancellation.clone();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        move |call| {
            count.fetch_add(1, Ordering::SeqCst);
            let fake = inside.lock().unwrap().as_ref().unwrap().upgrade().unwrap();
            assert_eq!(fake.calls().len(), 1);
            assert_eq!(fake.pending(), 0);
            assert_eq!(call.model, model());
            assert_eq!(call.context, context());
            assert_eq!(call.call_index, 1);
            assert!(!call.options.cancellation.is_cancelled());
            assert!(!cancellation.is_cancelled());
            Box::pin(async move {
                let mut updates = text(0, "factory");
                updates.extend([ProviderUpdate::Usage { usage: usage() }, done()]);
                Ok(updates.into_iter().map(ScriptStep::Update).collect())
            })
        },
    ))]));
    *holder.lock().unwrap() = Some(Arc::downgrade(&fake));
    let models = registry(fake.clone());
    let mut request = context();
    let stream = models.stream(model(), request.clone(), options.clone());
    request.messages.clear();
    assert_eq!(seen.load(Ordering::SeqCst), 0);
    let events = collect(stream, &model(), 73);
    assert_eq!(seen.load(Ordering::SeqCst), 1);
    assert_eq!(
        terminal(&events).content[0],
        AssistantContent::Text(TextContent {
            text: "factory".into(),
            replay_metadata: None
        })
    );
    assert_eq!(terminal(&events).usage, usage());
    assert_eq!(fake.calls()[0].context, context());
    fake.calls()[0].options.cancellation.cancel();
    assert!(options.cancellation.is_cancelled());
}

#[test]
fn factory_failure_is_a_secret_safe_prestart_error() {
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        |call| {
            Box::pin(async move {
                assert_eq!(
                    call.context.system_prompt.as_deref(),
                    Some("synthetic secret")
                );
                Err(Failure::AdapterFailed)
            })
        },
    ))]));
    let models = registry(fake);
    let events = collect(
        models.stream(model(), context(), support::auth::local()),
        &model(),
        73,
    );
    assert_eq!(trace(&events), vec![("error", None)]);
    let failure = terminal(&events).failure.unwrap();
    assert_eq!(failure.to_string(), "provider adapter failed");
    assert!(!failure.to_string().contains("synthetic secret"));
}

#[test]
fn controlled_script_waits_preserve_steps_across_polls() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let gate = Gate::default();
    let wait = gate.wait();
    let count = Arc::new(AtomicUsize::new(0));
    let inside = count.clone();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        move |_| {
            inside.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(vec![
                    ScriptStep::Wait(wait),
                    ScriptStep::Update(ProviderUpdate::TextStart { content_index: 0 }),
                    ScriptStep::Update(ProviderUpdate::TextDelta {
                        content_index: 0,
                        delta: "once".into(),
                    }),
                    ScriptStep::Update(ProviderUpdate::TextEnd {
                        content_index: 0,
                        replay_metadata: None,
                    }),
                    ScriptStep::Update(done()),
                ])
            })
        },
    ))]));
    let models = registry(fake.clone());
    let mut stream = models.stream(model(), context(), support::auth::local());
    let wakes = Arc::new(WakeCounter::default());
    {
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
        assert!(poll(next.as_mut(), &wakes).is_pending());
    }
    {
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(gate.1.load(Ordering::SeqCst), 0);
    gate.release();
    let events = collect(stream, &model(), 73);
    assert_eq!(
        trace(&events),
        vec![
            ("start", None),
            ("text_start", Some(0)),
            ("text_delta", Some(0)),
            ("text_end", Some(0)),
            ("done", None)
        ]
    );
    assert_eq!(
        terminal(&events).content[0],
        AssistantContent::Text(TextContent {
            text: "once".into(),
            replay_metadata: None
        })
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(gate.1.load(Ordering::SeqCst), 1);
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn stream_and_completion_share_the_same_conformance_assertions() {
    use std::sync::atomic::Ordering;
    fn caller(models: &Models) -> AssistantMessage {
        let events = collect(
            models.stream(model(), context(), support::auth::local()),
            &model(),
            73,
        );
        let result = block_on(models.complete(model(), context(), support::auth::local()));
        assert_eq!(&result, terminal(&events));
        assert_eq!(
            trace(&events),
            vec![
                ("start", None),
                ("text_start", Some(0)),
                ("text_delta", Some(0)),
                ("text_end", Some(0)),
                ("done", None)
            ]
        );
        result
    }
    let mut updates = text(0, "same");
    updates.extend([ProviderUpdate::Usage { usage: usage() }, done()]);
    let fake = Arc::new(ScriptedProvider::new(vec![
        steps(updates.clone()),
        steps(updates.clone()),
    ]));
    let fake_result = caller(&registry(fake.clone()));
    assert_eq!(fake.calls().len(), 2);
    let direct = Arc::new(DirectProvider::new(updates));
    assert_eq!(caller(&registry(direct.clone())), fake_result);
    assert_eq!(direct.calls.load(Ordering::SeqCst), 2);
    assert_eq!(fake_result.timestamp, 73);
}
