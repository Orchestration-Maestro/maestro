//! Owned constructor capture through a suspended payload callback and public preparation.
#![cfg(test)]
use crate::chat::{self, TestResult};
use maestro_models::providers::content::{
    google::create_client, google_shared::project_content_request,
};
use maestro_models::{OnPayload, PayloadHookInput};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Mutate borrowed constructor inputs after capture, then resume before preparing a request.
pub async fn assert_owned_constructor() -> TestResult {
    let mut model = chat::model(&json!({"baseUrl":"https://captured.invalid/v1"}))?;
    let mut key = String::from("captured-key");
    let mut headers = indexmap::IndexMap::from([("x-capture".into(), "before".into())]);
    let client = create_client(&model, &key, Some(&headers), None);
    let (entered, admission) = tokio::sync::oneshot::channel();
    let (release, held) = tokio::sync::oneshot::channel();
    let state = Arc::new(Mutex::new(Some((entered, held))));
    let hook: OnPayload = Arc::new(move |input, _| {
        let (entered, held) = state.lock().unwrap().take().unwrap();
        Box::pin(async move {
            entered.send(()).unwrap();
            held.await.unwrap();
            Ok(input)
        })
    });
    let payload = PayloadHookInput {
        payload: json!({"model":"captured-model","contents":"prompt"}),
        signal: None,
    };
    let call = hook(payload, Arc::new(model.clone()));
    let mut call = std::pin::pin!(call);
    assert!(
        std::future::Future::poll(
            call.as_mut(),
            &mut std::task::Context::from_waker(std::task::Waker::noop())
        )
        .is_pending()
    );
    admission.await?;
    model.base_url = "https://decoy.invalid/v9".into();
    key.clear();
    key.push_str("decoy-key");
    headers.insert("x-capture".into(), "after".into());
    release.send(()).unwrap();
    let selected = call.await?;
    let request = client
        .prepare_request(project_content_request(selected.payload)?, None)
        .await?
        .request;
    assert_eq!(
        request.url,
        "https://captured.invalid/v1/models/captured-model:streamGenerateContent?alt=sse"
    );
    assert!(
        request
            .headers
            .get("x-goog-api-key")
            .is_some_and(|value| value == "captured-key")
    );
    assert_eq!(request.headers["x-capture"], "before");
    Ok(())
}
