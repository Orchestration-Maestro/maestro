//! Provider invocation ownership through the public SDK.

use maestro_models::{
    Local, Model, OnPayload, OnResponse, SimpleStreamOptions, StreamOptions, build_base_options_for,
};
use std::{cell::Cell, rc::Rc};

#[test]
fn base_options_share_hooks_without_calling_them() {
    let model: Model = serde_json::from_value(serde_json::json!({
        "id": "controlled", "name": "Controlled", "api": "controlled",
        "provider": "controlled", "baseUrl": "https://fixture.invalid",
        "reasoning": false, "input": ["text"],
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
        "contextWindow": 100, "maxTokens": 40000
    }))
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    let payload_calls = Rc::clone(&calls);
    let payload: OnPayload<Local> = Rc::new(move |input, _| {
        payload_calls.set(payload_calls.get() + 1);
        Box::pin(async move { Ok(input) })
    });
    let response_calls = Rc::clone(&calls);
    let response: OnResponse<Local> = Rc::new(move |_, _| {
        response_calls.set(response_calls.get() + 1);
        Box::pin(async { Ok(()) })
    });
    for (with_payload, with_response) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let options = SimpleStreamOptions::<Local> {
            common: StreamOptions {
                on_payload: with_payload.then(|| Rc::clone(&payload)),
                on_response: with_response.then(|| Rc::clone(&response)),
                api_key: Some("original".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let prepared = build_base_options_for(&model, Some(&options), Some("selected"));
        assert_eq!(prepared.on_payload.is_some(), with_payload);
        assert_eq!(prepared.on_response.is_some(), with_response);
        if let Some(actual) = prepared.on_payload {
            assert!(Rc::ptr_eq(&actual, &payload));
        }
        if let Some(actual) = prepared.on_response {
            assert!(Rc::ptr_eq(&actual, &response));
        }
        assert_eq!(prepared.api_key.as_deref(), Some("selected"));
        assert_eq!(prepared.max_tokens, Some(32000.0));
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(
        maestro_models::build_base_options(&model, None, None).max_tokens,
        Some(32000.0)
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn default_sdk_futures_remain_thread_safe() {
    use maestro_models::{
        ApiProvider, Context, FauxResponseFactory, ProviderStreamOptions, SimpleStreamOptions,
        StreamOptionsInput,
    };
    use std::sync::Arc;
    fn shared<T: Send + Sync>() {}
    fn send<T: Send>(_: T) {}
    // A contextual default literal still coerces hook closures without a family annotation.
    fn default_options(options: StreamOptions) -> StreamOptions {
        options
    }
    shared::<StreamOptions>();
    shared::<ProviderStreamOptions>();
    shared::<SimpleStreamOptions>();
    shared::<StreamOptionsInput>();
    shared::<ApiProvider>();
    shared::<FauxResponseFactory>();
    let common = default_options(StreamOptions {
        on_payload: Some(Arc::new(|input, _| Box::pin(async move { Ok(input) }))),
        on_response: Some(Arc::new(|_, _| Box::pin(async { Ok(()) }))),
        ..Default::default()
    });
    assert!(common.on_payload.is_some());
    assert!(common.on_response.is_some());
    let inferred_with_expected_type: StreamOptions = StreamOptions {
        on_payload: Some(Arc::new(|input, _| Box::pin(async move { Ok(input) }))),
        ..Default::default()
    };
    assert!(inferred_with_expected_type.on_payload.is_some());
    send(async move {
        std::future::ready(()).await;
        let model: Model = serde_json::from_value(serde_json::json!({
            "id": "controlled", "name": "Controlled", "api": "controlled",
            "provider": "controlled", "baseUrl": "https://fixture.invalid",
            "reasoning": false, "input": ["text"],
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
            "contextWindow": 100, "maxTokens": 10
        }))
        .unwrap();
        let context = Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        };
        let _ = maestro_models::stream(model.clone(), context.clone(), None);
        let _ = maestro_models::stream_simple(model.clone(), context.clone(), None);
        let _ = maestro_models::complete(model.clone(), context.clone(), None).await;
        let _ = maestro_models::complete_simple(model, context, None).await;
    });
}
