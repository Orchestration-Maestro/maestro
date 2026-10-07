#[cfg(test)]
mod tests {
    use maestro_models::records::diagnostics::{
        DiagnosticErrorInfo, DiagnosticInput, extract_diagnostic_error,
    };
    use maestro_models::records::session_resources::{
        cleanup_session_resources, register_session_resource_cleanup,
    };
    use std::sync::{Arc, Mutex};

    static GLOBAL: Mutex<()> = Mutex::new(());

    fn error(text: &str) -> DiagnosticErrorInfo {
        extract_diagnostic_error(DiagnosticInput::Text(text))
    }

    #[test]
    fn maestro_cleanup_collects_failures_in_order() {
        let _serial = GLOBAL.lock().unwrap();
        let visits = Arc::new(Mutex::new(Vec::new()));
        let removals: Vec<_> = ["first", "success", "last"]
            .into_iter()
            .map(|name| {
                let visits = Arc::clone(&visits);
                register_session_resource_cleanup(Arc::new(move |session| {
                    assert_eq!(session, Some("session"));
                    visits.lock().unwrap().push(name);
                    match name {
                        "success" => Ok(()),
                        _ => Err(error(name)),
                    }
                }))
            })
            .collect();
        let aggregate = cleanup_session_resources(Some("session")).unwrap_err();
        assert_eq!(aggregate.to_string(), "Failed to cleanup session resources");
        assert_eq!(
            aggregate
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            ["first", "last"]
        );
        assert_eq!(*visits.lock().unwrap(), ["first", "success", "last"]);
        for removal in removals {
            removal.remove();
        }
        assert!(cleanup_session_resources(None).is_ok());
    }

    fn model(api: &str) -> maestro_models::records::types::Model {
        serde_json::from_value(serde_json::json!({"id":"custom-model","name":"Custom","api":api,"provider":"custom","baseUrl":"https://fixture.invalid","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":100,"maxTokens":10})).unwrap()
    }
    fn context() -> maestro_models::records::types::Context {
        maestro_models::records::types::Context {
            system_prompt: Some("system".into()),
            messages: vec![],
            tools: None,
        }
    }
    fn final_stream(timestamp: f64) -> maestro_models::records::types::AssistantMessageEventStream {
        use maestro_models::records::types::{
            AssistantMessageEvent, AssistantMessageEventStream, DoneReason,
        };
        let message = Arc::new(std::sync::RwLock::new(serde_json::from_value(serde_json::json!({"role":"assistant","content":[],"api":"custom","provider":"custom","model":"m","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":timestamp})).unwrap()));
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message,
        });
        stream
    }
    fn ready<F: std::future::Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("expected ready"),
        }
    }
    fn provider(api: &str, timestamp: f64) -> maestro_models::records::api_registry::ApiProvider {
        maestro_models::records::api_registry::ApiProvider {
            api: api.into(),
            stream: Arc::new(move |_, _, _| Ok(final_stream(timestamp))),
            stream_simple: Arc::new(move |_, _, _| Ok(final_stream(timestamp + 1.0))),
        }
    }

    fn assert_retained_provider(retained: &maestro_models::records::api_registry::ApiProvider) {
        assert_eq!(
            ready(
                (retained.stream)(model("2"), context(), None)
                    .unwrap()
                    .result()
            )
            .read()
            .unwrap()
            .timestamp
            .to_bits(),
            20.0_f64.to_bits()
        );
        assert_eq!(
            ready(
                (retained.stream_simple)(model("2"), context(), None)
                    .unwrap()
                    .result()
            )
            .read()
            .unwrap()
            .timestamp
            .to_bits(),
            21.0_f64.to_bits()
        );
    }

    #[test]
    fn maestro_dispatch_preserves_registration_order() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::{stream, stream_simple};
        let _serial = GLOBAL.lock().unwrap();
        clear_api_providers();
        for (api, stamp) in [("10", 10.0), ("2", 20.0), ("01", 30.0)] {
            register_api_provider(provider(api, stamp), Some("old".into()));
        }
        let retained = get_api_provider("2").unwrap();
        register_api_provider(provider("2", 40.0), Some("new".into()));
        assert_eq!(
            get_api_providers()
                .iter()
                .map(|entry| entry.api.as_str())
                .collect::<Vec<_>>(),
            ["10", "2", "01"]
        );
        assert_eq!(
            ready(stream(model("2"), context(), None).unwrap().result())
                .read()
                .unwrap()
                .timestamp
                .to_bits(),
            40.0_f64.to_bits()
        );
        assert_eq!(
            ready(stream_simple(model("2"), context(), None).unwrap().result())
                .read()
                .unwrap()
                .timestamp
                .to_bits(),
            41.0_f64.to_bits()
        );
        unregister_api_providers("old");
        assert_eq!(
            get_api_providers()
                .iter()
                .map(|entry| entry.api.as_str())
                .collect::<Vec<_>>(),
            ["2"]
        );
        unregister_api_providers("absent");
        clear_api_providers();
        assert!(get_api_providers().is_empty());
        assert!(get_api_provider("2").is_none());
        assert_retained_provider(&retained);
        register_api_provider(provider("2", 50.0), None);
        unregister_api_providers("new");
        assert!(get_api_provider("2").is_some());
        clear_api_providers();
    }

    #[test]
    fn maestro_api_wrappers_reject_wrong_models() {
        use maestro_models::records::api_registry::*;
        let _serial = GLOBAL.lock().unwrap();
        clear_api_providers();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let raw = Arc::clone(&calls);
        let simple = Arc::clone(&calls);
        register_api_provider(
            ApiProvider {
                api: "expected".into(),
                stream: Arc::new(move |_, _, _| {
                    raw.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(final_stream(1.0))
                }),
                stream_simple: Arc::new(move |_, _, _| {
                    simple.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(final_stream(2.0))
                }),
            },
            None,
        );
        let provider = get_api_provider("expected").unwrap();
        for result in [
            (provider.stream)(model("wrong"), context(), None),
            (provider.stream_simple)(model("wrong"), context(), None),
        ] {
            let failure = result.err().unwrap();
            assert_eq!(failure.message, "Mismatched api: wrong expected expected");
            assert_eq!(failure.name.as_deref(), Some("Error"));
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        clear_api_providers();
    }

    #[test]
    fn maestro_dispatch_reports_missing_api() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::*;
        let _serial = GLOBAL.lock().unwrap();
        clear_api_providers();
        for failure in [
            stream(model("missing"), context(), None).err().unwrap(),
            stream_simple(model("missing"), context(), None)
                .err()
                .unwrap(),
            ready(complete(model("missing"), context(), None)).unwrap_err(),
            ready(complete_simple(model("missing"), context(), None)).unwrap_err(),
        ] {
            assert_eq!(
                failure.message,
                "No API provider registered for api: missing"
            );
            assert_eq!(failure.name.as_deref(), Some("Error"));
        }
        let failure = DiagnosticErrorInfo {
            name: Some("Setup".into()),
            message: "adapter failure".into(),
            stack: Some("supplied stack".into()),
            code: Some(maestro_models::records::diagnostics::DiagnosticCode::Text(
                "E_SETUP".into(),
            )),
        };
        let raw = failure.clone();
        let simple = failure.clone();
        register_api_provider(
            ApiProvider {
                api: "failure".into(),
                stream: Arc::new(move |_, _, _| Err(raw.clone())),
                stream_simple: Arc::new(move |_, _, _| Err(simple.clone())),
            },
            None,
        );
        assert_eq!(
            stream(model("failure"), context(), None).err().unwrap(),
            failure
        );
        assert_eq!(
            stream_simple(model("failure"), context(), None)
                .err()
                .unwrap(),
            failure
        );
        assert_eq!(
            ready(complete(model("failure"), context(), None)).unwrap_err(),
            failure
        );
        assert_eq!(
            ready(complete_simple(model("failure"), context(), None)).unwrap_err(),
            failure
        );
        clear_api_providers();
    }

    fn common_options() -> maestro_models::records::types::StreamOptions {
        use maestro_models::records::types::{CacheRetention, StreamOptions, Transport};
        StreamOptions {
            temperature: Some(0.25),
            max_tokens: Some(17.0),
            signal: Some(maestro_models::Cancellation::new()),
            api_key: Some("controlled".into()),
            transport: Some(Transport::WebsocketCached),
            cache_retention: Some(CacheRetention::Long),
            session_id: Some("session".into()),
            on_payload: Some(Arc::new(|_, _| Box::pin(async { Ok(None) }))),
            on_response: Some(Arc::new(|_, _| Box::pin(async { Ok(()) }))),
            headers: Some([("x".into(), "y".into())].into()),
            timeout_ms: Some(123.0),
            max_retries: Some(2.0),
            max_retry_delay_ms: Some(456.0),
            metadata: Some(
                serde_json::json!({"caller":null})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        }
    }
    fn check_common(
        actual: &maestro_models::records::types::StreamOptions,
        expected: &maestro_models::records::types::StreamOptions,
    ) {
        assert_eq!(actual.temperature, expected.temperature);
        assert_eq!(actual.max_tokens, expected.max_tokens);
        assert_eq!(actual.api_key, expected.api_key);
        assert_eq!(actual.transport, expected.transport);
        assert_eq!(actual.cache_retention, expected.cache_retention);
        assert_eq!(actual.session_id, expected.session_id);
        assert_eq!(actual.headers, expected.headers);
        assert_eq!(actual.timeout_ms, expected.timeout_ms);
        assert_eq!(actual.max_retries, expected.max_retries);
        assert_eq!(actual.max_retry_delay_ms, expected.max_retry_delay_ms);
        assert_eq!(actual.metadata, expected.metadata);
        assert!(Arc::ptr_eq(
            actual.on_payload.as_ref().unwrap(),
            expected.on_payload.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(
            actual.on_response.as_ref().unwrap(),
            expected.on_response.as_ref().unwrap()
        ));
        actual.signal.as_ref().unwrap().abort();
        assert!(expected.signal.as_ref().unwrap().is_aborted());
    }
    fn completion_provider(
        output: &maestro_models::records::types::AssistantMessageEventStream,
        expected: &maestro_models::records::types::StreamOptions,
        count: &Arc<std::sync::atomic::AtomicUsize>,
    ) -> maestro_models::records::api_registry::ApiProvider {
        use maestro_models::records::api_registry::ApiProvider;
        use maestro_models::records::types::*;
        let output = output.clone();
        let raw_expected = expected.clone();
        let simple_expected = expected.clone();
        let raw = Arc::clone(count);
        let simple = Arc::clone(count);
        let output_simple = output.clone();
        ApiProvider {
            api: "custom".into(),
            stream: Arc::new(move |descriptor, conversation, options| {
                raw.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(descriptor, model("custom"));
                assert_eq!(conversation, context());
                let options = options.unwrap();
                check_common(&options.common, &raw_expected);
                assert_eq!(
                    options.extra,
                    serde_json::json!({"providerSpecific":"present","maxTokens":999})
                        .as_object()
                        .unwrap()
                        .clone()
                );
                Ok(output.clone())
            }),
            stream_simple: Arc::new(move |descriptor, conversation, options| {
                simple.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(descriptor, model("custom"));
                assert_eq!(conversation, context());
                let options = options.unwrap();
                check_common(&options.common, &simple_expected);
                assert_eq!(options.reasoning, Some(ThinkingLevel::Xhigh));
                assert_eq!(
                    options.thinking_budgets,
                    Some(ThinkingBudgets {
                        minimal: Some(1.0),
                        low: Some(2.0),
                        medium: Some(3.0),
                        high: Some(4.0)
                    })
                );
                Ok(output_simple.clone())
            }),
        }
    }

    #[test]
    fn maestro_completion_observes_without_consuming() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::{complete, complete_simple};
        use maestro_models::records::types::*;
        let _serial = GLOBAL.lock().unwrap();
        clear_api_providers();
        let output = final_stream(71.0);
        let retained = output.clone();
        let raw_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let simple_count = Arc::clone(&raw_count);
        let expected = common_options();
        register_api_provider(completion_provider(&output, &expected, &raw_count), None);
        let raw_result = complete(
            model("custom"),
            context(),
            Some(ProviderStreamOptions {
                common: expected.clone(),
                extra: serde_json::json!({"providerSpecific":"present","maxTokens":999})
                    .as_object()
                    .unwrap()
                    .clone(),
            }),
        );
        assert_eq!(raw_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        let simple_result = complete_simple(
            model("custom"),
            context(),
            Some(SimpleStreamOptions {
                common: expected,
                reasoning: Some(ThinkingLevel::Xhigh),
                thinking_budgets: Some(ThinkingBudgets {
                    minimal: Some(1.0),
                    low: Some(2.0),
                    medium: Some(3.0),
                    high: Some(4.0),
                }),
            }),
        );
        assert_eq!(simple_count.load(std::sync::atomic::Ordering::SeqCst), 2);
        register_api_provider(provider("custom", 99.0), None);
        clear_api_providers();
        let result = ready(raw_result).unwrap();
        assert_eq!(
            result.read().unwrap().timestamp.to_bits(),
            71.0_f64.to_bits()
        );
        assert!(Arc::ptr_eq(&result, &ready(simple_result).unwrap()));
        assert!(Arc::ptr_eq(&result, &ready(retained.result())));
        assert!(matches!(
            ready(retained.next()),
            Some(AssistantMessageEvent::Done { .. })
        ));
        assert!(ready(retained.next()).is_none());
    }

    fn hook_callbacks(
        calls: &Arc<std::sync::atomic::AtomicUsize>,
    ) -> (
        maestro_models::records::types::OnPayload,
        maestro_models::records::types::OnResponse,
    ) {
        use maestro_models::records::types::*;
        let payload_calls = Arc::clone(calls);
        let response_calls = Arc::clone(calls);
        let on_payload: OnPayload = Arc::new(move |payload, descriptor| {
            payload_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(descriptor, model("hooks"));
            Box::pin(async move {
                match payload["mode"].as_str().unwrap() {
                    "replace" => Ok(Some(serde_json::json!({"stamp":2}))),
                    "payload_error" => Err(error("payload failure")),
                    _ => Ok(None),
                }
            })
        });
        let on_response: OnResponse = Arc::new(move |response, descriptor| {
            response_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(descriptor, model("hooks"));
            assert_eq!(
                response.headers.get("x").map(String::as_str),
                Some("response")
            );
            Box::pin(async move {
                match response.status.to_bits() {
                    bits if bits == 500.0_f64.to_bits() => Err(error("response failure")),
                    _ => Ok(()),
                }
            })
        });
        (on_payload, on_response)
    }

    fn hook_provider() -> maestro_models::records::api_registry::ApiProvider {
        use maestro_models::records::api_registry::ApiProvider;
        use maestro_models::records::types::*;
        ApiProvider {
            api: "hooks".into(),
            stream: Arc::new(|descriptor, conversation, options| {
                let options = options.unwrap().common;
                let mode = conversation.system_prompt.unwrap();
                let payload = serde_json::json!({"mode":mode,"stamp":1});
                let replacement = ready(options.on_payload.unwrap()(
                    payload.clone(),
                    descriptor.clone(),
                ))?;
                let submitted = replacement.unwrap_or(payload);
                ready(options.on_response.unwrap()(
                    ProviderResponse {
                        status: if mode == "response_error" {
                            500.0
                        } else {
                            200.0
                        },
                        headers: [("x".into(), "response".into())].into(),
                    },
                    descriptor,
                ))?;
                Ok(final_stream(submitted["stamp"].as_f64().unwrap()))
            }),
            stream_simple: Arc::new(|_, _, _| Ok(final_stream(0.0))),
        }
    }

    #[test]
    fn maestro_hooks_keep_replacements_and_failures() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::stream;
        use maestro_models::records::types::*;
        let _serial = GLOBAL.lock().unwrap();
        clear_api_providers();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (on_payload, on_response) = hook_callbacks(&calls);
        register_api_provider(hook_provider(), None);
        let _retained = get_api_provider("hooks").unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        for (mode, expected) in [
            ("keep", Ok(1.0)),
            ("replace", Ok(2.0)),
            ("payload_error", Err(error("payload failure"))),
            ("response_error", Err(error("response failure"))),
        ] {
            let options = ProviderStreamOptions {
                common: StreamOptions {
                    on_payload: Some(Arc::clone(&on_payload)),
                    on_response: Some(Arc::clone(&on_response)),
                    ..StreamOptions::default()
                },
                extra: JsonObject::new(),
            };
            let result = stream(
                model("hooks"),
                Context {
                    system_prompt: Some(mode.into()),
                    ..context()
                },
                Some(options),
            )
            .map(|output| ready(output.result()).read().unwrap().timestamp);
            assert_eq!(result, expected);
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 7);
        clear_api_providers();
    }

    struct ReenterDrop(Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for ReenterDrop {
        fn drop(&mut self) {
            use maestro_models::records::api_registry::{get_api_provider, get_api_providers};
            let _entries = get_api_providers();
            let _entry = get_api_provider("absent");
            let removal = register_session_resource_cleanup(Arc::new(|_| Ok(())));
            removal.remove();
            cleanup_session_resources(None).unwrap();
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    fn reentrant_provider(
        api: &str,
        drops: &Arc<std::sync::atomic::AtomicUsize>,
    ) -> maestro_models::records::api_registry::ApiProvider {
        use maestro_models::records::api_registry::*;
        let probe = ReenterDrop(Arc::clone(drops));
        ApiProvider {
            api: api.into(),
            stream: Arc::new(move |_, _, _| {
                let _probe = &probe;
                assert!(get_api_provider("reentrant").is_some());
                register_api_provider(provider("added", 5.0), None);
                Ok(final_stream(1.0))
            }),
            stream_simple: Arc::new(|_, _, _| Ok(final_stream(2.0))),
        }
    }
    #[test]
    fn maestro_callback_destruction_can_reenter() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::stream;
        let _serial = GLOBAL.lock().unwrap();
        clear_api_providers();
        let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        register_api_provider(reentrant_provider("reentrant", &drops), None);
        assert_eq!(
            ready(
                stream(model("reentrant"), context(), None)
                    .unwrap()
                    .result()
            )
            .read()
            .unwrap()
            .timestamp
            .to_bits(),
            1.0_f64.to_bits()
        );
        register_api_provider(provider("reentrant", 3.0), None);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        register_api_provider(reentrant_provider("remove", &drops), Some("source".into()));
        unregister_api_providers("source");
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 2);
        register_api_provider(reentrant_provider("clear", &drops), None);
        clear_api_providers();
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 3);
        let probe = ReenterDrop(Arc::clone(&drops));
        let removal = register_session_resource_cleanup(Arc::new(move |_| {
            let _probe = &probe;
            let removal = register_session_resource_cleanup(Arc::new(|_| Ok(())));
            removal.remove();
            Ok(())
        }));
        cleanup_session_resources(None).unwrap();
        removal.remove();
        drop(removal);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 4);
    }
}
