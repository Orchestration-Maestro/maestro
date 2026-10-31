#[cfg(test)]
mod tests {
    use crate::response_support::failure as error;
    use maestro_models::records::session_resources::{
        cleanup_session_resources, register_session_resource_cleanup,
    };
    use maestro_models::{DiagnosticErrorInfo, SharedHandle};
    use std::sync::{Arc, Mutex};

    #[test]
    fn maestro_cleanup_collects_failures_in_order() {
        let _serial = crate::bundled::registry();
        let visits = Arc::new(Mutex::new(Vec::new()));
        let removals: Vec<_> = ["first", "success", "last"]
            .into_iter()
            .map(|name| {
                let visits = Arc::clone(&visits);
                register_session_resource_cleanup(SharedHandle::new(move |session| {
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
            stream: SharedHandle::new(TimestampProvider(timestamp)),
            stream_simple: SharedHandle::new(TimestampProvider(timestamp + 1.0)),
        }
    }

    fn assert_timestamp(stream: &maestro_models::AssistantMessageEventStream, expected: f64) {
        assert_eq!(
            ready(stream.result()).read().unwrap().timestamp.to_bits(),
            expected.to_bits()
        );
    }

    #[test]
    fn explicit_protocol_keeps_the_public_model_api() {
        use maestro_models::*;
        let _serial = crate::bundled::registry();
        register_api_provider(
            ApiProvider {
                api: "selected-backend".into(),
                stream: SharedHandle::new(PublicIdentityProvider),
                stream_simple: SharedHandle::new(PublicIdentityProvider),
            },
            None,
        );
        let stream = stream_simple_via_for::<Local>(
            "selected-backend",
            model("public-api"),
            context(),
            None,
            std::rc::Rc::new(|_| panic!("the fake backend needs no producer")),
        )
        .unwrap();
        assert_eq!(ready(stream.result()).read().unwrap().api, "public-api");
        clear_api_providers();
    }

    /// Replacement protocol that preserves its caller's model identity.
    struct PublicIdentityProvider;
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for PublicIdentityProvider
    {
        type Return = Result<maestro_models::AssistantMessageEventStream, DiagnosticErrorInfo>;
        fn invoke(&self, call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            assert_eq!(call.model.api, "public-api");
            let stream = final_stream(0.0);
            ready(stream.result()).write().unwrap().api = call.model.api;
            Ok(stream)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Invoke a retained default-family callback without changing its result observer.
    fn assert_retained_timestamp<K: maestro_models::StreamOptionsFamily>(
        callback: &maestro_models::StreamFunction<K>,
        expected: f64,
    ) -> crate::chat::TestResult {
        let stream = callback.call(
            model("separate"),
            context(),
            maestro_models::ProviderInvocation::Threaded(None, None),
        )?;
        assert_timestamp(&stream, expected);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn assert_cloud_selection() -> crate::chat::TestResult {
        for (call, expected) in [
            (
                maestro_models::stream(model("bedrock-converse-stream"), context(), None)?,
                81.0_f64,
            ),
            (
                maestro_models::stream_simple(model("bedrock-converse-stream"), context(), None)?,
                82.0,
            ),
        ] {
            assert_timestamp(&call, expected);
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn maestro_platform_stores_keep_distinct_owners() -> crate::chat::TestResult {
        use maestro_models::records::api_registry::*;
        use maestro_models::{BedrockProviderModule, get_oauth_provider, reset_oauth_providers};
        let _serial = crate::bundled::registry();
        if !maestro_test_process::selection().is_child() {
            let name = "maestro_platform_stores_keep_distinct_owners";
            return {
                let output = maestro_test_process::TestProcess::current(name)?
                    .nocapture()
                    .env_clear()
                    .case("owners")
                    .output()?;
                assert!(output.status.success(), "{output:?}");
                Ok(())
            };
        }
        let visits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&visits);
        let removal = register_session_resource_cleanup(SharedHandle::new(move |session| {
            recorded.lock().unwrap().push(session.map(str::to_owned));
            Ok(())
        }));
        register_api_provider(provider("separate", 71.0), None);
        let retained = get_api_provider("separate").unwrap();
        let cloud = provider("bedrock-converse-stream", 81.0);
        maestro_models::set_bedrock_provider_module(BedrockProviderModule {
            stream_bedrock: cloud.stream,
            stream_simple_bedrock: cloud.stream_simple,
        });
        reset_oauth_providers();
        let account = get_oauth_provider("anthropic").unwrap();
        assert_retained_timestamp(&retained.stream, 71.0)?;
        cleanup_session_resources(Some("before"))?;
        clear_api_providers();
        assert!(Arc::ptr_eq(
            &account,
            &get_oauth_provider("anthropic").unwrap()
        ));
        assert_retained_timestamp(&retained.stream_simple, 72.0)?;
        cleanup_session_resources(Some("cleared"))?;
        maestro_models::reset_api_providers();
        assert!(Arc::ptr_eq(
            &account,
            &get_oauth_provider("anthropic").unwrap()
        ));
        assert_cloud_selection()?;
        removal.remove();
        cleanup_session_resources(Some("removed"))?;
        assert_eq!(
            *visits.lock().unwrap(),
            [Some("before".into()), Some("cleared".into())]
        );
        assert_retained_timestamp(&retained.stream, 71.0)?;
        Ok(())
    }

    fn assert_retained_provider(retained: &maestro_models::records::api_registry::ApiProvider) {
        assert_retained_families(&retained.stream, 20.0);
        assert_retained_families(&retained.stream_simple, 21.0);
    }

    /// Source removal retains both raw/simple handles in either invocation family.
    fn assert_retained_families<K: maestro_models::StreamOptionsFamily>(
        callback: &maestro_models::StreamFunction<K>,
        expected: f64,
    ) {
        let spawn: maestro_models::ProviderSpawner<maestro_models::Local> =
            std::rc::Rc::new(|_| panic!("synchronous retained callback must not submit"));
        for invocation in [
            maestro_models::ProviderInvocation::Local(None, spawn),
            maestro_models::ProviderInvocation::Threaded(None, None),
        ] {
            let stream = callback.call(model("2"), context(), invocation).unwrap();
            assert_timestamp(&stream, expected);
        }
    }

    #[test]
    fn maestro_dispatch_preserves_registration_order() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::{stream, stream_simple};
        let _serial = crate::bundled::registry();
        clear_api_providers();
        for (api, stamp) in [("10", 10.0), ("2", 20.0), ("01", 30.0)] {
            register_api_provider(provider(api, stamp), Some("old".into()));
        }
        let retained = get_api_provider("2").unwrap();
        register_api_provider(provider("2", 40.0), Some("new".into()));
        assert_eq!(crate::bundled::registered(), ["10", "2", "01"]);
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
        assert_eq!(crate::bundled::registered(), ["2"]);
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
        let _serial = crate::bundled::registry();
        clear_api_providers();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        register_api_provider(
            ApiProvider {
                api: "expected".into(),
                stream: SharedHandle::new(CountProvider(Arc::clone(&calls), 1.0)),
                stream_simple: SharedHandle::new(CountProvider(Arc::clone(&calls), 2.0)),
            },
            None,
        );
        let provider = get_api_provider("expected").unwrap();
        for result in [
            provider.stream.call(
                model("wrong"),
                context(),
                maestro_models::ProviderInvocation::Threaded(None, None),
            ),
            provider.stream_simple.call(
                model("wrong"),
                context(),
                maestro_models::ProviderInvocation::Threaded(None, None),
            ),
        ] {
            let failure = result.err().unwrap();
            assert_eq!(failure.message, "Mismatched api: wrong expected expected");
            assert_eq!(failure.name.as_deref(), Some("Error"));
        }
        assert_local_wrong_models(&provider);
        assert_threaded_wrong_models(&provider);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        clear_api_providers();
    }

    /// Guard rejection precedes local callback and task admission.
    fn assert_local_wrong_models(provider: &maestro_models::ApiProvider) {
        let spawn: maestro_models::ProviderSpawner<maestro_models::Local> =
            std::rc::Rc::new(|_| panic!("mismatch must not submit local work"));
        for failure in [
            provider.stream.call(
                model("wrong"),
                context(),
                maestro_models::ProviderInvocation::Local(None, spawn.clone()),
            ),
            provider.stream_simple.call(
                model("wrong"),
                context(),
                maestro_models::ProviderInvocation::Local(None, spawn),
            ),
        ] {
            assert_mismatch(failure);
        }
    }
    /// Supplied threaded admission is inert when the model API is wrong.
    fn assert_threaded_wrong_models(provider: &maestro_models::ApiProvider) {
        let spawn: maestro_models::ProviderSpawner<maestro_models::Threaded> =
            SharedHandle::new(|_| panic!("mismatch must not submit threaded work"));
        for failure in [
            provider.stream.call(
                model("wrong"),
                context(),
                maestro_models::ProviderInvocation::Threaded(None, Some(spawn.clone())),
            ),
            provider.stream_simple.call(
                model("wrong"),
                context(),
                maestro_models::ProviderInvocation::Threaded(None, Some(spawn)),
            ),
        ] {
            assert_mismatch(failure);
        }
    }
    /// Check exact checked-callback failure without observing a stream.
    fn assert_mismatch(
        result: Result<maestro_models::AssistantMessageEventStream, DiagnosticErrorInfo>,
    ) {
        let failure = result.err().unwrap();
        assert_eq!(failure.message, "Mismatched api: wrong expected expected");
        assert_eq!(failure.name.as_deref(), Some("Error"));
    }

    fn assert_missing_api() {
        use maestro_models::records::stream::*;
        assert_missing_api_for::<maestro_models::Local>(std::rc::Rc::new(|_| {
            panic!("missing API must not submit")
        }));
        assert_missing_api_for::<maestro_models::Threaded>(SharedHandle::new(|_| {
            panic!("missing API must not submit")
        }));
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
    }

    /// All mode-aware lookup failures retain their ordinary synchronous/eager boundary.
    fn assert_missing_api_for<E: maestro_models::ProviderExecution>(
        spawn: maestro_models::ProviderSpawner<E>,
    ) {
        use maestro_models::{complete_for, complete_simple_for, stream_for, stream_simple_for};
        let raw = complete_for::<E>(model("missing"), context(), None, spawn.clone());
        let simple = complete_simple_for::<E>(model("missing"), context(), None, spawn.clone());
        for failure in [
            stream_for::<E>(model("missing"), context(), None, spawn.clone())
                .err()
                .unwrap(),
            stream_simple_for::<E>(model("missing"), context(), None, spawn)
                .err()
                .unwrap(),
            ready(raw).unwrap_err(),
            ready(simple).unwrap_err(),
        ] {
            assert_eq!(
                failure.message,
                "No API provider registered for api: missing"
            );
            assert_eq!(failure.name.as_deref(), Some("Error"));
        }
    }

    #[test]
    fn maestro_dispatch_reports_missing_api() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::*;
        let _serial = crate::bundled::registry();
        clear_api_providers();
        assert_missing_api();
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
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let raw_calls = Arc::clone(&calls);
        let simple_calls = Arc::clone(&calls);
        register_api_provider(
            ApiProvider {
                api: "failure".into(),
                stream: SharedHandle::new(FailedProvider(raw, raw_calls)),
                stream_simple: SharedHandle::new(FailedProvider(simple, simple_calls)),
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
        let raw = complete(model("failure"), context(), None);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
        let simple = complete_simple(model("failure"), context(), None);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert_eq!(ready(raw).unwrap_err(), failure);
        assert_eq!(ready(simple).unwrap_err(), failure);
        assert_setup_failure_for::<maestro_models::Local>(
            std::rc::Rc::new(|_| panic!("setup failure must not submit")),
            &failure,
            &calls,
        );
        assert_setup_failure_for::<maestro_models::Threaded>(
            SharedHandle::new(|_| panic!("setup failure must not submit")),
            &failure,
            &calls,
        );
        clear_api_providers();
        assert_missing_api();
    }

    /// Setup errors preserve the original diagnostic and call immediately in both families.
    fn assert_setup_failure_for<E: maestro_models::ProviderExecution>(
        spawn: maestro_models::ProviderSpawner<E>,
        failure: &DiagnosticErrorInfo,
        calls: &std::sync::atomic::AtomicUsize,
    ) {
        use maestro_models::{complete_for, complete_simple_for, stream_for, stream_simple_for};
        let before = calls.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            &stream_for::<E>(model("failure"), context(), None, spawn.clone())
                .err()
                .unwrap(),
            failure
        );
        assert_eq!(
            &stream_simple_for::<E>(model("failure"), context(), None, spawn.clone())
                .err()
                .unwrap(),
            failure
        );
        let raw = complete_for::<E>(model("failure"), context(), None, spawn.clone());
        let simple = complete_simple_for::<E>(model("failure"), context(), None, spawn);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), before + 4);
        assert_eq!(&ready(raw).unwrap_err(), failure);
        assert_eq!(&ready(simple).unwrap_err(), failure);
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
            on_payload: Some(SharedHandle::new(|payload, _| {
                Box::pin(async { Ok(payload) })
            })),
            on_response: Some(SharedHandle::new(|_, _| Box::pin(async { Ok(()) }))),
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
            fetch: Some(Arc::new(|_| {
                Box::pin(async { Err(maestro_models::FetchError::Aborted) })
            })),
        }
    }
    fn check_common<E: maestro_models::ProviderExecution>(
        actual: &maestro_models::records::types::StreamOptions<E>,
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
        assert!(std::ptr::addr_eq(
            &raw const **actual.on_payload.as_ref().unwrap(),
            &raw const **expected.on_payload.as_ref().unwrap()
        ));
        assert!(std::ptr::addr_eq(
            &raw const **actual.on_response.as_ref().unwrap(),
            &raw const **expected.on_response.as_ref().unwrap()
        ));
        actual.signal.as_ref().unwrap().abort();
        assert!(expected.signal.as_ref().unwrap().is_aborted());
    }
    fn check_fetch<E: maestro_models::ProviderExecution>(
        actual: &maestro_models::records::types::StreamOptions<E>,
        expected: &maestro_models::records::types::StreamOptions,
    ) {
        assert!(Arc::ptr_eq(
            actual.fetch.as_ref().unwrap(),
            expected.fetch.as_ref().unwrap()
        ));
    }
    fn completion_provider(
        output: &maestro_models::records::types::AssistantMessageEventStream,
        expected: &maestro_models::records::types::StreamOptions,
        count: &Arc<std::sync::atomic::AtomicUsize>,
    ) -> maestro_models::records::api_registry::ApiProvider {
        use maestro_models::records::api_registry::ApiProvider;
        ApiProvider {
            api: "custom".into(),
            stream: SharedHandle::new(CompleteRaw {
                output: output.clone(),
                expected: expected.clone(),
                count: Arc::clone(count),
            }),
            stream_simple: SharedHandle::new(CompleteSimple {
                output: output.clone(),
                expected: expected.clone(),
                count: Arc::clone(count),
            }),
        }
    }

    #[test]
    fn maestro_completion_observes_without_consuming() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::{complete, complete_simple};
        use maestro_models::records::types::*;
        let _serial = crate::bundled::registry();
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
                ..ProviderStreamOptions::default()
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
                tool_choice: Some(ToolChoice::Required),
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
        let on_payload: OnPayload = SharedHandle::new(
            move |maestro_models::PayloadHookInput {
                      payload,
                      signal: hook_signal,
                  },
                  descriptor| {
                payload_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(*descriptor, model("hooks"));
                Box::pin(async move {
                    match payload["mode"].as_str().unwrap() {
                        "replace" => Ok(maestro_models::PayloadHookInput {
                            payload: serde_json::json!({"stamp":2}),
                            signal: hook_signal,
                        }),
                        "payload_error" => Err(error("payload failure")),
                        _ => Ok(maestro_models::PayloadHookInput {
                            payload,
                            signal: hook_signal,
                        }),
                    }
                })
            },
        );
        let on_response: OnResponse = SharedHandle::new(move |response, descriptor| {
            response_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(*descriptor, model("hooks"));
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
        ApiProvider {
            api: "hooks".into(),
            stream: SharedHandle::new(HookProvider),
            stream_simple: SharedHandle::new(TimestampProvider(0.0)),
        }
    }

    #[test]
    fn maestro_hooks_keep_replacements_and_failures() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::stream;
        use maestro_models::records::types::*;
        let _serial = crate::bundled::registry();
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
                    on_payload: Some(SharedHandle::clone(&on_payload)),
                    on_response: Some(SharedHandle::clone(&on_response)),
                    ..StreamOptions::default()
                },
                extra: JsonObject::new(),
                ..ProviderStreamOptions::default()
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
            let removal = register_session_resource_cleanup(SharedHandle::new(|_| Ok(())));
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
            stream: SharedHandle::new(ReentrantProvider { _probe: probe }),
            stream_simple: SharedHandle::new(TimestampProvider(2.0)),
        }
    }
    #[test]
    fn maestro_callback_destruction_can_reenter() {
        use maestro_models::records::api_registry::*;
        use maestro_models::records::stream::stream;
        let _serial = crate::bundled::registry();
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
        let removal = register_session_resource_cleanup(SharedHandle::new(move |_| {
            let _probe = &probe;
            let removal = register_session_resource_cleanup(SharedHandle::new(|_| Ok(())));
            removal.remove();
            Ok(())
        }));
        cleanup_session_resources(None).unwrap();
        removal.remove();
        drop(removal);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    /// Retained invocation owner for the controlled provider.
    struct TimestampProvider(f64);
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for TimestampProvider
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            Ok(final_stream(self.0))
        }
    }

    /// Retained invocation owner for the controlled provider.
    struct CountProvider(Arc<std::sync::atomic::AtomicUsize>, f64);
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for CountProvider
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(final_stream(self.1))
        }
    }

    /// Retained invocation owner for the controlled provider.
    struct FailedProvider(DiagnosticErrorInfo, Arc<std::sync::atomic::AtomicUsize>);
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for FailedProvider
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(self.0.clone())
        }
    }

    /// Retained invocation owner for the controlled provider.
    struct CompleteRaw {
        output: maestro_models::AssistantMessageEventStream,
        expected: maestro_models::StreamOptions,
        count: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl<E: maestro_models::ProviderExecution>
        maestro_models::Invoke<
            E,
            maestro_models::ProviderCall<E, maestro_models::ProviderStreamOptions<E>>,
        > for CompleteRaw
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(
            &self,
            call: maestro_models::ProviderCall<E, maestro_models::ProviderStreamOptions<E>>,
        ) -> Self::Return {
            let maestro_models::ProviderCall {
                model: descriptor,
                context: conversation,
                options,
                ..
            } = call;

            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(descriptor, model("custom"));
            assert_eq!(conversation, context());
            let options = options.unwrap();
            check_common(&options.common, &self.expected);
            check_fetch(&options.common, &self.expected);
            assert_eq!(
                options.extra,
                serde_json::json!({"providerSpecific":"present","maxTokens":999})
                    .as_object()
                    .unwrap()
                    .clone()
            );
            Ok(self.output.clone())
        }
    }

    /// Retained invocation owner for the controlled provider.
    struct CompleteSimple {
        output: maestro_models::AssistantMessageEventStream,
        expected: maestro_models::StreamOptions,
        count: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl<E: maestro_models::ProviderExecution>
        maestro_models::Invoke<
            E,
            maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
        > for CompleteSimple
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(
            &self,
            call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
        ) -> Self::Return {
            use maestro_models::*;
            let maestro_models::ProviderCall {
                model: descriptor,
                context: conversation,
                options,
                ..
            } = call;

            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(descriptor, model("custom"));
            assert_eq!(conversation, context());
            let options = options.unwrap();
            check_common(&options.common, &self.expected);
            check_fetch(&options.common, &self.expected);
            assert_eq!(options.tool_choice, Some(ToolChoice::Required));
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
            Ok(self.output.clone())
        }
    }

    /// Retained invocation owner for the controlled provider.
    struct HookProvider;
    impl<E: maestro_models::ProviderExecution>
        maestro_models::Invoke<
            E,
            maestro_models::ProviderCall<E, maestro_models::ProviderStreamOptions<E>>,
        > for HookProvider
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(
            &self,
            call: maestro_models::ProviderCall<E, maestro_models::ProviderStreamOptions<E>>,
        ) -> Self::Return {
            use maestro_models::*;
            let maestro_models::ProviderCall {
                model: descriptor,
                context: conversation,
                options,
                ..
            } = call;

            let options = options.unwrap().common;
            let mode = conversation.system_prompt.unwrap();
            let payload = serde_json::json!({"mode":mode,"stamp":1});
            let descriptor = Arc::new(descriptor);
            let submitted = ready(options.on_payload.unwrap()(
                PayloadHookInput {
                    payload,
                    signal: None,
                },
                Arc::clone(&descriptor),
            ))?;
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
            Ok(final_stream(submitted.payload["stamp"].as_f64().unwrap()))
        }
    }

    /// Retained invocation owner for the controlled provider.
    struct ReentrantProvider {
        _probe: ReenterDrop,
    }
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for ReentrantProvider
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            use maestro_models::records::api_registry::*;
            assert!(get_api_provider("reentrant").is_some());
            register_api_provider(provider("added", 5.0), None);
            Ok(final_stream(1.0))
        }
    }
}
