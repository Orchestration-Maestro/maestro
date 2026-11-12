//! Credential expiry and acquisition boundaries under controlled clocks and waits.
#![cfg(not(target_arch = "wasm32"))]
use super::AuthStorage;
use super::types::{AuthCredential, AuthStorageData, stored_record};
use crate::ConfigValueOperations;
use maestro_models::{
    BoxFuture, DiagnosticErrorInfo, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface, register_oauth_provider, unregister_oauth_provider,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../../../tests/support/runtime.rs"]
mod runtime;
use runtime::block_on;

/// Operations that never resolve anything.
struct Nothing;

impl ConfigValueOperations for Nothing {
    fn execute(&self, _command: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Provider whose key is the access token; its refresh succeeds only when `refreshes_to` is set.
struct Counting {
    /// Registered identifier.
    id: &'static str,
    /// Refresh calls made.
    refreshes: AtomicUsize,
    /// Access token returned by a refresh, or `None` to fail it.
    refreshes_to: Option<&'static str>,
}

impl OAuthProviderInterface for Counting {
    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &str {
        self.id
    }

    fn login(
        &self,
        _callbacks: OAuthCallbacks,
        _fetch: Option<maestro_models::Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Err(failure("no login")) })
    }

    fn refresh_token(
        &self,
        _credentials: OAuthCredentials,
        _fetch: Option<maestro_models::Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        let outcome = self
            .refreshes_to
            .map(|access| credentials(access, 4_102_444_800_000.0))
            .ok_or_else(|| failure("refused"));
        Box::pin(async move { outcome })
    }

    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        Ok(std::borrow::Cow::Borrowed(&credentials.access))
    }
}

/// Error carrying only a message.
fn failure(message: &str) -> OAuthError {
    DiagnosticErrorInfo {
        name: None,
        message: message.to_owned(),
        stack: None,
        code: None,
    }
    .into()
}

/// OAuth tokens with the given access token and expiry.
fn credentials(access: &str, expires: f64) -> OAuthCredentials {
    OAuthCredentials {
        refresh: "refresh".to_owned(),
        access: access.to_owned(),
        expires,
        extra: serde_json::Map::new(),
    }
}

/// Memory storage holding one OAuth record for `id`, with the clock fixed at 1000 ms.
fn storage_at_1000(id: &str, access: &str, expires: f64) -> AuthStorage {
    let mut data = AuthStorageData::new();
    data.insert(
        id.to_owned(),
        AuthCredential::OAuth(credentials(access, expires)),
    );
    AuthStorage::in_memory(data).with_clock(|| 1000.0)
}

/// Register a provider and return the shared handle.
fn register(provider: Counting) -> Arc<Counting> {
    let provider = Arc::new(provider);
    register_oauth_provider(provider.clone());
    provider
}

#[test]
fn request_expiry_equality_starts_refresh() {
    block_on(async {
        let provider = register(Counting {
            id: "unit-request",
            refreshes: AtomicUsize::new(0),
            refreshes_to: Some("refreshed"),
        });
        let storage = storage_at_1000("unit-request", "external", 5000.0);
        let stale = credentials("stale", 1000.0);
        storage.state().data.insert(
            "unit-request".to_owned(),
            stored_record(AuthCredential::OAuth(stale)),
        );
        let key = storage.get_api_key("unit-request", true, &Nothing).await;
        assert_eq!(key.unwrap().as_deref(), Some("external"));
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        unregister_oauth_provider("unit-request");
    });
}

#[test]
fn locked_expiry_equality_is_not_reused() {
    block_on(async {
        let provider = register(Counting {
            id: "unit-locked",
            refreshes: AtomicUsize::new(0),
            refreshes_to: Some("refreshed"),
        });
        let storage = storage_at_1000("unit-locked", "stored", 1000.0);
        let key = storage.get_api_key("unit-locked", true, &Nothing).await;
        assert_eq!(key.unwrap().as_deref(), Some("refreshed"));
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
        unregister_oauth_provider("unit-locked");

        register(Counting {
            id: "unit-recovery",
            refreshes: AtomicUsize::new(0),
            refreshes_to: None,
        });
        let storage = storage_at_1000("unit-recovery", "stored", 1000.0);
        let key = storage.get_api_key("unit-recovery", true, &Nothing).await;
        assert_eq!(key.unwrap(), None);
        assert_eq!(storage.drain_errors().len(), 1);
        unregister_oauth_provider("unit-recovery");
    });
}

/// Blocking waits observed on this test thread, with an optional holder to release.
#[derive(Default)]
struct WaitingRead {
    /// Delays requested by the file adapter.
    delays: Vec<std::time::Duration>,
    /// Competing lock and the document it will publish.
    holder: Option<(std::fs::File, String)>,
}

thread_local! {
    /// Each test thread owns its recording and competing file handle.
    static WAITING_READ: std::cell::RefCell<WaitingRead> = std::cell::RefCell::default();
}

/// Record a wait, publishing and releasing the competing writer on the third wait.
fn record_wait(delay: std::time::Duration) {
    WAITING_READ.with_borrow_mut(|trace| {
        trace.delays.push(delay);
        if trace.delays.len() == 3
            && let Some((holder, path)) = trace.holder.take()
        {
            std::fs::write(path, r#"{"p":{"type":"api_key","key":"fresh"}}"#).unwrap();
            drop(holder);
        }
    });
}

/// Hold the sidecar as an independent writer.
fn hold_file(path: &str) -> std::fs::File {
    let holder = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(format!("{path}.lock"))
        .unwrap();
    holder.lock().unwrap();
    holder
}

#[test]
fn reread_waits_for_the_holder_on_the_scheduled_acquisition() {
    use super::backend::FileAuthStorageBackend;
    use std::time::Duration;

    for reader in 0..3 {
        let dir = maestro_test_directory::Scratch::new().unwrap();
        let path = dir.path().join("auth.json").to_str().unwrap().to_owned();
        let backend = FileAuthStorageBackend::new(&path)
            .with_draw(|| 0.75)
            .with_wait(record_wait);
        let storage = AuthStorage::from_storage(backend);
        WAITING_READ.with_borrow_mut(|trace| {
            *trace = WaitingRead {
                delays: Vec::new(),
                holder: Some((hold_file(&path), path)),
            };
        });

        let observed = match reader {
            0 => storage
                .get("p")
                .map(|_| vec!["p".to_owned()])
                .unwrap_or_default(),
            1 => storage.list(),
            _ => storage.get_all().into_keys().collect(),
        };
        let trace = WAITING_READ.with_borrow_mut(std::mem::take);

        assert_eq!(observed, ["p"]);
        assert_eq!(trace.delays, [175, 350, 700].map(Duration::from_millis));
        assert!(trace.holder.is_none());
        assert!(storage.drain_errors().is_empty());
    }
}

#[test]
fn reread_keeps_accepted_records_after_the_last_scheduled_wait() {
    use super::backend::FileAuthStorageBackend;
    use std::time::Duration;

    let dir = maestro_test_directory::Scratch::new().unwrap();
    let path = dir.path().join("auth.json").to_str().unwrap().to_owned();
    let backend = FileAuthStorageBackend::new(&path)
        .with_draw(|| 0.75)
        .with_wait(record_wait);
    let storage = AuthStorage::from_storage(backend);
    storage.set(
        "p",
        AuthCredential::ApiKey(super::types::ApiKeyCredential {
            key: "accepted".into(),
            env: None,
        }),
    );
    let _holder = hold_file(&path);
    for reader in 0..3 {
        WAITING_READ.with_borrow_mut(|trace| *trace = WaitingRead::default());
        let observed = match reader {
            0 => storage
                .get("p")
                .map(|_| vec!["p".to_owned()])
                .unwrap_or_default(),
            1 => storage.list(),
            _ => storage.get_all().into_keys().collect(),
        };
        let trace = WAITING_READ.with_borrow_mut(std::mem::take);
        assert_eq!(observed, ["p"]);
        assert_eq!(
            trace.delays,
            [175, 350, 700, 1400, 2800, 5600, 10000, 10000, 10000, 10000]
                .map(Duration::from_millis)
        );
        assert!(storage.drain_errors().is_empty());
    }
}

/// Stored literal key for one provider.
fn api_key(key: &str) -> AuthCredential {
    AuthCredential::ApiKey(super::types::ApiKeyCredential {
        key: key.into(),
        env: None,
    })
}

#[test]
fn repeated_reads_of_an_unchanged_file_take_no_lock() {
    use super::backend::FileAuthStorageBackend;

    let dir = maestro_test_directory::Scratch::new().unwrap();
    let path = dir.path().join("auth.json").to_str().unwrap().to_owned();
    let backend = FileAuthStorageBackend::new(&path)
        .with_draw(|| 0.75)
        .with_wait(record_wait);
    let storage = AuthStorage::from_storage(backend);
    storage.set("p", api_key("accepted"));
    assert_eq!(storage.get("p"), Some(api_key("accepted")));

    let _holder = hold_file(&path);
    WAITING_READ.with_borrow_mut(|trace| *trace = WaitingRead::default());
    for _ in 0..50 {
        assert_eq!(storage.get("p"), Some(api_key("accepted")));
        assert!(storage.has("p"));
    }
    let waits = WAITING_READ.with_borrow_mut(std::mem::take).delays.len();
    assert_eq!(waits, 0);
}

#[test]
fn reread_follows_another_storage_and_in_place_edits() {
    let dir = maestro_test_directory::Scratch::new().unwrap();
    let path = dir.path().join("auth.json").to_str().unwrap().to_owned();
    let other = AuthStorage::create(&path);
    other.set("p", api_key("old-1"));
    let storage = AuthStorage::create(&path);
    assert_eq!(storage.get("p"), Some(api_key("old-1")));

    let first_write = std::fs::metadata(&path).unwrap().modified().unwrap();
    other.set("p", api_key("new-1"));
    // The write is in place at the same size; restoring the time reproduces one timestamp tick.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(first_write)
        .unwrap();
    assert_eq!(storage.get("p"), Some(api_key("new-1")));

    std::fs::write(&path, r#"{"p":{"type":"api_key","key":"edited-in-place"}}"#).unwrap();
    assert_eq!(storage.get("p"), Some(api_key("edited-in-place")));

    std::fs::remove_file(&path).unwrap();
    assert_eq!(storage.get("p"), None);
}

/// Exercise acquisition with the competing writer released just before one scheduled retry.
async fn acquire_after_release(path: &str, release_before: usize) {
    use super::backend::{AuthStorageBackend, FileAuthStorageBackend};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    let delays = [175, 350, 700, 1400, 2800, 5600, 10000, 10000, 10000, 10000];
    let mut holder = Some(hold_file(path));
    let backend = FileAuthStorageBackend::new(path).with_draw(|| 0.75);
    let calls = AtomicUsize::new(0);
    let mut operation = backend.with_lock_async(Box::new(|_| {
        Box::pin(async {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(None)
        })
    }));
    let mut context = Context::from_waker(Waker::noop());
    assert!(operation.as_mut().poll(&mut context).is_pending());
    for (index, delay) in delays.into_iter().enumerate() {
        tokio::time::advance(Duration::from_millis(delay - 1)).await;
        assert!(operation.as_mut().poll(&mut context).is_pending());
        if index == release_before {
            drop(holder.take());
        }
        tokio::time::advance(Duration::from_millis(1)).await;
        let outcome = operation.as_mut().poll(&mut context);
        if index == release_before || index == 9 {
            let Poll::Ready(result) = outcome else {
                panic!("scheduled acquisition completes");
            };
            if release_before < 10 {
                result.unwrap();
                assert_eq!(calls.load(Ordering::Relaxed), 1);
            } else {
                assert!(matches!(
                    result.unwrap_err().downcast_ref::<std::fs::TryLockError>(),
                    Some(std::fs::TryLockError::WouldBlock)
                ));
                assert_eq!(calls.load(Ordering::Relaxed), 0);
            }
            break;
        }
        assert!(outcome.is_pending());
    }
}

#[test]
fn scheduled_acquisition_lengthens_each_wait_before_the_cap() {
    runtime::runtime(true).block_on(async {
        let dir = maestro_test_directory::Scratch::new().unwrap();
        let path = dir.path().join("auth.json").to_str().unwrap().to_owned();
        for release_before in 0..=10 {
            acquire_after_release(&path, release_before).await;
        }
    });
}
