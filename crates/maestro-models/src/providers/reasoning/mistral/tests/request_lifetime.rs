//! Controlled-clock request and body lifetime witnesses.

use super::super::request;
use futures_util::StreamExt;
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

/// A body whose producer observes teardown through channel closure.
struct ChannelBody(tokio::sync::mpsc::UnboundedReceiver<Result<Vec<u8>, crate::FetchError>>);

impl futures_core::Stream for ChannelBody {
    type Item = Result<Vec<u8>, crate::FetchError>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(context)
    }
}

/// Assert pending at the controlled operation boundary without a delay.
async fn pending<T>(mut operation: Pin<&mut impl Future<Output = T>>) {
    poll_fn(|context| {
        assert!(operation.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
}

/// Prepared operation with a synthetic credential and supplied transport.
async fn prepared(fetch: crate::Fetch, signal: Option<crate::Cancellation>) -> request::Prepared {
    let model = super::request_corpus::model(&serde_json::json!({}));
    let context = serde_json::from_value(serde_json::json!({"messages":[]})).unwrap();
    let options = crate::MistralOptions {
        common: crate::StreamOptions {
            api_key: Some("synthetic".into()),
            signal,
            fetch: Some(fetch),
            ..Default::default()
        },
        ..Default::default()
    };
    request::prepare(Arc::new(model), &context, &options, None)
        .await
        .unwrap()
}

/// Headers with a controlled body.
fn response(
    receiver: tokio::sync::mpsc::UnboundedReceiver<Result<Vec<u8>, crate::FetchError>>,
) -> crate::HttpResponse {
    crate::HttpResponse {
        status: 200,
        status_text: String::new(),
        headers: std::collections::BTreeMap::from([(
            "content-type".into(),
            "text/event-stream".into(),
        )]),
        body: Box::pin(ChannelBody(receiver)),
    }
}

/// Transport whose completion is explicitly released by the test producer.
fn delayed_fetch() -> (
    crate::Fetch,
    tokio::sync::oneshot::Sender<crate::HttpResponse>,
) {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let slot = Arc::new(Mutex::new(Some(receiver)));
    let fetch = Arc::new(move |_| {
        let receiver = slot.lock().unwrap().take().unwrap();
        Box::pin(async move { Ok(receiver.await.unwrap()) }) as crate::BoxFuture<_>
    });
    (fetch, sender)
}

/// Check the setup deadline and supplied-signal cancellation paths.
async fn setup_lifetime() {
    for supplied in [false, true] {
        let signal = supplied.then(crate::Cancellation::new);
        let (fetch, producer) = delayed_fetch();
        let mut send = Box::pin(request::send(prepared(fetch, signal.clone()).await));
        pending(send.as_mut()).await;
        tokio::time::advance(Duration::from_secs(31)).await;
        if let Some(signal) = signal {
            pending(send.as_mut()).await;
            signal.abort();
        }
        let error = ready(send.as_mut()).await.err().unwrap().message;
        assert!(
            error.starts_with(if supplied {
                "Request aborted by client"
            } else {
                "Request timed out"
            }),
            "{error}"
        );
        assert!(
            producer.is_closed(),
            "completed send must drop pending client work"
        );
    }
}

/// The header completion and each chunk consume the same deadline.
async fn body_lifetime() {
    body_deadline(false).await;
    body_deadline(true).await;
    immediate_headers().await;
}

/// Caller signals disable the deadline, then end the same body on abort.
async fn supplied_body_and_teardown() {
    let signal = crate::Cancellation::new();
    let (fetch, producer) = delayed_fetch();
    let mut send = Box::pin(request::send(prepared(fetch, Some(signal.clone())).await));
    pending(send.as_mut()).await;
    let (chunks, receiver) = tokio::sync::mpsc::unbounded_channel();
    producer.send(response(receiver)).ok().unwrap();
    let mut body = send.await.unwrap();
    let mut next = Box::pin(body.next());
    pending(next.as_mut()).await;
    tokio::time::advance(Duration::from_secs(60)).await;
    pending(next.as_mut()).await;
    signal.abort();
    assert_eq!(next.await, Some(Err(crate::FetchError::Aborted)));
    assert!(body.next().await.is_none());
    chunks.closed().await;
    teardown().await;
}

/// Time awaiting a hook is outside the request lifetime, even for an aborted caller.
async fn hook_lifetime() {
    for aborted in [false, true] {
        let (release, wait) = tokio::sync::oneshot::channel();
        let slot = Arc::new(Mutex::new(Some(wait)));
        let mut options = crate::MistralOptions::default();
        options.common.api_key = Some("synthetic".into());
        let signal = crate::Cancellation::new();
        if aborted {
            signal.abort();
            options.common.signal = Some(signal);
        }
        options.common.on_payload = Some(Arc::new(move |payload, _| {
            let wait = slot.lock().unwrap().take().unwrap();
            Box::pin(async move {
                wait.await.unwrap();
                Ok(payload)
            })
        }));
        let (fetch, producer) = delayed_fetch();
        options.common.fetch = Some(fetch);
        let context = serde_json::from_value(serde_json::json!({"messages":[]})).unwrap();
        let model = Arc::new(super::request_corpus::model(&serde_json::json!({})));
        let mut prepare = Box::pin(request::prepare(model, &context, &options, None));
        pending(prepare.as_mut()).await;
        tokio::time::advance(Duration::from_secs(35)).await;
        pending(prepare.as_mut()).await;
        release.send(()).unwrap();
        let mut send = Box::pin(request::send(prepare.await.unwrap()));
        if !aborted {
            pending(send.as_mut()).await;
            tokio::time::advance(Duration::from_secs(29)).await;
            pending(send.as_mut()).await;
            tokio::time::advance(Duration::from_secs(1)).await;
        }
        assert!(ready(send.as_mut()).await.is_err());
        assert!(producer.is_closed());
        options.common.on_payload = None;
    }
}

/// All lifetime witnesses share paused time and producer-closed teardown guarantees.
pub(super) fn verify() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(async {
            setup_lifetime().await;
            body_lifetime().await;
            error_body_lifetime().await;
            supplied_body_and_teardown().await;
            hook_lifetime().await;
        });
}

/// Immediate headers arm the timer before any body read.
async fn immediate_headers() {
    // Immediate headers must arm the timer even before the first body poll.
    let (chunks, receiver) = tokio::sync::mpsc::unbounded_channel();
    let slot = Arc::new(Mutex::new(Some(response(receiver))));
    let fetch: crate::Fetch = Arc::new(move |_| {
        let response = slot.lock().unwrap().take().unwrap();
        Box::pin(async { Ok(response) })
    });
    let mut body = request::send(prepared(fetch, None).await).await.unwrap();
    tokio::time::advance(Duration::from_secs(31)).await;
    assert_eq!(
        ready(Box::pin(body.next()).as_mut()).await,
        Some(Err(crate::FetchError::Timeout))
    );
    assert!(body.next().await.is_none());
    chunks.closed().await;
}

/// EOF and drop both close the producer channel.
async fn teardown() {
    for eof in [false, true] {
        let (fetch, producer) = delayed_fetch();
        let mut send = Box::pin(request::send(prepared(fetch, None).await));
        pending(send.as_mut()).await;
        let (chunks, receiver) = tokio::sync::mpsc::unbounded_channel();
        producer.send(response(receiver)).ok().unwrap();
        let mut body = send.await.unwrap();
        if eof {
            drop(chunks);
            assert!(body.next().await.is_none());
            assert!(body.next().await.is_none());
        } else {
            drop(body);
            chunks.closed().await;
        }
    }
}

/// Headers and a late chunk preserve the original lifetime.
async fn body_deadline(late_chunk: bool) {
    let (fetch, producer) = delayed_fetch();
    let mut send = Box::pin(request::send(prepared(fetch, None).await));
    pending(send.as_mut()).await;
    let (chunks, receiver) = tokio::sync::mpsc::unbounded_channel();
    producer.send(response(receiver)).ok().unwrap();
    let mut body = send.await.unwrap();
    tokio::time::advance(Duration::from_secs(20)).await;
    if late_chunk {
        chunks.send(Ok(b"late".to_vec())).unwrap();
        assert_eq!(body.next().await.unwrap().unwrap(), b"late");
    }
    let mut next = Box::pin(body.next());
    pending(next.as_mut()).await;
    tokio::time::advance(Duration::from_secs(10)).await;
    assert_eq!(
        ready(next.as_mut()).await,
        Some(Err(crate::FetchError::Timeout))
    );
    assert!(body.next().await.is_none(), "same body ends after timeout");
    chunks.closed().await;
}

/// Assert completion at the chosen clock boundary without advancing time implicitly.
async fn ready<T>(mut operation: Pin<&mut impl Future<Output = T>>) -> T {
    poll_fn(|context| match operation.as_mut().poll(context) {
        Poll::Ready(value) => Poll::Ready(value),
        Poll::Pending => panic!("operation did not complete at the original deadline"),
    })
    .await
}

/// The same deadline also terminates a pending matched error body.
async fn error_body_lifetime() {
    let (fetch, producer) = delayed_fetch();
    let mut send = Box::pin(request::send(prepared(fetch, None).await));
    pending(send.as_mut()).await;
    let (chunks, receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut error_response = response(receiver);
    error_response.status = 500;
    producer.send(error_response).ok().unwrap();
    pending(send.as_mut()).await;
    tokio::time::advance(Duration::from_secs(30)).await;
    assert_eq!(
        ready(send.as_mut()).await.err().unwrap().message,
        "Request timed out."
    );
    chunks.closed().await;
}
