//! Request defaults observed by the existing scripted provider at actual dispatch.
use super::{Model, Session};
use crate::storage_exports::adapter::Adapter;
use maestro_app::{Execution, FauxResponseInput, Invoke, Local, PromptInput, Threaded};
use maestro_web::{AgentInterface, AppStorage, ProviderKeysStore, SettingsStore};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

/// Current storage with a controlled backend and no browser persistence.
fn storage(backend: Rc<Adapter>) -> Rc<AppStorage> {
    let settings = Rc::new(SettingsStore::default());
    settings.set_backend(backend.clone());
    let keys = Rc::new(ProviderKeysStore::default());
    keys.set_backend(backend.clone());
    Rc::new(AppStorage::new(
        settings,
        keys,
        Rc::default(),
        Rc::default(),
        backend,
    ))
}
/// Replace an individual stored value, including literal null.
fn seed(backend: &Adapter, store: &str, key: &str, value: Option<Value>) {
    let mut values = backend.values.borrow_mut();
    let key = (store.into(), key.into());
    if let Some(value) = value {
        values.insert(key, value);
    } else {
        values.remove(&key);
    }
}
/// Original invocation operands, captured before producing any response events.
type Calls = Rc<RefCell<Vec<(String, Option<String>)>>>;
/// Callback retaining the original invocation observations.
struct Capture(Calls);
/// The factory has the same input in both existing execution representations.
macro_rules! capture {
    ($family:ty) => {
        impl Invoke<$family, FauxResponseInput<$family>> for Capture {
            type Return = <$family as Execution>::Future<
                'static,
                Result<maestro_app::AssistantMessage, maestro_app::DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: FauxResponseInput<$family>) -> Self::Return {
                self.0.borrow_mut().push((
                    input.model.base_url,
                    input.options.and_then(|options| match options {
                        maestro_app::StreamOptionsInput::Raw(options) => options.common.api_key,
                        maestro_app::StreamOptionsInput::Simple(options) => options.common.api_key,
                    }),
                ));
                Box::pin(async {
                    Ok(maestro_app::faux_assistant_message(
                        "response",
                        Default::default(),
                    ))
                })
            }
        }
    };
}
capture!(Local);
capture!(Threaded);
/// Real session, retained view and request observation, without a second executor.
struct Requests {
    model: Model,
    agent: Rc<Session>,
    calls: Calls,
    _view: AgentInterface,
    _scene: crate::reading_slots_support::Slots,
}
impl Requests {
    /// Bind only after both absent operation slots have been established.
    fn new() -> Self {
        let model = Model(maestro_app::register_faux_provider(
            maestro_app::RegisterFauxProviderOptions {
                provider: Some("openai-codex".into()),
                ..Default::default()
            },
        ));
        let agent = model.session();
        *agent.stream_fn.write().unwrap() = None;
        *agent.get_api_key.write().unwrap() = None;
        agent
            .state()
            .read()
            .unwrap()
            .model
            .write()
            .unwrap()
            .base_url = "https://model.test/messages".into();
        let scene = crate::reading_slots_support::Slots::new();
        let mut view = AgentInterface::new(scene.host.clone());
        view.set_session(Some(agent.clone())).unwrap();
        Self {
            model,
            agent,
            calls: Rc::default(),
            _view: view,
            _scene: scene,
        }
    }
    /// Reach the provider through the real Agent request machinery.
    async fn request(&self) {
        self.model
            .0
            .append_responses(vec![maestro_app::FauxResponseStep::Factory(Rc::new(
                Capture(self.calls.clone()),
            ))]);
        self.agent
            .prompt(PromptInput::Text {
                text: "request".into(),
                images: vec![],
            })
            .await
            .unwrap();
    }
    /// Current provider key identity is defined by the registered model.
    fn provider(&self) -> String {
        self.agent
            .state()
            .read()
            .unwrap()
            .model
            .read()
            .unwrap()
            .provider
            .clone()
    }
    /// Consume the most recent actually reached invocation.
    fn last(&self) -> (String, Option<String>) {
        self.calls.borrow().last().unwrap().clone()
    }
}
/// Defaults read current storage per reached operation, never once at binding.
/// # Panics
/// Panics if request operands or skipped reads differ from the storage contract.
#[wasm_bindgen]
pub async fn session_proxy_and_key_defaults_read_current_storage_per_request() {
    let backend = Rc::new(Adapter::default());
    maestro_web::set_app_storage(storage(backend.clone()));
    let requests = Requests::new();
    let provider = requests.provider();
    recorded_defaults(&requests, &backend).await;
    for (enabled, proxy) in [
        (json!(true), "https://one.test"),
        (json!({}), "https://two.test"),
    ] {
        seed(&backend, "settings", "proxy.enabled", Some(enabled));
        seed(&backend, "settings", "proxy.url", Some(json!(proxy)));
        seed(&backend, "provider-keys", &provider, Some(json!(proxy)));
        requests.request().await;
        assert_eq!(
            requests.last(),
            (
                format!("{proxy}/?url=https%3A%2F%2Fmodel.test%2Fmessages"),
                Some(proxy.into())
            )
        );
    }
    for enabled in [
        None,
        Some(Value::Null),
        Some(json!("")),
        Some(json!(0)),
        Some(json!(false)),
    ] {
        seed(&backend, "settings", "proxy.enabled", enabled);
        *backend.fail_key.borrow_mut() = Some("proxy.url".into());
        backend.calls.borrow_mut().clear();
        requests.request().await;
        assert_eq!(requests.last().0, "https://model.test/messages");
        assert!(
            !backend
                .calls
                .borrow()
                .iter()
                .any(|call| call[2] == "proxy.url")
        );
    }
    *backend.fail_key.borrow_mut() = None;
    for enabled in [json!("yes"), json!(1), json!([]), json!(true)] {
        seed(&backend, "settings", "proxy.enabled", Some(enabled));
        for value in [None, Some(Value::Null), Some(json!(""))] {
            seed(&backend, "settings", "proxy.url", value.clone());
            seed(&backend, "provider-keys", &provider, value.clone());
            let expected = value.and_then(|value| value.as_str().map(str::to_owned));
            let lookup = requests.agent.get_api_key.read().unwrap().clone().unwrap();
            assert_eq!(lookup(provider.clone()).await.unwrap(), expected);
            requests.request().await;
            assert_eq!(requests.last().0, "https://model.test/messages");
            assert_eq!(requests.last().1, expected.filter(|key| !key.is_empty()));
        }
    }
    replace_storage(&requests, &backend).await;
    independent_callbacks(&requests, &backend).await;
}
/// Replacing storage between requests and during enabled lookup affects the next reached read.
async fn replace_storage(requests: &Requests, old: &Rc<Adapter>) {
    let fresh = Rc::new(Adapter::default());
    seed(&fresh, "settings", "proxy.enabled", Some(json!(true)));
    seed(
        &fresh,
        "settings",
        "proxy.url",
        Some(json!("https://fresh.test")),
    );
    seed(
        &fresh,
        "provider-keys",
        &requests.provider(),
        Some(json!("fresh-key")),
    );
    maestro_web::set_app_storage(storage(fresh.clone()));
    requests.request().await;
    assert_eq!(
        requests.last(),
        (
            "https://fresh.test/?url=https%3A%2F%2Fmodel.test%2Fmessages".into(),
            Some("fresh-key".into())
        )
    );
    maestro_web::set_app_storage(storage(old.clone()));
    seed(old, "settings", "proxy.enabled", Some(json!(true)));
    seed(
        old,
        "provider-keys",
        &requests.provider(),
        Some(json!("old-key")),
    );
    let next = storage(fresh);
    *old.on_read.borrow_mut() = Some(Rc::new(move |key| {
        if key == "proxy.enabled" {
            maestro_web::set_app_storage(next.clone());
        }
    }));
    requests.request().await;
    assert_eq!(
        requests.last().0,
        "https://fresh.test/?url=https%3A%2F%2Fmodel.test%2Fmessages"
    );
    *old.on_read.borrow_mut() = None;
}
/// Caller operations can be replaced independently after binding.
async fn independent_callbacks(requests: &Requests, backend: &Rc<Adapter>) {
    maestro_web::set_app_storage(storage(backend.clone()));
    seed(backend, "settings", "proxy.enabled", Some(json!(true)));
    seed(
        backend,
        "settings",
        "proxy.url",
        Some(json!("https://default.test")),
    );
    let default_key = requests.agent.get_api_key.read().unwrap().clone();
    *requests.agent.get_api_key.write().unwrap() = Some(Rc::new(|_| {
        Box::pin(async { Ok(Some("custom-key".into())) })
    }));
    requests.request().await;
    assert_eq!(
        requests.last(),
        (
            "https://default.test/?url=https%3A%2F%2Fmodel.test%2Fmessages".into(),
            Some("custom-key".into())
        )
    );
    *requests.agent.get_api_key.write().unwrap() = default_key;
    seed(
        backend,
        "provider-keys",
        &requests.provider(),
        Some(json!("default-key")),
    );
    *requests.agent.stream_fn.write().unwrap() =
        Some(maestro_web::create_stream_fn(Rc::new(|| {
            Box::pin(async { Ok(Some("https://custom.test".into())) })
        })));
    *backend.fail_key.borrow_mut() = Some("proxy.enabled".into());
    requests.request().await;
    assert_eq!(
        requests.last(),
        (
            "https://custom.test/?url=https%3A%2F%2Fmodel.test%2Fmessages".into(),
            Some("default-key".into())
        )
    );
}
/// Reached storage and typed failures survive request dispatch and reduction.
/// # Panics
/// Panics if a failure is swallowed or converted into an empty successful request.
#[wasm_bindgen]
pub async fn session_request_failures_preserve_storage_failure_category() {
    let backend = Rc::new(Adapter::default());
    maestro_web::set_app_storage(storage(backend.clone()));
    let requests = Requests::new();
    let provider = requests.provider();
    seed(&backend, "settings", "proxy.enabled", Some(json!(true)));
    seed(&backend, "provider-keys", &provider, Some(json!("key")));
    for key in ["proxy.url", provider.as_str()] {
        *backend.fail_key.borrow_mut() = Some(key.into());
        requests.request().await;
        assert_failure(&requests, "controlled adapter read failure");
    }
    *backend.fail_key.borrow_mut() = Some("proxy.url".into());
    seed(&backend, "settings", "proxy.enabled", Some(json!(false)));
    requests.request().await;
    assert!(
        requests
            .agent
            .state()
            .read()
            .unwrap()
            .error_message()
            .is_none()
    );
    *backend.fail_key.borrow_mut() = None;
    seed(&backend, "settings", "proxy.enabled", Some(json!(true)));
    for (store, key) in [
        ("settings", "proxy.url"),
        ("provider-keys", provider.as_str()),
    ] {
        seed(&backend, "settings", "proxy.url", Some(json!("")));
        seed(&backend, store, key, Some(json!({"invalid": true})));
        requests.request().await;
        assert_failure(&requests, "invalid type");
    }
    independent_callbacks(&requests, &backend).await;
}
/// Check both reduced error text and the terminal assistant, not prompt rejection.
fn assert_failure(requests: &Requests, category: &str) {
    let state = requests.agent.state().read().unwrap();
    assert!(state.error_message().unwrap().contains(category));
    let messages = state.messages().read().unwrap();
    let maestro_app::AgentMessage::Assistant(message) = messages.last().unwrap() else {
        panic!("missing error assistant")
    };
    let message = message.read().unwrap();
    assert_eq!(message.stop_reason, maestro_app::StopReason::Error);
    assert!(message.error_message.as_ref().unwrap().contains(category));
}

/// Recorded typed storage operands reach the real request defaults and their exact read paths.
async fn recorded_defaults(requests: &Requests, backend: &Adapter) {
    let corpus: Value = serde_json::from_str(include_str!("../fixtures/transcript.json")).unwrap();
    let provider = requests.provider();
    for row in corpus["defaults"].as_array().unwrap() {
        seed(backend, "settings", "proxy.enabled", Some(json!(true)));
        seed(
            backend,
            "settings",
            "proxy.url",
            Some(json!("https://proxy.test")),
        );
        seed(backend, "provider-keys", &provider, Some(json!("key")));
        let operand = row["operand"].as_str().unwrap();
        let (store, key) = match operand {
            "enabled" => ("settings", "proxy.enabled"),
            "url" => ("settings", "proxy.url"),
            "key" => ("provider-keys", provider.as_str()),
            _ => unreachable!(),
        };
        seed(backend, store, key, row.get("input").cloned());
        backend.calls.borrow_mut().clear();
        let expected = row
            .get("output")
            .map(|value| value.as_str().unwrap().to_owned());
        if operand == "key" {
            let lookup = requests.agent.get_api_key.read().unwrap().clone().unwrap();
            assert_eq!(
                lookup(provider.clone()).await.unwrap(),
                expected,
                "{}",
                row["kind"]
            );
        } else {
            requests.request().await;
            let url = expected.map_or_else(
                || "https://model.test/messages".into(),
                |proxy| format!("{proxy}/?url=https%3A%2F%2Fmodel.test%2Fmessages"),
            );
            assert_eq!(requests.last().0, url, "{operand}: {}", row["kind"]);
        }
        let reads: Vec<String> = backend
            .calls
            .borrow()
            .iter()
            .filter(|call| call[0] == "get" && call[1] == store)
            .map(|call| {
                format!(
                    "{store}/{}",
                    if operand == "key" {
                        "provider"
                    } else {
                        call[2].as_str().unwrap()
                    }
                )
            })
            .collect();
        let expected_reads: Vec<String> = serde_json::from_value(row["reads"].clone()).unwrap();
        assert_eq!(reads, expected_reads, "{operand}: {}", row["kind"]);
    }
}
