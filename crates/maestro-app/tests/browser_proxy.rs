//! Browser request policy exercised through application exports.
#![cfg(test)]
use maestro_app::browser::utils::proxy_utils::should_use_proxy_for_provider;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict recorded Provider case.
struct ProviderCase {
    /// Provider identifier.
    provider: String,
    /// Optional or supplied test credential.
    key: String,
    /// Recorded expected result.
    expected: bool,
}

#[test]
/// Provider proxy rules match key classes.
fn provider_proxy_rules_match_key_classes() {
    let cases: Vec<ProviderCase> =
        serde_json::from_str(include_str!("fixtures/browser_providers/providers.json")).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(
            should_use_proxy_for_provider(&case.provider, &case.key),
            case.expected,
            "case {index}"
        );
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Strict recorded Rewrite case.
struct RewriteCase {
    /// Case or model name.
    name: String,
    /// Supplied model descriptor.
    model: maestro_app::Model,
    /// Optional or supplied test credential.
    key: String,
    /// Supplied authored proxy URL.
    proxy_url: Option<String>,
    /// Recorded expected result.
    expected: maestro_app::Model,
}

#[test]
/// Proxy rewrite preserves model fields.
fn proxy_rewrite_preserves_model_fields() {
    let cases: Vec<RewriteCase> =
        serde_json::from_str(include_str!("fixtures/browser_providers/rewrite.json")).unwrap();
    for case in cases {
        assert_eq!(
            maestro_app::browser::utils::proxy_utils::apply_proxy_if_needed(
                case.model,
                &case.key,
                case.proxy_url.as_deref()
            ),
            case.expected,
            "{}",
            case.name
        );
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Strict recorded Cors case.
struct CorsCase {
    /// Case or model name.
    name: String,
    /// Supplied diagnostic message.
    message: String,
    /// Supplied diagnostic class.
    error_name: String,
    /// Recorded expected result.
    expected: bool,
}

#[test]
/// Cors classifier distinguishes failure classes.
fn cors_classifier_distinguishes_failure_classes() {
    use maestro_app::browser::utils::proxy_utils::is_cors_error;
    use maestro_app::{DiagnosticErrorInfo, FetchError};
    let cases: Vec<CorsCase> =
        serde_json::from_str(include_str!("fixtures/browser_providers/cors.json")).unwrap();
    for case in cases {
        let error = DiagnosticErrorInfo {
            name: Some(case.error_name),
            message: case.message,
            stack: None,
            code: None,
        };
        assert_eq!(is_cors_error(&error), case.expected, "{}", case.name);
        let transport = FetchError::Connection(error);
        assert_eq!(is_cors_error(&transport), case.expected, "{}", case.name);
    }
    assert!(!is_cors_error(&FetchError::Timeout));
    assert!(!is_cors_error(&FetchError::Aborted));
    assert!(is_cors_error(&std::io::Error::other("CORS failure")));
    assert!(!is_cors_error(&std::io::Error::other("Failed to fetch")));
}

#[cfg(not(target_arch = "wasm32"))]
use maestro_app::{
    AssistantMessageEventStream, Context, DiagnosticErrorInfo, Model, ProviderStreamOptions,
    SimpleStreamOptions,
};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use crate::support;
#[cfg(not(target_arch = "wasm32"))]
use support::run_io as run;

#[cfg(not(target_arch = "wasm32"))]
/// Load a complete controlled model.
fn model() -> Model {
    let cases: Vec<RewriteCase> =
        serde_json::from_str(include_str!("fixtures/browser_providers/rewrite.json")).unwrap();
    cases.into_iter().next().unwrap().model
}
#[cfg(not(target_arch = "wasm32"))]
/// Construct distinguishable invocation context.
fn context() -> Context {
    Context {
        system_prompt: Some("controlled instructions".into()),
        messages: Vec::new(),
        tools: Some(Vec::new()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Read one queued setting per invocation.
fn settings_getter(
    reads: Arc<std::sync::atomic::AtomicUsize>,
) -> maestro_app::browser::utils::proxy_utils::GetProxyUrl {
    let settings = Mutex::new(
        [
            None,
            Some(String::new()),
            Some("https://first".into()),
            Some("https://second".into()),
            Some("https://third".into()),
            Some("https://fourth".into()),
        ]
        .into_iter(),
    );
    Arc::new(move || {
        reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let setting = settings.lock().unwrap().next().unwrap();
        Box::pin(async move { Ok(setting) })
    })
}
#[cfg(not(target_arch = "wasm32"))]
/// Inputs retained by the controlled simple provider.
type Invocation = (Model, Context, AssistantMessageEventStream);
#[cfg(not(target_arch = "wasm32"))]
/// Register a uniquely owned provider and retain the exact forwarded values.
fn recording_provider(
    observations: Arc<Mutex<Vec<Invocation>>>,
    expected: Arc<Mutex<Option<SimpleStreamOptions>>>,
) {
    use maestro_models::{ApiProvider, register_api_provider};
    register_api_provider(
        ApiProvider {
            api: "browser-proxy-current".into(),
            stream: Arc::new(UnexpectedRaw),
            stream_simple: Arc::new(ProxyRecorder {
                observations,
                expected,
            }),
        },
        Some("browser-proxy-current".into()),
    );
}
#[cfg(not(target_arch = "wasm32"))]
/// Assert scalar options and identity-bearing callbacks from the same invocation.
fn forwarded_options<E: maestro_models::ProviderExecution>(
    actual: &SimpleStreamOptions<E>,
    expected: &SimpleStreamOptions,
) {
    assert_eq!(actual.common.temperature, expected.common.temperature);
    assert_eq!(actual.common.max_tokens, expected.common.max_tokens);
    assert_eq!(actual.common.session_id, expected.common.session_id);
    assert_eq!(actual.common.timeout_ms, expected.common.timeout_ms);
    assert_eq!(actual.common.max_retries, expected.common.max_retries);
    assert_eq!(actual.reasoning, expected.reasoning);
    assert_eq!(actual.tool_choice, expected.tool_choice);
    assert!(
        actual.common.api_key == expected.common.api_key,
        "credential forwarded"
    );
    assert!(
        actual.common.headers == expected.common.headers,
        "headers forwarded"
    );
    assert!(std::ptr::addr_eq(
        &raw const **actual.common.on_payload.as_ref().unwrap(),
        &raw const **expected.common.on_payload.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        actual.common.fetch.as_ref().unwrap(),
        expected.common.fetch.as_ref().unwrap()
    ));
}
#[cfg(not(target_arch = "wasm32"))]
/// Construct distinguishable simple options with retained callback handles.
fn simple_options(key: Option<&str>) -> SimpleStreamOptions {
    use maestro_models::{StreamOptions, ThinkingLevel, ToolChoice};
    SimpleStreamOptions {
        common: StreamOptions {
            api_key: key.map(str::to_owned),
            temperature: Some(0.25),
            max_tokens: Some(77.0),
            session_id: Some("session".into()),
            timeout_ms: Some(234.0),
            max_retries: Some(2.0),
            headers: Some([("x-test".into(), "kept".into())].into_iter().collect()),
            on_payload: Some(Arc::new(|payload, _| Box::pin(async move { Ok(payload) }))),
            fetch: Some(maestro_app::default_fetch()),
            ..StreamOptions::default()
        },
        reasoning: Some(ThinkingLevel::High),
        tool_choice: Some(ToolChoice::Required),
        ..SimpleStreamOptions::default()
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
/// Proxy stream reads current settings and forwards inputs.
fn proxy_stream_reads_current_settings_and_forwards_inputs() {
    use maestro_app::browser::utils::proxy_utils::create_stream_fn;
    run(async {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observations = Arc::new(Mutex::new(Vec::new()));
        let expected_options = Arc::new(Mutex::new(None));
        recording_provider(Arc::clone(&observations), Arc::clone(&expected_options));
        let wrapper = create_stream_fn(settings_getter(Arc::clone(&reads)));
        for (index, key) in [None, Some("key"), None, Some("key"), Some(""), Some("key")]
            .into_iter()
            .enumerate()
        {
            let options = simple_options(key);
            *expected_options.lock().unwrap() = Some(options.clone());
            let mut input = model();
            input.api = "browser-proxy-current".into();
            let returned = wrapper(
                input,
                context(),
                options.clone(),
                ProviderStreamOptions::default(),
            )
            .await
            .unwrap();
            assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), index + 1);
            returned.end(None);
            let (actual_model, actual_context, stream) =
                observations.lock().unwrap().pop().unwrap();
            assert!(stream.next().await.is_none());
            assert_eq!(actual_context, context());
            assert!(expected_options.lock().unwrap().is_none());
            let expected = match index { 3 => "https://second/?url=https%3A%2F%2Fapi.example%2Fv1%3Fx%3D1%26y%3Dtwo%20words%23part".to_owned(), 5 => "https://fourth/?url=https%3A%2F%2Fapi.example%2Fv1%3Fx%3D1%26y%3Dtwo%20words%23part".to_owned(), _ => model().base_url };
            assert_eq!(actual_model.base_url, expected);
            assert_eq!(
                actual_model.input,
                [
                    maestro_app::ModelInput::Text,
                    maestro_app::ModelInput::Image
                ]
            );
        }
        maestro_models::unregister_api_providers("browser-proxy-current");
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
/// Proxy stream does not dispatch after setting failure.
fn proxy_stream_does_not_dispatch_after_setting_failure() {
    run(async {
        use futures_util::FutureExt;
        use maestro_app::browser::utils::proxy_utils::create_stream_fn;
        use maestro_models::{ApiProvider, register_api_provider, unregister_api_providers};
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let simple = {
            let calls = Arc::clone(&calls);
            Arc::new(CountingProxy(calls))
        };
        register_api_provider(
            ApiProvider {
                api: "browser-proxy-failure".into(),
                stream: Arc::new(UnexpectedRaw),
                stream_simple: simple,
            },
            Some("browser-proxy-failure".into()),
        );
        let (release, receiver) = tokio::sync::oneshot::channel();
        let receiver = Arc::new(Mutex::new(Some(receiver)));
        let wrapper = create_stream_fn(Arc::new(move || {
            let receiver = receiver.lock().unwrap().take().unwrap();
            Box::pin(async move { receiver.await.unwrap() })
        }));
        let mut input = model();
        input.api = "browser-proxy-failure".into();
        let mut invocation = wrapper(
            input,
            context(),
            SimpleStreamOptions::default(),
            ProviderStreamOptions::default(),
        );
        assert!(invocation.as_mut().now_or_never().is_none());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        release
            .send(Err(DiagnosticErrorInfo::error("settings unavailable")))
            .unwrap();
        let error = invocation.await.err().unwrap();
        assert_eq!(error.message, "settings unavailable");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        unregister_api_providers("browser-proxy-failure");
    });
}

#[cfg(not(target_arch = "wasm32"))]
/// Retained invocation owner for the controlled provider.
struct UnexpectedRaw;
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for UnexpectedRaw
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        panic!("raw dispatch")
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Retained invocation owner for the controlled provider.
struct CountingProxy(Arc<std::sync::atomic::AtomicUsize>);
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for CountingProxy
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(AssistantMessageEventStream::new())
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Retained invocation owner for the controlled provider.
struct ProxyRecorder {
    observations: Arc<Mutex<Vec<Invocation>>>,
    expected: Arc<Mutex<Option<SimpleStreamOptions>>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for ProxyRecorder
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    ) -> Self::Return {
        let expected = self.expected.lock().unwrap().take().unwrap();
        forwarded_options(&call.options.unwrap(), &expected);
        let stream = AssistantMessageEventStream::new();
        self.observations
            .lock()
            .unwrap()
            .push((call.model, call.context, stream.clone()));
        Ok(stream)
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Base addresses the local recorder received, in order.
type Addresses = Arc<Mutex<Vec<String>>>;
#[cfg(not(target_arch = "wasm32"))]
/// Retained invocation owner recording only the requested address.
struct AddressRecorder(Addresses);
#[cfg(not(target_arch = "wasm32"))]
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for AddressRecorder
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        self.0.lock().unwrap().push(call.model.base_url);
        Ok(AssistantMessageEventStream::new())
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
/// The local proxy stream reads current settings per invocation and reaches the registered provider.
fn local_proxy_stream_reads_current_settings_per_invocation() {
    use maestro_app::browser::utils::proxy_utils::create_local_stream_fn;
    use maestro_models::{ApiProvider, register_api_provider, unregister_api_providers};
    run(async {
        let addresses: Addresses = Arc::default();
        register_api_provider(
            ApiProvider {
                api: "browser-proxy-local".into(),
                stream: Arc::new(UnexpectedRaw),
                stream_simple: Arc::new(AddressRecorder(Arc::clone(&addresses))),
            },
            Some("browser-proxy-local".into()),
        );
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let spawner: maestro_app::AgentSpawner<maestro_app::Local> = std::rc::Rc::new(|_| {});
        let wrapper = create_local_stream_fn(settings_getter(Arc::clone(&reads)), spawner);
        for key in [None, Some("key"), None, Some("key"), Some(""), Some("key")] {
            let mut options = maestro_models::SimpleStreamOptions::<maestro_app::Local>::default();
            options.common.api_key = key.map(str::to_owned);
            let mut input = model();
            input.api = "browser-proxy-local".into();
            wrapper(
                input,
                context(),
                options,
                maestro_models::ProviderStreamOptions::default(),
            )
            .await
            .unwrap();
        }
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 6);
        let proxied = |proxy: &str| {
            format!("{proxy}/?url=https%3A%2F%2Fapi.example%2Fv1%3Fx%3D1%26y%3Dtwo%20words%23part")
        };
        let original = model().base_url;
        assert_eq!(
            *addresses.lock().unwrap(),
            [
                original.clone(),
                original.clone(),
                original.clone(),
                proxied("https://second"),
                original,
                proxied("https://fourth"),
            ]
        );
        unregister_api_providers("browser-proxy-local");
    });
}
