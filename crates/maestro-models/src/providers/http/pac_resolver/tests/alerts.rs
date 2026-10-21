//! Script diagnostics observed through a scoped tracing subscriber.

use std::sync::{Arc, Mutex};

/// Exact message fields delivered to the caller's subscriber.
#[derive(Clone, Default)]
struct Messages(Arc<Recording>);

/// A subscriber's destruction signals actual worker release, not sampled liveness.
#[derive(Default)]
struct Recording {
    /// Collected authored message fields.
    messages: Mutex<Vec<String>>,
    /// Optional completion signal owned by this subscriber.
    released: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl Drop for Recording {
    fn drop(&mut self) {
        if let Some(released) = self.released.get_mut().unwrap().take() {
            let _ = released.send(());
        }
    }
}

impl tracing::Subscriber for Messages {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if event.metadata().target() == "maestro_models::pac" {
            event.record(&mut self.clone());
        }
    }
}

impl tracing::field::Visit for Messages {
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0.messages.lock().unwrap().push(value.into());
        }
    }
}

#[test]
fn pac_alerts_preserve_exact_messages_without_printing() {
    let messages = Messages::default();
    tracing::subscriber::with_default(messages.clone(), || {
        crate::tests::runtime::run(async {
            assert_eq!(
                super::expression(
                    "[alert(),alert(''),alert('hello %s\\n雪'),alert(undefined),alert(null)]"
                )
                .await,
                "[null,null,null,null,null]"
            );
        });
    });
    assert_eq!(
        *messages.0.messages.lock().unwrap(),
        ["", "", "hello %s\n雪", "", "null"]
    );
}

#[test]
fn pac_containing_source_owner_releases_interpreter() {
    use crate::providers::http::{HttpRequest, Route, configured_fetch};
    use tracing::instrument::WithSubscriber;
    crate::tests::runtime::run(async {
        let (released, mut finished) = tokio::sync::oneshot::channel();
        let subscriber = Messages(Arc::new(Recording {
            messages: Mutex::default(),
            released: Mutex::new(Some(released)),
        }));
        let proxy =
            url::Url::parse("pac+data:,function FindProxyForURL(){return 'UNKNOWN';}").unwrap();
        let fetch = configured_fetch(Route {
            proxy: Some(Arc::new(move |_| Ok(Some(proxy.clone())))),
            ..Route::default()
        });
        let request = HttpRequest {
            method: "GET".into(),
            url: "http://unused.invalid/".into(),
            headers: indexmap::IndexMap::new(),
            body: Vec::new(),
            signal: None,
        };
        let error = fetch(request)
            .with_subscriber(subscriber)
            .await
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("Failed to establish a socket"),
            "{error}"
        );
        assert_eq!(
            finished.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );
        drop(fetch);
        finished.await.unwrap();
    });
}

#[test]
fn pac_cancelled_startup_releases_queued_interpreter() {
    use tracing::instrument::WithSubscriber;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(async {
            let (started, ready) = tokio::sync::oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                started.send(()).unwrap();
                released.recv().unwrap();
            });
            ready.await.unwrap();
            let (released, finished) = tokio::sync::oneshot::channel();
            let messages = Messages(Arc::new(Recording {
                messages: Mutex::default(),
                released: Mutex::new(Some(released)),
            }));
            let mut startup = Box::pin(
                super::super::Resolver::new(
                    "alert('started');function FindProxyForURL(){return 'DIRECT'}".into(),
                )
                .with_subscriber(messages.clone()),
            );
            assert!(
                std::future::Future::poll(
                    startup.as_mut(),
                    &mut std::task::Context::from_waker(std::task::Waker::noop())
                )
                .is_pending()
            );
            drop(startup);
            release.send(()).unwrap();
            blocker.await.unwrap();
            tokio::task::spawn_blocking(|| {}).await.unwrap();
            assert!(messages.0.messages.lock().unwrap().is_empty());
            drop(messages);
            finished.await.unwrap();
        });
}
