//! Pending body reads release admission without discarding the retained transport.
use super::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};

#[test]
fn cancelled_pending_body_read_retains_the_stream() {
    crate::tests::io_runtime().block_on(async {
        let ready = Arc::new(AtomicBool::new(false));
        let producer = ready.clone();
        let mut bytes = Some(vec![0, 255, 226, 130, 172]);
        let response = Http::new(
            HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: BTreeMap::new(),
                body: Box::pin(futures_util::stream::poll_fn(move |_| {
                    if producer.load(Ordering::SeqCst) {
                        Poll::Ready(bytes.take().map(Ok))
                    } else {
                        Poll::Pending
                    }
                })),
            },
            None,
        );
        let body = response.body().unwrap();
        let alias = response.body().unwrap();
        drop(response);
        let mut reading = body.read();
        assert!(
            reading
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert_eq!(alias.read().await.unwrap_err(), "body already in use");
        drop(reading);
        ready.store(true, Ordering::SeqCst);
        assert_eq!(
            alias.read().await.unwrap(),
            Some(vec![0, 255, 226, 130, 172])
        );
        assert_eq!(body.read().await.unwrap(), None);
        body.close().unwrap();
    });
}
