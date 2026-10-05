mod support;

use ThinkingLevel::{High, Low, Medium, Minimal, Off, Xhigh};
use maestro_models::*;
use std::sync::Arc;
use support::{block_on, conformance::*};

fn reasoning() -> Model {
    let mut model = model();
    model.capabilities.reasoning = true;
    model
}

fn observe(model: Model, mut options: StreamOptions) -> ProviderOptions {
    options.auth = support::auth::local().auth;
    let factory_options = Arc::new(std::sync::Mutex::new(None));
    let captured = factory_options.clone();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        move |call| {
            *captured.lock().unwrap() = Some(call.options);
            Box::pin(async { Ok(vec![ScriptStep::Update(done())]) })
        },
    ))]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake.clone()).unwrap();
    let resolved = models.resolve_options(&model, options.clone()).unwrap();
    let events = collect(models.stream(model.clone(), context(), options), &model, 73);
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Stop));
    assert_eq!(fake.calls().len(), 1);
    let observed = fake.calls()[0].options.clone();
    assert_options(&resolved, &effective(&observed));
    assert_options(
        &resolved,
        &effective(factory_options.lock().unwrap().as_ref().unwrap()),
    );
    observed
}

fn effective(options: &ProviderOptions) -> EffectiveOptions {
    EffectiveOptions {
        tool_choice: None,
        timeout_ms: None,
        max_retries: None,
        cancellation: options.cancellation.clone(),
        requested_thinking: options.requested_thinking,
        thinking: options.thinking,
        effort: options.effort.clone(),
        thinking_budget: options.thinking_budget,
        temperature: options.temperature,
        output_limit: options.output_limit,
        transport: options.transport.clone(),
        cache_preference: options.cache_preference.clone(),
        session_affinity: options.session_affinity.clone(),
    }
}

fn assert_options(a: &EffectiveOptions, b: &EffectiveOptions) {
    assert_eq!(a.requested_thinking, b.requested_thinking);
    assert_eq!(a.thinking, b.thinking);
    assert_eq!(a.effort, b.effort);
    assert_eq!(a.thinking_budget, b.thinking_budget);
    assert_eq!(a.output_limit, b.output_limit);
    assert_eq!(a.temperature, b.temperature);
    assert_eq!(a.transport, b.transport);
    assert_eq!(a.cache_preference, b.cache_preference);
    assert_eq!(a.session_affinity, b.session_affinity);
}

#[test]
fn thinking_levels_resolve_through_registered_requests() {
    assert_eq!(StreamOptions::default().thinking, Off);
    for (requested, expected, effort) in [
        (Off, Off, None),
        (Minimal, Minimal, Some("minimal")),
        (Low, Low, Some("low")),
        (Medium, Medium, Some("medium")),
        (High, High, Some("high")),
        (Xhigh, High, Some("high")),
    ] {
        let result = observe(
            reasoning(),
            StreamOptions {
                thinking: requested,
                ..Default::default()
            },
        );
        assert_eq!(result.requested_thinking, requested);
        assert_eq!(result.thinking, expected);
        assert_eq!(result.effort.as_deref(), effort);
    }
    let mut model = reasoning();
    model
        .capabilities
        .thinking_level_map
        .insert(Xhigh, Some("extreme".into()));
    let result = observe(
        model,
        StreamOptions {
            thinking: Xhigh,
            ..Default::default()
        },
    );
    assert_eq!(result.thinking, Xhigh);
    assert_eq!(result.effort.as_deref(), Some("extreme"));
}

#[test]
fn disabled_levels_clamp_upward_then_downward() {
    for (disabled, requested, expected) in [
        (vec![Minimal, Low, Medium], Minimal, High),
        (vec![Minimal, Low, Medium], Low, High),
        (vec![Minimal, Low, Medium], Medium, High),
        (vec![High, Xhigh], High, Medium),
        (vec![High, Xhigh], Xhigh, Medium),
        (vec![Off], Off, Minimal),
    ] {
        let mut model = reasoning();
        for level in disabled {
            model.capabilities.thinking_level_map.insert(level, None);
        }
        let result = observe(
            model,
            StreamOptions {
                thinking: requested,
                ..Default::default()
            },
        );
        assert_eq!(result.thinking, expected);
        assert!(result.effort.is_some());
    }
}

#[test]
fn nonreasoning_and_fully_disabled_models_dispatch_without_thinking() {
    for reasoning_enabled in [false, true] {
        let mut model = model();
        model.capabilities.reasoning = reasoning_enabled;
        for level in [Off, Minimal, Low, Medium, High, Xhigh] {
            model.capabilities.thinking_level_map.insert(
                level,
                if reasoning_enabled {
                    None
                } else {
                    Some("ignored".into())
                },
            );
        }
        for requested in [Off, Minimal, Low, Medium, High, Xhigh] {
            let result = observe(
                model.clone(),
                StreamOptions {
                    thinking: requested,
                    ..Default::default()
                },
            );
            assert_eq!(result.thinking, Off);
            assert_eq!(result.effort, None);
            assert_eq!(result.thinking_budget, None);
        }
    }
}

#[test]
fn effort_mappings_are_used_without_identity_inference() {
    for labels in [
        ["local", "test", "script"],
        ["https://endpoint", "reasoning-xhigh", "websocket"],
    ] {
        for (requested, mapping, expected) in [
            (Off, Some("disabled-wire"), Some("disabled-wire")),
            (
                Medium,
                Some("EXACT unusual value"),
                Some("EXACT unusual value"),
            ),
            (Low, None, Some("low")),
            (Off, None, None),
        ] {
            let mut model = reasoning();
            model.identity.provider = labels[0].into();
            model.identity.model = labels[1].into();
            model.protocol = labels[2].into();
            if let Some(mapping) = mapping {
                model
                    .capabilities
                    .thinking_level_map
                    .insert(requested, Some(mapping.into()));
            }
            let result = observe(
                model,
                StreamOptions {
                    thinking: requested,
                    ..Default::default()
                },
            );
            assert_eq!(result.effort.as_deref(), expected);
            assert_eq!(result.thinking_budget, None);
        }
    }
}

#[test]
fn explicit_output_limits_are_preserved() {
    for ceiling in [-1, 0, 1, 32000, 100000] {
        for output in [0, 1, 31999, 32000, 32001, 100001, u64::MAX] {
            let mut model = reasoning();
            model.capabilities.output_limit = ceiling;
            let result = observe(
                model,
                StreamOptions {
                    thinking: Medium,
                    output_limit: Some(output),
                    ..Default::default()
                },
            );
            assert_eq!(result.output_limit, Some(output));
        }
    }
}

#[test]
fn omitted_output_uses_exact_positive_limit_default() {
    for (ceiling, expected) in [
        (1, Some(1)),
        (31999, Some(31999)),
        (32000, Some(32000)),
        (32001, Some(32000)),
        (0, None),
        (-1, None),
    ] {
        let mut model = reasoning();
        model.capabilities.output_limit = ceiling;
        let result = observe(model, StreamOptions::default());
        assert_eq!(result.output_limit, expected);
    }
}

#[test]
fn token_budgets_use_exact_levels_and_xhigh_alias() {
    for (level, budget, default_output, explicit_output) in [
        (Minimal, Some(1024), 33024, 11024),
        (Low, Some(2048), 34048, 12048),
        (Medium, Some(8192), 40192, 18192),
        (High, Some(16384), 48384, 26384),
        (Xhigh, Some(16384), 48384, 26384),
        (Off, None, 32000, 10000),
    ] {
        for (base, expected) in [(None, default_output), (Some(10000), explicit_output)] {
            let mut model = reasoning();
            model.capabilities.thinking_mode = ThinkingMode::TokenBudget;
            model.capabilities.output_limit = 100000;
            model
                .capabilities
                .thinking_level_map
                .insert(Xhigh, Some("not-a-number".into()));
            let result = observe(
                model,
                StreamOptions {
                    thinking: level,
                    output_limit: base,
                    ..Default::default()
                },
            );
            assert_eq!(result.thinking_budget, budget);
            assert_eq!(result.output_limit, Some(expected));
            assert_eq!(result.effort, None);
        }
    }
    let mut model = reasoning();
    model.capabilities.thinking_mode = ThinkingMode::TokenBudget;
    model.capabilities.output_limit = 40000;
    let result = observe(
        model,
        StreamOptions {
            thinking: High,
            ..Default::default()
        },
    );
    assert_eq!(result.output_limit, Some(40000));
    assert_eq!(result.thinking_budget, Some(16384));
}

#[test]
fn shared_ceiling_adjusts_budget_only_at_or_below_budget() {
    for (ceiling, base, output, budget) in [
        (8191, Some(10000), 8191, 7167),
        (8192, Some(10000), 8192, 7168),
        (8193, Some(10000), 8193, 8192),
        (1, None, 1, 0),
        (1023, None, 1023, 0),
        (1024, None, 1024, 0),
        (100000, Some(0), 8192, 7168),
        (100000, Some(u64::MAX), 100000, 8192),
    ] {
        let mut model = reasoning();
        model.capabilities.thinking_mode = ThinkingMode::TokenBudget;
        model.capabilities.output_limit = ceiling;
        let result = observe(
            model,
            StreamOptions {
                thinking: Medium,
                output_limit: base,
                ..Default::default()
            },
        );
        assert_eq!(result.output_limit, Some(output));
        assert_eq!(result.thinking_budget, Some(budget));
    }
}

#[test]
fn nonpositive_budget_ceiling_fails_only_for_enabled_thinking() {
    for ceiling in [0, -1] {
        for output in [None, Some(10000)] {
            let mut model = reasoning();
            model.capabilities.thinking_mode = ThinkingMode::TokenBudget;
            model.capabilities.output_limit = ceiling;
            let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
            let mut models = Models::new(Arc::new(|| 73));
            models.register(model.clone(), fake.clone()).unwrap();
            let options = StreamOptions {
                thinking: Medium,
                output_limit: output,
                ..Default::default()
            };
            assert!(matches!(
                models.resolve_options(&model, options.clone()),
                Err(Failure::UnsupportedOperation)
            ));
            let events = collect(
                models.stream(model.clone(), context(), options.clone()),
                &model,
                73,
            );
            assert_eq!(trace(&events), vec![("error", None)]);
            assert_eq!(
                terminal(&events).failure,
                Some(Failure::UnsupportedOperation)
            );
            assert_eq!(
                block_on(models.complete(model.clone(), context(), options.clone())),
                *terminal(&events)
            );
            assert!(fake.calls().is_empty());
            assert_eq!(fake.pending(), 1);
            options.cancellation.cancel();
            assert!(matches!(
                models.resolve_options(&model, options.clone()),
                Err(Failure::Cancelled)
            ));
            let cancelled = block_on(models.complete(model.clone(), context(), options));
            assert_eq!(cancelled.stop_reason, Some(StopReason::Aborted));
            assert_eq!(cancelled.failure, Some(Failure::Cancelled));
            assert!(fake.calls().is_empty());
            assert_eq!(fake.pending(), 1);
            let off = observe(
                model.clone(),
                StreamOptions {
                    output_limit: output,
                    ..Default::default()
                },
            );
            assert_eq!(off.output_limit, output);
            model.capabilities.thinking_mode = ThinkingMode::Effort;
            let effort = observe(
                model,
                StreamOptions {
                    thinking: Medium,
                    output_limit: output,
                    ..Default::default()
                },
            );
            assert_eq!(effort.output_limit, output);
        }
    }
}

fn supported_model() -> Model {
    let mut model = reasoning();
    model.capabilities.temperature = true;
    model.capabilities.transports = ["auto", "sse"].map(String::from).into();
    model.capabilities.cache_preferences = ["none", "short", "long"].map(String::from).into();
    model.capabilities.session_affinity = true;
    model
}

fn preferences() -> StreamOptions {
    StreamOptions {
        thinking: Medium,
        temperature: Some(0.0),
        output_limit: Some(40001),
        transport: Some("auto".into()),
        cache_preference: Some("short".into()),
        session_affinity: Some("session-exact".into()),
        headers: [("x-example".into(), "exact value".into())].into(),
        ..support::auth::local()
    }
}

#[test]
fn supported_preferences_reach_request_factory_unchanged() {
    for temperature in [Some(0.0), Some(0.75), None] {
        for transport in ["auto", "sse"] {
            for cache in ["none", "short", "long"] {
                let options = StreamOptions {
                    temperature,
                    transport: Some(transport.into()),
                    cache_preference: Some(cache.into()),
                    ..preferences()
                };
                let mut result = observe(supported_model(), options.clone());
                assert_eq!(result.temperature, temperature);
                assert_eq!(result.transport.as_deref(), Some(transport));
                assert_eq!(result.cache_preference.as_deref(), Some(cache));
                assert_eq!(result.output_limit, Some(40001));
                assert_eq!(result.session_affinity.as_deref(), Some("session-exact"));
                assert_eq!(result.headers, options.headers);
                result.headers.clear();
                assert_eq!(options.headers.len(), 1);
            }
        }
    }
    let options = preferences();
    let expected = options.clone();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        move |call| {
            assert_eq!(call.options.headers, expected.headers);
            assert_eq!(call.options.temperature, expected.temperature);
            assert_eq!(call.options.output_limit, expected.output_limit);
            assert_eq!(call.options.transport, expected.transport);
            assert_eq!(call.options.cache_preference, expected.cache_preference);
            assert_eq!(call.options.session_affinity, expected.session_affinity);
            Box::pin(async { Ok(vec![ScriptStep::Update(done())]) })
        },
    ))]));
    let model = supported_model();
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake.clone()).unwrap();
    let stream = models.stream(model.clone(), context(), options.clone());
    let mut caller_copy = options;
    caller_copy.headers.clear();
    caller_copy.transport = None;
    let events = collect(stream, &model, 73);
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Stop));
    let mut call_copy = fake.calls()[0].clone();
    call_copy.options.headers.clear();
    call_copy.model.capabilities.temperature = false;
    assert_eq!(fake.calls()[0].options.headers.len(), 1);
    assert!(fake.calls()[0].model.capabilities.temperature);
}

#[test]
fn unsupported_preferences_are_absent_without_capability_inference() {
    for labels in [
        ["local", "test", "script"],
        ["https://api.example", "thinking-cache-long", "sse"],
    ] {
        let mut model = model();
        model.identity.provider = labels[0].into();
        model.identity.model = labels[1].into();
        model.protocol = labels[2].into();
        let result = observe(model.clone(), preferences());
        assert_eq!(result.temperature, None);
        assert_eq!(result.transport, None);
        assert_eq!(result.cache_preference, None);
        assert_eq!(result.session_affinity, None);
        assert_eq!(result.headers, preferences().headers);
        model.capabilities.temperature = true;
        let changed = observe(model, preferences());
        assert_eq!(changed.temperature, Some(0.0));
        assert_eq!(changed.transport, None);
        assert_eq!(changed.cache_preference, None);
        assert_eq!(changed.session_affinity, None);
        assert_eq!(changed.headers, preferences().headers);
    }
    let result = observe(
        supported_model(),
        StreamOptions {
            transport: Some("undeclared-transport".into()),
            cache_preference: Some("undeclared-cache".into()),
            ..preferences()
        },
    );
    assert_eq!(result.transport, None);
    assert_eq!(result.cache_preference, None);
    let absent = observe(supported_model(), StreamOptions::default());
    assert_eq!(absent.temperature, None);
    assert_eq!(absent.transport, None);
    assert_eq!(absent.cache_preference, None);
    assert_eq!(absent.session_affinity, None);
    assert!(absent.headers.is_empty());
}

#[test]
fn resolution_uses_registered_snapshot_without_dispatch() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let samples = Arc::new(AtomicUsize::new(0));
    let clock_samples = samples.clone();
    let mut models = Models::new(Arc::new(move || {
        clock_samples.fetch_add(1, Ordering::SeqCst);
        73
    }));
    let registered = supported_model();
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    models.register(registered.clone(), fake.clone()).unwrap();
    let mut caller = registered.clone();
    caller.capabilities = RequestCapabilities::default();
    let resolved = models.resolve_options(&caller, preferences()).unwrap();
    assert_eq!(resolved.thinking, Medium);
    assert_eq!(resolved.temperature, Some(0.0));
    assert_eq!(samples.load(Ordering::SeqCst), 0);
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
    for (request, failure) in [
        (caller.clone(), Failure::UnknownProvider),
        (caller.clone(), Failure::UnknownModel),
        (caller.clone(), Failure::UnsupportedOperation),
        (caller.clone(), Failure::UnsupportedOperation),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (mut request, failure))| {
        match index {
            0 => request.identity.provider = "unknown".into(),
            1 => request.identity.model = "unknown".into(),
            2 => request.identity.operation = "embedding".into(),
            _ => request.protocol = "different".into(),
        }
        (request, failure)
    }) {
        assert!(
            matches!(models.resolve_options(&request, preferences()), Err(actual) if actual == failure)
        );
    }
    assert_eq!(samples.load(Ordering::SeqCst), 0);
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
    let stream = models.stream(caller.clone(), context(), preferences());
    caller.capabilities.temperature = false;
    caller.capabilities.thinking_level_map.insert(Medium, None);
    let events = collect(stream, &caller, 73);
    assert_eq!(terminal(&events).stop_reason, Some(StopReason::Stop));
    assert_eq!(fake.calls()[0].model, registered);
    assert_options(&resolved, &effective(&fake.calls()[0].options));
    assert_eq!(samples.load(Ordering::SeqCst), 1);

    struct CapabilityProvider {
        cancellation: Option<Cancellation>,
    }
    impl Provider for CapabilityProvider {
        fn supports(&self, _: &str) -> bool {
            if let Some(cancellation) = &self.cancellation {
                cancellation.cancel();
            }
            false
        }
        fn stream(
            &self,
            _: Model,
            _: Context,
            _: ProviderOptions,
        ) -> Result<Box<dyn ProviderStream>, Failure> {
            panic!("unsupported adapter dispatched")
        }
    }
    for cancel in [false, true] {
        let options = preferences();
        let mut models = Models::new(Arc::new(|| panic!("resolution sampled clock")));
        models
            .register(
                registered.clone(),
                Arc::new(CapabilityProvider {
                    cancellation: cancel.then(|| options.cancellation.clone()),
                }),
            )
            .unwrap();
        let failure = if cancel {
            Failure::Cancelled
        } else {
            Failure::UnsupportedOperation
        };
        assert!(
            matches!(models.resolve_options(&registered, options), Err(actual) if actual == failure)
        );
    }
}

#[test]
fn stream_and_completion_resolve_once_across_adapter_replacement() {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };
    struct RecordingProvider {
        options: Mutex<Vec<ProviderOptions>>,
        checks: AtomicUsize,
    }
    struct EmptySuccess(bool);
    impl ProviderStream for EmptySuccess {
        fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
            Box::pin(async move {
                if self.0 {
                    None
                } else {
                    self.0 = true;
                    Some(done())
                }
            })
        }
    }
    impl Provider for RecordingProvider {
        fn supports(&self, operation: &str) -> bool {
            self.checks.fetch_add(1, Ordering::SeqCst);
            operation == "chat"
        }
        fn stream(
            &self,
            _: Model,
            _: Context,
            options: ProviderOptions,
        ) -> Result<Box<dyn ProviderStream>, Failure> {
            self.options.lock().unwrap().push(options);
            Ok(Box::new(EmptySuccess(false)))
        }
    }
    fn invoke(
        provider: Arc<dyn Provider>,
        model: &Model,
        options: StreamOptions,
    ) -> (EffectiveOptions, AssistantMessage) {
        let mut models = Models::new(Arc::new(|| 73));
        models.register(model.clone(), provider).unwrap();
        let expected = models.resolve_options(model, options.clone()).unwrap();
        let events = collect(
            models.stream(model.clone(), context(), options.clone()),
            model,
            73,
        );
        let completed = block_on(models.complete(model.clone(), context(), options));
        assert_eq!(completed, *terminal(&events));
        assert_eq!(completed.stop_reason, Some(StopReason::Stop));
        (expected, completed)
    }
    let model = supported_model();
    let fake = Arc::new(ScriptedProvider::new(vec![
        steps(vec![done()]),
        steps(vec![done()]),
    ]));
    let direct = Arc::new(RecordingProvider {
        options: Mutex::new(vec![]),
        checks: AtomicUsize::new(0),
    });
    let (expected_fake, fake_terminal) = invoke(fake.clone(), &model, preferences());
    let (expected_direct, direct_terminal) = invoke(direct.clone(), &model, preferences());
    assert_options(&expected_fake, &expected_direct);
    assert_eq!(fake_terminal, direct_terminal);
    assert_eq!(fake.calls().len(), 2);
    assert_eq!(direct.checks.load(Ordering::SeqCst), 3);
    let direct_options = direct.options.lock().unwrap();
    assert_eq!(direct_options.len(), 2);
    for (call, options) in fake.calls().iter().zip(direct_options.iter()) {
        assert_options(&expected_fake, &effective(&call.options));
        assert_options(&expected_direct, &effective(options));
        assert_options(&effective(&call.options), &effective(options));
        assert_eq!(call.options.headers, options.headers);
    }
}

#[test]
fn resolved_options_preserve_cancellation() {
    use std::sync::atomic::Ordering;
    let model = supported_model();
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake.clone()).unwrap();
    let options = preferences();
    let shared = models.resolve_options(&model, options.clone()).unwrap();
    let independent = models
        .resolve_options(&model, StreamOptions::default())
        .unwrap();
    shared.clone().cancellation.cancel();
    assert!(options.cancellation.is_cancelled());
    assert!(!independent.cancellation.is_cancelled());
    let events = collect(models.stream(model.clone(), context(), options), &model, 73);
    assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
    for with_partial in [false, true] {
        let gate = Gate::default();
        let mut updates = vec![];
        if with_partial {
            updates.push(ScriptStep::Update(ProviderUpdate::TextStart {
                content_index: 0,
            }));
            updates.push(ScriptStep::Update(ProviderUpdate::TextDelta {
                content_index: 0,
                delta: "retained".into(),
            }));
        }
        updates.push(ScriptStep::Wait(gate.wait()));
        updates.push(ScriptStep::Update(done()));
        let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(updates)]));
        let mut models = Models::new(Arc::new(|| 73));
        models.register(model.clone(), fake.clone()).unwrap();
        let options = preferences();
        let resolved = models.resolve_options(&model, options.clone()).unwrap();
        let mut stream = models.stream(model.clone(), context(), options);
        let mut events = vec![];
        if with_partial {
            for _ in 0..3 {
                events.push(block_on(stream.next()).unwrap());
            }
        }
        let wakes = Arc::new(WakeCounter::default());
        let mut next = Box::pin(stream.next());
        assert!(poll(next.as_mut(), &wakes).is_pending());
        assert_options(&resolved, &effective(&fake.calls()[0].options));
        fake.calls()[0].options.cancellation.cancel();
        assert!(resolved.cancellation.is_cancelled());
        assert!(wakes.0.load(Ordering::SeqCst) > 0);
        let std::task::Poll::Ready(Some(event)) = poll(next.as_mut(), &wakes) else {
            panic!("resolved cancellation did not wake the blocked request")
        };
        drop(next);
        events.push(event);
        assert_contract(&events, &model, 73);
        assert_eq!(terminal(&events).failure, Some(Failure::Cancelled));
        assert_eq!(terminal(&events).stop_reason, Some(StopReason::Aborted));
        let expected = if with_partial {
            vec![AssistantContent::Text(TextContent {
                text: "retained".into(),
                replay_metadata: None,
            })]
        } else {
            vec![]
        };
        assert_eq!(terminal(&events).content, expected);
        assert_eq!(gate.1.load(Ordering::SeqCst), 1);
        assert_eq!(fake.calls().len(), 1);
        assert_eq!(block_on(stream.next()), None);
    }
}
