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

#[test]
fn chat_direct_local_submission_preserves_hook_phases() -> crate::TestResult {
    use crate::{chat, transport};
    use maestro_models::{ProviderSpawner, stream_openai_completions_for};
    use serde_json::json;
    use std::cell::RefCell;
    chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            let transport = transport::transport(vec![transport::Attempt::success()]);
            let phases = Rc::new(RefCell::new(Vec::new()));
            let tasks = Rc::new(RefCell::new(Vec::new()));
            let admitted_tasks = Rc::clone(&tasks);
            let admissions = Rc::new(Cell::new(0));
            let counted = Rc::clone(&admissions);
            let spawn: ProviderSpawner<Local> = Rc::new(move |task| {
                counted.set(counted.get() + 1);
                admitted_tasks
                    .borrow_mut()
                    .push(tokio::task::spawn_local(task));
                true
            });
            let options = local_chat_options(Rc::clone(&phases), transport.fetch.clone());
            let stream = stream_openai_completions_for(
                chat::model(&json!({}))?,
                chat::context(&json!({"messages": []}))?,
                Some(options),
                spawn,
            );
            assert_eq!(admissions.get(), 1);
            assert!(phases.borrow().is_empty());
            let (events, result) = transport::drain(&stream).await?;
            let admitted = tasks.take();
            for task in admitted {
                task.await?;
            }
            assert_eq!(events, ["start", "done"]);
            assert_eq!(result.stop_reason, maestro_models::StopReason::Stop);
            assert_eq!(
                *phases.borrow(),
                [
                    "payload-before",
                    "payload-after",
                    "response-before",
                    "response-after"
                ]
            );
            let (_, body) = transport.first_request()?;
            let body: serde_json::Value = serde_json::from_slice(&body)?;
            assert_eq!(body["temperature"], json!(0.25));
            Ok(())
        }),
    )
}

/// Local hooks retain their original phase log across real suspension.
fn local_chat_options(
    phases: Rc<std::cell::RefCell<Vec<&'static str>>>,
    fetch: maestro_models::Fetch,
) -> maestro_models::OpenAICompletionsOptions<Local> {
    use serde_json::json;
    let payload_phases = Rc::clone(&phases);
    let response_phases = phases;
    maestro_models::OpenAICompletionsOptions::<Local> {
        common: StreamOptions {
            api_key: Some("fixture-key".into()),
            fetch: Some(fetch),
            on_payload: Some(Rc::new(move |mut input, _| {
                let phases = Rc::clone(&payload_phases);
                Box::pin(async move {
                    phases.borrow_mut().push("payload-before");
                    tokio::task::yield_now().await;
                    phases.borrow_mut().push("payload-after");
                    input.payload["temperature"] = json!(0.25);
                    Ok(input)
                })
            })),
            on_response: Some(Rc::new(move |response, _| {
                let phases = Rc::clone(&response_phases);
                Box::pin(async move {
                    assert_eq!(response.status.to_bits(), 200.0_f64.to_bits());
                    phases.borrow_mut().push("response-before");
                    tokio::task::yield_now().await;
                    phases.borrow_mut().push("response-after");
                    Ok(())
                })
            })),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn chat_local_simple_entries_keep_setup_and_admission_errors() -> crate::TestResult {
    use crate::{chat, transport};
    use maestro_models::{ProviderSpawner, stream_simple_openai_completions_for};
    use serde_json::json;
    let transport = transport::transport(vec![transport::Attempt::success()]);
    let phases = Rc::new(std::cell::RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let counted = Rc::clone(&admissions);
    let spawn: ProviderSpawner<Local> = Rc::new(move |_| {
        counted.set(counted.get() + 1);
        false
    });
    let model = chat::model(&json!({}))?;
    let context = chat::context(&json!({"messages": []}))?;
    let missing =
        maestro_models::providers::chat::openai_completions::stream_simple_openai_completions_for::<
            Local,
        >(model.clone(), context.clone(), None, Rc::clone(&spawn));
    let Err(error) = missing else {
        panic!("missing key must fail before a stream exists")
    };
    assert_eq!(error.message, "No API key for provider: fixture");
    assert_eq!(admissions.get(), 0);
    let options = SimpleStreamOptions {
        common: local_chat_options(Rc::clone(&phases), transport.fetch.clone()).common,
        ..Default::default()
    };
    let stream =
        stream_simple_openai_completions_for::<Local>(model, context, Some(options), spawn);
    assert_eq!(admissions.get(), 1);
    let (events, result) = chat::block_on(false, transport::drain(&stream))?;
    assert_eq!(events, ["error"]);
    assert_eq!(
        result.error_message.as_deref(),
        Some("Streaming requires a running Tokio runtime.")
    );
    assert!(phases.borrow().is_empty());
    assert_eq!(transport.attempts(), 0);
    Ok(())
}
