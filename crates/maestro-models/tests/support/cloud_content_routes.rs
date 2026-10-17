//! Root exports and raw-option decoding through actual cloud requests.
#![cfg(test)]
use crate::{
    chat::{self, TestResult},
    content_generation_invocation::native,
};
use maestro_models::{
    GoogleVertexOptions, ProviderSpawner, ProviderStreamOptions, SimpleStreamOptions, StopReason,
    StreamOptions, Threaded, stream_google_vertex, stream_google_vertex_for,
    stream_simple_google_vertex, stream_simple_google_vertex_for,
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// Every new root function sends a distinguishable invocation, followed by registered routes.
pub async fn root_routes() -> TestResult {
    let mut inventory = crate::bundled::registered_apis();
    assert_eq!(inventory.remove(7), "google-vertex");
    assert_eq!(maestro_models::get_api_providers().len(), 9);
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let owned = Arc::clone(&tasks);
    let submit: ProviderSpawner<Threaded> = Arc::new(move |future| {
        let handle = tokio::spawn(future);
        owned.lock().unwrap().push(handle);
        true
    });
    for route in 0..6 {
        let (mut model, _) = native::native_input()?;
        model.api = "google-vertex".into();
        model.provider = "google-vertex".into();
        let context = chat::context(
            &json!({"messages":[{"role":"user","content":format!("root route {route}")}]}),
        )?;
        let (fetch, requests) = crate::bundled::fetch(b"data: {}\n\n".to_vec());
        let common = StreamOptions::<Threaded> {
            api_key: Some("fixture-key".into()),
            fetch: Some(fetch),
            ..StreamOptions::default()
        };
        let stream = route_stream(route, model, context, common, &submit)?;
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            StopReason::Stop
        );
        let recorded = crate::bundled::recorded(&requests);
        assert_eq!(recorded.len(), 1);
        let body = &recorded[0].body;
        assert_eq!(
            body["contents"][0]["parts"][0]["text"],
            format!("root route {route}")
        );
    }
    let admitted = std::mem::take(&mut *tasks.lock().unwrap());
    for task in admitted {
        task.await?;
    }
    Ok(())
}

/// Reject malformed declared scope before callbacks; consume common and raw controls together.
pub async fn raw_options() -> TestResult {
    let hooks = Arc::new(AtomicUsize::new(0));
    for (index, extra) in [json!({"project":7}), json!({"location":false}),json!({"project":[]}),json!({"location":{}}),json!({"project":null,"location":null,"thinking":{"enabled":true,"budgetTokens":0},"toolChoice":"any"})].into_iter().enumerate() {
        let (mut model,_) = native::native_input()?;
        model.api="google-vertex".into();model.provider="google-vertex".into();
        let context = chat::context(&json!({"messages":[{"role":"user","content":"raw controls"}],"tools":[{"name":"lookup","description":"find","parameters":{"type":"object"}}]}))?;
        let count = Arc::clone(&hooks);
        let (fetch, requests) = crate::bundled::fetch(b"data: {}\n\n".to_vec());
        let common = StreamOptions::<Threaded> {api_key:Some("fixture-key".into()),temperature:Some(0.125),max_tokens:Some(17.0),fetch:Some(fetch),on_payload:Some(Arc::new(move |input,_| {count.fetch_add(1,Ordering::SeqCst);Box::pin(async {Ok(input)})})),..StreamOptions::default()};
        let stream = maestro_models::stream(model,context,Some(ProviderStreamOptions {common,extra:extra.as_object().unwrap().clone(),..ProviderStreamOptions::default()}))?;
        let result=stream.result().await;
        if index < 4 {
            assert_eq!(result.read().unwrap().stop_reason,StopReason::Error);
            assert_eq!(hooks.load(Ordering::SeqCst),0);
            assert!(crate::bundled::recorded(&requests).is_empty());
        } else {
            assert_eq!(result.read().unwrap().stop_reason,StopReason::Stop);
            assert_eq!(hooks.load(Ordering::SeqCst),1);
            let requests=crate::bundled::recorded(&requests);
            assert_eq!(requests.len(),1);
            let body=&requests[0].body;
            assert_eq!(body["generationConfig"],json!({"temperature":0.125,"maxOutputTokens":17,"thinkingConfig":{"includeThoughts":true,"thinkingBudget":0}}));
            assert_eq!(body["toolConfig"]["functionCallingConfig"]["mode"],"ANY");
        }
    }
    Ok(())
}

/// Select a root or registered route without changing supplied controls.
fn route_stream(
    route: usize,
    model: maestro_models::Model,
    context: maestro_models::Context,
    common: StreamOptions,
    submit: &ProviderSpawner<Threaded>,
) -> TestResult<maestro_models::AssistantMessageEventStream> {
    Ok(match route {
        0 => stream_google_vertex(
            model,
            context,
            Some(GoogleVertexOptions {
                common,
                ..GoogleVertexOptions::default()
            }),
        ),
        1 => stream_simple_google_vertex(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..SimpleStreamOptions::default()
            }),
        ),
        2 => stream_google_vertex_for::<Threaded>(
            model,
            context,
            Some(GoogleVertexOptions {
                common,
                ..GoogleVertexOptions::default()
            }),
            submit.clone(),
        ),
        3 => stream_simple_google_vertex_for::<Threaded>(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..SimpleStreamOptions::default()
            }),
            submit.clone(),
        ),
        4 => maestro_models::stream(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                ..ProviderStreamOptions::default()
            }),
        )?,
        _ => maestro_models::stream_simple(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..SimpleStreamOptions::default()
            }),
        )?,
    })
}
