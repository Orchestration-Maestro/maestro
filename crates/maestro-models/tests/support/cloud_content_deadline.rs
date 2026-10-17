//! Native authentication remains alive while consuming the request's deadline.
#![cfg(test)]
use crate::{chat, content_credentials, loopback};
use maestro_models::{StopReason, StreamOptions};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// Hold a real workforce refresh, then observe expiration or an unaffected neighbor.
pub async fn delayed_refresh(expired: bool) -> chat::TestResult {
    let (entered, admission) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    let gate = Mutex::new(Some((entered, held)));
    let server = loopback::serve_routed(move |request| {
        assert_eq!(request.request_line, "POST /refresh HTTP/1.1");
        let (entered, held) = gate.lock().unwrap().take().unwrap();
        entered.send(()).unwrap();
        held.recv().unwrap();
        loopback::Reply {
            status: 200,
            content_type: "application/json",
            headers: &[],
            close: false,
            body: br#"{"access_token":"fixture-access","expires_in":3600}"#.to_vec(),
        }
    })?;
    let credentials = content_credentials::workforce(&format!("{}/refresh", server.url), "quota");
    std::fs::write(
        std::env::var("GOOGLE_APPLICATION_CREDENTIALS")?,
        serde_json::to_vec(&credentials)?,
    )?;
    let calls = Arc::new(AtomicUsize::new(0));
    let caller = maestro_models::Cancellation::new();
    let stream = invocation(Arc::clone(&calls), caller.clone())?;
    tokio::select! {
        ready = admission => ready?,
        output = stream.result() => panic!("refresh not reached: {:?}", output.read().unwrap().error_message),
    }
    tokio::time::pause();
    if expired {
        tokio::time::advance(std::time::Duration::from_millis(1001)).await;
    }
    let mut result = std::pin::pin!(stream.result());
    assert!(
        std::future::Future::poll(
            result.as_mut(),
            &mut std::task::Context::from_waker(std::task::Waker::noop())
        )
        .is_pending()
    );
    release.send(())?;
    let output = result.await;
    let output = output.read().unwrap();
    assert_eq!(
        output.stop_reason,
        if expired {
            StopReason::Error
        } else {
            StopReason::Stop
        }
    );
    if expired {
        assert_eq!(output.error_message.as_deref(), Some("Request was aborted"));
    }
    assert_eq!(calls.load(Ordering::SeqCst), usize::from(!expired));
    assert!(!caller.is_aborted());
    server.finish()?;
    Ok(())
}

/// Start the cloud entry with a successful content transport and a selected deadline.
fn invocation(
    calls: Arc<AtomicUsize>,
    caller: maestro_models::Cancellation,
) -> chat::TestResult<maestro_models::AssistantMessageEventStream> {
    let (mut model, context) = (
        chat::model(&json!({"baseUrl":"https://native.invalid/v1"}))?,
        chat::context(&json!({"messages":[{"role":"user","content":"prompt","timestamp":1}]}))?,
    );
    model.api = "google-vertex".into();
    model.provider = "google-vertex".into();
    Ok(maestro_models::stream_google_vertex(
        model,
        context,
        Some(maestro_models::GoogleVertexOptions {
            project: Some("project".into()),
            location: Some("global".into()),
            common: StreamOptions {
                signal: Some(caller),
                on_payload: Some(Arc::new(|mut input, _| {
                    input.payload["config"]["httpOptions"] = json!({"timeout":1000});
                    Box::pin(async { Ok(input) })
                })),
                fetch: Some(Arc::new(move |request| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(request.headers["authorization"], "Bearer fixture-access");
                    Box::pin(async {
                        Ok(crate::content_generation_invocation::native::empty_response())
                    })
                })),
                ..StreamOptions::default()
            },
            ..Default::default()
        }),
    ))
}
