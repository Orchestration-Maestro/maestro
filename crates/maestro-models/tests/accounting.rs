mod support;
use maestro_models::*;
use std::sync::Arc;
use support::{auth::local, block_on, conformance::*};

fn rates() -> TokenRates {
    TokenRates {
        input: 2.0,
        output: 8.0,
        cache_read: 1.0,
        cache_write: 4.0,
    }
}
fn report() -> Usage {
    Usage {
        input: 1_000_000,
        output: 500_000,
        cache_read: 250_000,
        cache_write: 125_000,
        total_tokens: 99,
        ..Usage::default()
    }
}
fn priced_registry(provider: Arc<dyn Provider>, rates: Option<TokenRates>) -> Models {
    let mut models = Models::new(Arc::new(|| 73));
    let mut selected = model();
    selected.rates = rates;
    models.register(selected, provider).unwrap();
    models
}
fn attempt(updates: Vec<ProviderUpdate>, rates: Option<TokenRates>) -> Vec<ModelEvent> {
    let fake = Arc::new(ScriptedProvider::new(vec![steps(updates)]));
    let models = priced_registry(fake.clone(), rates);
    let events = collect(models.stream(model(), context(), local()), &model(), 73);
    assert_eq!(fake.calls().len(), 1);
    events
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-12, "{actual} != {expected}");
}
fn assert_fixture(usage: &Usage) {
    assert_eq!(
        (
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write,
            usage.total_tokens
        ),
        (1_000_000, 500_000, 250_000, 125_000, 1_875_000)
    );
    assert!(usage.reported && usage.cost.priced);
    close(usage.cost.input, 2.0);
    close(usage.cost.output, 4.0);
    close(usage.cost.cache_read, 0.25);
    close(usage.cost.cache_write, 0.5);
    close(usage.cost.total, 6.75);
}

#[test]
fn reported_categories_derive_total_and_flat_cost() {
    for total in [99, 1_875_000, 0] {
        let mut usage = report();
        usage.total_tokens = total;
        usage.reported = false;
        usage.cost = UsageCost {
            input: 900.0,
            output: 901.0,
            cache_read: 902.0,
            cache_write: 903.0,
            total: 999.0,
            priced: false,
        };
        let events = attempt(vec![ProviderUpdate::Usage { usage }, done()], Some(rates()));
        assert_fixture(&terminal(&events).usage);
    }
}

#[test]
fn reasoning_and_cache_writes_are_counted_once() {
    for length in [0, 1, 500] {
        for (input, output, cache_read, cache_write, expected) in [
            (600, 100, 300, 100, 1_100),
            (0, 0, 0, 100, 100),
            (0, 0, 100, 0, 100),
        ] {
            let mut updates = vec![
                ProviderUpdate::ThinkingStart {
                    content_index: 0,
                    signature: None,
                },
                ProviderUpdate::ThinkingDelta {
                    content_index: 0,
                    delta: "r".repeat(length),
                },
                ProviderUpdate::ThinkingEnd { content_index: 0 },
                ProviderUpdate::RedactedThinking {
                    content_index: 1,
                    data: "opaque".into(),
                },
            ];
            updates.extend(text(2, &"t".repeat(length)));
            updates.extend([
                ProviderUpdate::Usage {
                    usage: Usage {
                        input,
                        output,
                        cache_read,
                        cache_write,
                        total_tokens: 9_999,
                        ..Usage::default()
                    },
                },
                done(),
            ]);
            let events = attempt(updates, None);
            let usage = &terminal(&events).usage;
            assert_eq!(usage.total_tokens, expected);
            assert_eq!(
                (
                    usage.input,
                    usage.output,
                    usage.cache_read,
                    usage.cache_write
                ),
                (input, output, cache_read, cache_write)
            );
        }
    }
}

#[test]
fn usage_updates_replace_instead_of_accumulate() {
    let first = Usage {
        input: 100,
        output: 50,
        cache_read: 20,
        cache_write: 10,
        ..Usage::default()
    };
    let second = Usage {
        input: 120,
        output: 60,
        cache_read: 25,
        cache_write: 15,
        ..Usage::default()
    };
    let mut updates = vec![ProviderUpdate::Usage { usage: first }];
    updates.extend(text(0, "early"));
    updates.extend([
        ProviderUpdate::Usage {
            usage: second.clone(),
        },
        ProviderUpdate::Usage { usage: second },
        done(),
    ]);
    let events = attempt(
        updates,
        Some(TokenRates {
            input: 1.0,
            output: 2.0,
            cache_read: 3.0,
            cache_write: 4.0,
        }),
    );
    let early = &snapshot(&events[0]).usage;
    assert_eq!(early.total_tokens, 180);
    close(early.cost.total, 0.0003);
    let final_usage = &terminal(&events).usage;
    assert_eq!(
        (
            final_usage.input,
            final_usage.output,
            final_usage.cache_read,
            final_usage.cache_write,
            final_usage.total_tokens
        ),
        (120, 60, 25, 15, 220)
    );
    close(final_usage.cost.total, 0.000375);
    assert!(early.reported && early.cost.priced && final_usage.reported && final_usage.cost.priced);
}

#[test]
fn unreported_usage_differs_from_an_explicit_zero_report() {
    let mut content = text(0, &"long".repeat(500));
    content.extend([
        ProviderUpdate::ThinkingStart {
            content_index: 1,
            signature: None,
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 1,
            delta: "reason".repeat(500),
        },
        ProviderUpdate::ThinkingEnd { content_index: 1 },
        done(),
    ]);
    for updates in [
        vec![done()],
        content,
        vec![ProviderUpdate::Error {
            failure: Failure::Transport,
        }],
        vec![],
        vec![ProviderUpdate::Error {
            failure: Failure::Cancelled,
        }],
    ] {
        let events = attempt(updates, Some(rates()));
        for event in &events {
            assert_eq!(snapshot(event).usage, Usage::default());
        }
    }
    let fake = Arc::new(ScriptedProvider::new(vec![Script::SetupFailure(
        Failure::AdapterFailed,
    )]));
    let models = priced_registry(fake, Some(rates()));
    let failed = block_on(models.complete(model(), context(), local()));
    assert_eq!(failed.failure, Some(Failure::AdapterFailed));
    assert_eq!(failed.usage, Usage::default());
    let options = local();
    options.cancellation.cancel();
    let aborted = block_on(models.complete(model(), context(), options));
    assert_eq!(aborted.stop_reason, Some(StopReason::Aborted));
    assert_eq!(aborted.usage, Usage::default());
    for supplied in [None, Some(rates())] {
        let expected_priced = supplied.is_some();
        let events = attempt(
            vec![
                ProviderUpdate::Usage {
                    usage: Usage::default(),
                },
                done(),
            ],
            supplied,
        );
        let usage = &terminal(&events).usage;
        assert!(usage.reported);
        assert_eq!(usage.cost.priced, expected_priced);
        assert_eq!(
            (
                usage.input,
                usage.output,
                usage.cache_read,
                usage.cache_write,
                usage.total_tokens
            ),
            (0, 0, 0, 0, 0)
        );
        assert_eq!(
            (
                usage.cost.input,
                usage.cost.output,
                usage.cost.cache_read,
                usage.cost.cache_write,
                usage.cost.total
            ),
            (0.0, 0.0, 0.0, 0.0, 0.0)
        );
    }
}

#[test]
fn missing_rates_and_explicit_zero_rates_are_distinct() {
    for supplied in [None, Some(TokenRates::default())] {
        let expected_priced = supplied.is_some();
        let events = attempt(
            vec![ProviderUpdate::Usage { usage: report() }, done()],
            supplied,
        );
        let usage = &terminal(&events).usage;
        assert_eq!(usage.total_tokens, 1_875_000);
        assert!(usage.reported);
        assert_eq!(
            usage.cost,
            UsageCost {
                priced: expected_priced,
                ..UsageCost::default()
            }
        );
    }
    let events = attempt(
        vec![ProviderUpdate::Usage { usage: report() }, done()],
        Some(TokenRates {
            input: 0.0,
            output: 8.0,
            cache_read: 0.0,
            cache_write: 4.0,
        }),
    );
    let cost = &terminal(&events).usage.cost;
    assert!(cost.priced);
    assert_eq!(cost.input, 0.0);
    assert_eq!(cost.cache_read, 0.0);
    close(cost.output, 4.0);
    close(cost.cache_write, 0.5);
    close(cost.total, 4.5);
}

#[test]
fn partial_reported_accounting_survives_error_and_eof() {
    for failure in [
        Failure::Transport,
        Failure::Cancelled,
        Failure::IncompleteStream,
    ] {
        let mut updates = text(0, "partial");
        updates.extend([
            ProviderUpdate::Usage { usage: report() },
            ProviderUpdate::ResponseIdentity {
                response_model: Some("actual".into()),
                response_id: Some("response".into()),
            },
        ]);
        if failure != Failure::IncompleteStream {
            updates.push(ProviderUpdate::Error { failure });
        }
        let events = attempt(updates, Some(rates()));
        let result = terminal(&events);
        assert_eq!(result.failure, Some(failure));
        assert_eq!(
            result.stop_reason,
            Some(if failure == Failure::Cancelled {
                StopReason::Aborted
            } else {
                StopReason::Error
            })
        );
        assert_fixture(&result.usage);
        assert_eq!(
            result.content,
            vec![AssistantContent::Text(TextContent {
                text: "partial".into(),
                replay_metadata: None,
            })]
        );
        assert_eq!(result.response_model.as_deref(), Some("actual"));
        assert_eq!(result.response_id.as_deref(), Some("response"));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, ModelEvent::Done { .. }))
        );
    }
}

#[test]
fn catalog_rates_are_captured_without_rewriting_response_identity() {
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![
        ProviderUpdate::Usage { usage: report() },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual-other".into()),
            response_id: Some("id-other".into()),
        },
        done(),
    ])]));
    let models = priced_registry(fake.clone(), Some(rates()));
    let mut invocation = model();
    invocation.rates = Some(TokenRates {
        input: 100.0,
        ..TokenRates::default()
    });
    let stream = models.stream(invocation.clone(), context(), local());
    invocation.rates.as_mut().unwrap().input = 1_000.0;
    invocation.identity.model = "changed".into();
    let events = collect(stream, &model(), 73);
    let result = terminal(&events);
    assert_fixture(&result.usage);
    assert_eq!(
        (&result.provider, &result.protocol, &result.model),
        (
            &model().identity.provider,
            &model().protocol,
            &model().identity.model
        )
    );
    assert_eq!(result.response_model.as_deref(), Some("actual-other"));
    assert_eq!(result.response_id.as_deref(), Some("id-other"));
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn separate_attempts_never_share_accounting() {
    let mut first = vec![
        ProviderUpdate::Usage { usage: usage() },
        tool_start(0),
        ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "{}".into(),
        },
        ProviderUpdate::ToolCallEnd { content_index: 0 },
    ];
    first.push(ProviderUpdate::Done {
        reason: StopReason::ToolUse,
    });
    let fake = Arc::new(ScriptedProvider::new(vec![
        steps(first),
        steps(vec![
            ProviderUpdate::Usage {
                usage: Usage {
                    input: 2,
                    output: 1,
                    cache_read: 1,
                    ..Usage::default()
                },
            },
            done(),
        ]),
        steps(vec![done()]),
    ]));
    let models = priced_registry(fake.clone(), Some(rates()));
    let first = block_on(models.complete(model(), context(), local()));
    let second = block_on(models.complete(model(), context(), local()));
    let third = block_on(models.complete(model(), context(), local()));
    assert_eq!(first.usage.total_tokens, 23);
    assert_eq!(
        (
            first.usage.input,
            first.usage.output,
            first.usage.cache_read,
            first.usage.cache_write
        ),
        (11, 7, 3, 2)
    );
    close(first.usage.cost.total, 0.000089);
    assert!(first.usage.reported && first.usage.cost.priced);
    assert!(
        matches!(&first.content[0], AssistantContent::ToolCall(call) if call.arguments().is_some())
    );
    assert_eq!(
        (
            second.usage.input,
            second.usage.output,
            second.usage.cache_read,
            second.usage.cache_write,
            second.usage.total_tokens
        ),
        (2, 1, 1, 0, 4)
    );
    close(second.usage.cost.total, 0.000013);
    assert!(second.usage.reported && second.usage.cost.priced);
    assert_eq!(third.usage, Usage::default());
    assert_eq!(first.usage.total_tokens, 23);
    assert_eq!(fake.calls().len(), 3);
}

#[test]
fn invalid_accounting_does_not_publish_or_replace_measurements() {
    let mut updates = text(0, "valid");
    updates.extend([
        ProviderUpdate::Usage { usage: usage() },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual".into()),
            response_id: Some("id".into()),
        },
        ProviderUpdate::TextStart { content_index: 1 },
        ProviderUpdate::Usage {
            usage: Usage {
                input: u64::MAX,
                output: 1,
                ..Usage::default()
            },
        },
        done(),
    ]);
    let events = attempt(updates, Some(rates()));
    let result = terminal(&events);
    assert_eq!(result.failure, Some(Failure::MalformedStream));
    assert_eq!(result.usage.total_tokens, 23);
    close(result.usage.cost.total, 0.000089);
    assert!(result.usage.reported && result.usage.cost.priced);
    assert_eq!(result.usage, snapshot(&events[events.len() - 2]).usage);
    assert_eq!(result.content, snapshot(&events[events.len() - 2]).content);
    assert_eq!(result.response_model.as_deref(), Some("actual"));
    assert_eq!(result.response_id.as_deref(), Some("id"));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ModelEvent::Error { .. }))
            .count(),
        1
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, ModelEvent::Done { .. }))
    );
    for invalid in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for category in 0..4 {
            let mut rates = rates();
            match category {
                0 => rates.input = invalid,
                1 => rates.output = invalid,
                2 => rates.cache_read = invalid,
                _ => rates.cache_write = invalid,
            }
            let events = attempt(
                vec![
                    ProviderUpdate::Usage {
                        usage: Usage::default(),
                    },
                    done(),
                ],
                Some(rates),
            );
            assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
            assert_eq!(terminal(&events).usage, Usage::default());
            assert_eq!(trace(&events), vec![("error", None)]);
        }
    }
    for supplied in [
        Usage {
            input: u64::MAX,
            ..Usage::default()
        },
        Usage {
            input: 300_000,
            output: 300_000,
            cache_read: 300_000,
            cache_write: 300_000,
            ..Usage::default()
        },
    ] {
        let events = attempt(
            vec![ProviderUpdate::Usage { usage: supplied }, done()],
            Some(TokenRates {
                input: f64::MAX,
                output: f64::MAX,
                cache_read: f64::MAX,
                cache_write: f64::MAX,
            }),
        );
        assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
        assert_eq!(terminal(&events).usage, Usage::default());
    }
}
