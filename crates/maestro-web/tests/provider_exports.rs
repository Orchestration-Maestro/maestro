//! Browser provider facade reaches application-owned policies.
#![cfg(test)]
use maestro_app::{DiagnosticErrorInfo, Model};
use maestro_web::{
    apply_proxy_if_needed, create_stream_fn, is_cors_error, should_use_proxy_for_provider,
};

#[test]
/// Browser proxy exports reach application.
fn browser_proxy_exports_reach_application() {
    assert!(should_use_proxy_for_provider("zai", ""));
    let model = model();
    assert_eq!(
        apply_proxy_if_needed(model.clone(), "", Some("https://proxy.test")).base_url,
        "https://proxy.test/?url=https%3A%2F%2Ffacade.test"
    );
    assert!(is_cors_error(&DiagnosticErrorInfo::error(
        "cross-origin refusal"
    )));
    let wrapper: maestro_app::StreamFn = create_stream_fn(maestro_app::SharedHandle::new(|| {
        Box::pin(async { Err(DiagnosticErrorInfo::error("facade settings")) })
    }));
    let mut invocation = wrapper(
        model,
        maestro_app::Context {
            system_prompt: None,
            messages: Vec::new(),
            tools: None,
        },
        maestro_app::SimpleStreamOptions::default(),
        maestro_app::ProviderStreamOptions::default(),
    );
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(Err(error)) = invocation.as_mut().poll(&mut context) else {
        panic!("settings failure must complete immediately")
    };
    assert_eq!(error.message, "facade settings");
}

/// Complete controlled model used by browser provider fixtures.
pub(crate) fn model() -> Model {
    serde_json::from_str(
        r#"{
  "id": "facade",
  "name": "Facade",
  "api": "unregistered-facade",
  "provider": "zai",
  "baseUrl": "https://facade.test",
  "reasoning": false,
  "input": [
    "text"
  ],
  "cost": {
    "input": 0,
    "output": 0,
    "cacheRead": 0,
    "cacheWrite": 0
  },
  "contextWindow": 8192,
  "maxTokens": 4096
}"#,
    )
    .unwrap()
}
