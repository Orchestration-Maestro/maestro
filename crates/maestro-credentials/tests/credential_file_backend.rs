//! Native credential file backend through the public interface.
#[cfg(test)]
mod gate_support;
#[cfg(test)]
mod oauth_support;
#[cfg(test)]
mod support;
#[cfg(test)]
mod tests {

    use super::gate_support::gated;
    use super::oauth_support::{
        ControlledProvider, FUTURE, RefreshBehavior, Unregister, credentials,
    };
    use super::support::{TempDir, block_on, block_on_paused, is_child, run_child};
    use maestro_credentials::{
        ApiKeyCredential, AsyncLockUpdate, AuthCredential, AuthStorage, AuthStorageBackend,
        ConfigValueOperations, FileAuthStorageBackend, InMemoryAuthStorageBackend, LockUpdate,
    };
    use maestro_models::{BoxFuture, OAuthCredentials, OAuthError};
    use std::fs::{File, TryLockError};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// Environment-free configuration operations.
    struct NoOperations;

    impl ConfigValueOperations for NoOperations {
        fn environment(&self, _name: &str) -> Option<String> {
            None
        }

        fn execute(&self, _command: &str) -> Option<Vec<u8>> {
            None
        }
    }

    /// Stored API key credential.
    fn api_key(key: &str) -> AuthCredential {
        AuthCredential::ApiKey(ApiKeyCredential {
            key: key.to_owned(),
        })
    }

    /// Pretty text of one API key record per entry.
    fn pretty(records: &[(&str, &str)]) -> String {
        let body: Vec<String> = records
            .iter()
            .map(|(name, key)| {
                format!(
                    "  \"{name}\": {{\n    \"type\": \"api_key\",\n    \"key\": \"{key}\"\n  }}"
                )
            })
            .collect();
        format!("{{\n{}\n}}", body.join(",\n"))
    }

    /// Mode bits of a path.
    #[cfg(unix)]
    fn mode(path: &str) -> u32 {
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path).unwrap().permissions())
            & 0o777
    }

    /// Set the mode bits of a path.
    #[cfg(unix)]
    fn set_mode(path: &str, bits: u32) {
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(bits)).unwrap();
    }

    /// Hold the exclusive lock of the sidecar for `path`.
    fn hold_lock(path: &str) -> File {
        let file = File::options()
            .create(true)
            .write(true)
            .truncate(false)
            .open(format!("{path}.lock"))
            .unwrap();
        file.lock().unwrap();
        file
    }

    #[cfg(unix)]
    #[test]
    fn missing_credentials_file_is_initialized_privately() {
        let dir = TempDir::without_forks("initialize");
        let path = dir.path("nested/agent/auth.json");
        let storage = AuthStorage::create(&path);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&dir.path("nested")), 0o700);
        assert_eq!(mode(&dir.path("nested/agent")), 0o700);
        assert!(storage.list().is_empty());
        assert!(storage.drain_errors().is_empty());
        assert!(std::path::Path::new(&format!("{path}.lock")).exists());
    }

    #[cfg(unix)]
    #[test]
    fn zero_byte_file_left_by_interrupted_creation_stays_usable() {
        let dir = TempDir::without_forks("zero-byte");
        let path = dir.path("agent/auth.json");
        std::fs::create_dir(dir.path("agent")).unwrap();
        set_mode(&dir.path("agent"), 0o755);
        std::fs::write(&path, "").unwrap();
        set_mode(&path, 0o644);
        let storage = AuthStorage::create(&path);
        assert!(storage.drain_errors().is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        assert_eq!((mode(&path), mode(&dir.path("agent"))), (0o644, 0o755));
        storage.set("openai", api_key("o"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("openai", "o")])
        );
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn relative_credentials_path_follows_working_directory() {
        if is_child() {
            let storage = AuthStorage::create("auth.json");
            storage.set("openai", api_key("o"));
            assert!(storage.drain_errors().is_empty());
            return;
        }
        let dir = TempDir::new("relative");
        run_child(
            "tests::relative_credentials_path_follows_working_directory",
            dir.root(),
            &[],
        );
        assert_eq!(
            std::fs::read_to_string(dir.path("auth.json")).unwrap(),
            pretty(&[("openai", "o")])
        );
    }

    #[test]
    fn competing_creator_cannot_truncate_locked_winner() {
        let dir = TempDir::without_forks("competing");
        let path = dir.path("auth.json");
        let holder = hold_lock(&path);
        let storage = AuthStorage::create(&path);
        let errors = storage.drain_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0].downcast_ref::<TryLockError>(),
            Some(TryLockError::WouldBlock)
        ));
        assert!(!std::path::Path::new(&path).exists());
        std::fs::write(&path, pretty(&[("winner", "w")])).unwrap();
        drop(holder);
        storage.set("late", api_key("l"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("winner", "w")])
        );
        storage.reload();
        assert_eq!(storage.list(), ["winner"]);
        storage.set("late", api_key("l"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("winner", "w"), ("late", "l")])
        );
    }

    #[test]
    fn contended_lock_retries_then_records_failure() {
        let dir = TempDir::without_forks("contended");
        let path = dir.path("auth.json");
        let storage = AuthStorage::create(&path);
        let initial = std::fs::read_to_string(&path).unwrap();
        let holder = hold_lock(&path);
        let started = Instant::now();
        storage.set("openai", api_key("o"));
        assert!(started.elapsed() >= Duration::from_millis(180));
        let errors = storage.drain_errors();
        assert!(matches!(
            errors[0].downcast_ref::<TryLockError>(),
            Some(TryLockError::WouldBlock)
        ));
        assert_eq!(storage.list(), ["openai"]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), initial);
        storage.reload();
        assert_eq!(storage.drain_errors().len(), 1);
        drop(holder);
        storage.set("google", api_key("g"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), initial);
        storage.reload();
        storage.set("google", api_key("g"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("google", "g")])
        );
    }

    #[test]
    fn lock_acquisition_failure_skips_the_callback() {
        let dir = TempDir::without_forks("acquire-failure");
        let path = dir.path("auth.json");
        std::fs::create_dir(format!("{path}.lock")).unwrap();
        let backend = FileAuthStorageBackend::new(&path);
        let result = backend.with_lock(&mut |_| panic!("callback must not run"));
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn missing_symlink_target_is_initialized_and_live_link_is_kept() {
        let dir = TempDir::without_forks("symlink");
        let target = dir.path("real.json");
        let link = dir.path("auth.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let storage = AuthStorage::create(&link);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "{}");
        assert_eq!(mode(&target), 0o600);
        assert!(storage.drain_errors().is_empty());

        let live = dir.path("live.json");
        let kept = dir.path("kept.json");
        std::fs::write(&kept, pretty(&[("a", "1")])).unwrap();
        std::os::unix::fs::symlink(&kept, &live).unwrap();
        let storage = AuthStorage::create(&live);
        assert_eq!(storage.list(), ["a"]);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            pretty(&[("a", "1")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_link_chain_creates_final_target_and_existing_file_is_kept() {
        let dir = TempDir::without_forks("chain");
        let target = dir.path("final.json");
        let middle = dir.path("middle.json");
        let link = dir.path("auth.json");
        std::os::unix::fs::symlink("final.json", &middle).unwrap();
        std::os::unix::fs::symlink("middle.json", &link).unwrap();
        let storage = AuthStorage::create(&link);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "{}");
        assert_eq!(mode(&target), 0o600);
        assert!(storage.drain_errors().is_empty());

        let winner = dir.path("winner.json");
        std::fs::write(&winner, pretty(&[("a", "1")])).unwrap();
        let storage = AuthStorage::create(&winner);
        assert_eq!(storage.list(), ["a"]);
        assert_eq!(
            std::fs::read_to_string(&winner).unwrap(),
            pretty(&[("a", "1")])
        );
    }

    #[test]
    fn callback_read_and_write_failures_release_the_lock() {
        let dir = TempDir::without_forks("release");
        let path = dir.path("auth.json");
        let backend = FileAuthStorageBackend::new(&path);
        let free = || {
            let sidecar = File::options()
                .write(true)
                .open(format!("{path}.lock"))
                .unwrap();
            assert!(sidecar.try_lock().is_ok());
        };
        let error = backend
            .with_lock(&mut |_| Err("callback failed".into()))
            .unwrap_err();
        assert_eq!(error.to_string(), "callback failed");
        free();
        let result = backend.with_lock(&mut |_| {
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            Ok(Some("text".into()))
        });
        assert!(result.is_err());
        free();
        let result = backend.with_lock(&mut |_| panic!("read failure precedes the callback"));
        assert!(result.is_err());
        free();
    }

    #[cfg(unix)]
    #[test]
    fn writes_restrict_existing_file_to_owner() {
        let dir = TempDir::without_forks("restrict");
        let path = dir.path("auth.json");
        std::fs::write(&path, pretty(&[("a", "1")])).unwrap();
        set_mode(&path, 0o644);
        let storage = AuthStorage::create(&path);
        assert_eq!(mode(&path), 0o644);
        storage.set("b", api_key("2"));
        assert_eq!(mode(&path), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn removed_or_absent_records_rewrite_the_document() {
        let dir = TempDir::without_forks("rewrite");
        let path = dir.path("auth.json");
        std::fs::write(&path, "{\"a\":{\"type\":\"api_key\",\"key\":\"1\"}}").unwrap();
        let storage = AuthStorage::create(&path);
        storage.remove("absent");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("a", "1")])
        );
        std::fs::remove_file(&path).unwrap();
        storage.set("b", api_key("2"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("b", "2")])
        );
        assert_eq!(mode(&path), 0o600);
        assert_eq!(storage.list(), ["a", "b"]);
    }

    #[test]
    fn lossy_utf8_and_integer_times_survive_rewrite() {
        let dir = TempDir::without_forks("lossy");
        let path = dir.path("auth.json");
        let mut bytes = b"{\"p\":{\"type\":\"api_key\",\"key\":\"k".to_vec();
        bytes.extend([0xFF, 0xC3]);
        bytes.extend(b"z\"},\"o\":{\"type\":\"oauth\",\"refresh\":\"r\",\"access\":\"a\",\"expires\":1730000000000,\"zeta\":1,\"alpha\":{\"y\":1,\"x\":2}}}");
        std::fs::write(&path, bytes).unwrap();
        let storage = AuthStorage::create(&path);
        assert_eq!(storage.get("p"), Some(api_key("k\u{fffd}\u{fffd}z")));
        storage.set("other", api_key("x"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"key\": \"k\u{fffd}\u{fffd}z\""));
        assert!(text.contains("\"expires\": 1730000000000,\n    \"zeta\": 1,\n    \"alpha\": {\n      \"y\": 1,\n      \"x\": 2\n    }"));
    }

    /// Text held by a memory backend.
    fn memory_text(backend: &InMemoryAuthStorageBackend) -> Option<String> {
        let mut text = None;
        backend
            .with_lock(&mut |current| {
                text = current.map(str::to_owned);
                Ok(None)
            })
            .unwrap();
        text
    }

    /// Callback that keeps the text it saw and returns `next`.
    fn replacing(
        seen: &std::sync::Mutex<Vec<Option<String>>>,
        next: LockUpdate,
    ) -> AsyncLockUpdate<'_> {
        Box::new(move |current| {
            Box::pin(async move {
                seen.lock().unwrap().push(current);
                next
            })
        })
    }

    /// Callback that expects `"a"`, waits for `wait_for`, raises `raise` and replaces the text.
    fn handshake<'a>(
        wait_for: &'a AtomicBool,
        raise: &'a AtomicBool,
        next: &'static str,
    ) -> AsyncLockUpdate<'a> {
        Box::new(move |current| Box::pin(handshake_body(current, (wait_for, raise), next)))
    }

    /// Body of [`handshake`]; a waiter that never sees its flag is a test failure by hang.
    async fn handshake_body(
        current: Option<String>,
        flags: (&AtomicBool, &AtomicBool),
        next: &str,
    ) -> LockUpdate {
        assert_eq!(current.as_deref(), Some("a"));
        flags.1.store(true, Ordering::SeqCst);
        while !flags.0.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        Ok(Some(next.to_owned()))
    }

    /// Callback that counts its runs.
    fn counting(calls: &AtomicUsize) -> AsyncLockUpdate<'_> {
        Box::new(move |_| {
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            })
        })
    }

    /// Callback that fails with `message`.
    fn failing<'a>(message: &'static str) -> AsyncLockUpdate<'a> {
        Box::new(move |_| Box::pin(async move { Err(message.into()) }))
    }

    /// Callback that turns the credential file into a directory, then asks for a write.
    fn breaking_write(path: &str) -> AsyncLockUpdate<'_> {
        Box::new(move |_| Box::pin(std::future::ready(Ok(Some(break_file(path))))))
    }

    /// Replace the file with a directory and ask for a write.
    fn break_file(path: &str) -> String {
        std::fs::remove_file(path).unwrap();
        std::fs::create_dir(path).unwrap();
        "text".into()
    }

    /// Run one memory callback over `initial` text and check the seen text, stored text and error.
    async fn memory_input_and_outcome(
        initial: Option<&str>,
        outcome: Result<Option<&str>, &'static str>,
    ) {
        let backend = InMemoryAuthStorageBackend::default();
        if let Some(text) = initial {
            let discarded = std::sync::Mutex::new(Vec::new());
            let seed = Ok(Some(text.into()));
            backend
                .with_lock_async(replacing(&discarded, seed))
                .await
                .unwrap();
        }
        let seen = std::sync::Mutex::new(Vec::new());
        let update = outcome.map(|text| text.map(Into::into)).map_err(Into::into);
        let result = backend.with_lock_async(replacing(&seen, update)).await;
        let expected_text = match outcome {
            Ok(Some(text)) => Some(text.to_owned()),
            _ => initial.map(str::to_owned),
        };
        let label = format!("{initial:?} {outcome:?}");
        assert_eq!(memory_text(&backend), expected_text, "{label}");
        assert_eq!(
            *seen.lock().unwrap(),
            [initial.map(str::to_owned)],
            "{label}"
        );
        let message = result.err().map(|error| error.to_string());
        assert_eq!(message, outcome.err().map(str::to_owned), "{label}");
    }

    #[test]
    fn async_memory_callbacks_are_unlocked_and_last_write_wins() {
        block_on(async {
            let outcomes = [Ok(None), Ok(Some("b")), Ok(Some("")), Err("refused")];
            for (initial, outcome) in [None, Some(""), Some("a")]
                .into_iter()
                .flat_map(|initial| outcomes.map(|outcome| (initial, outcome)))
            {
                memory_input_and_outcome(initial, outcome).await;
            }

            let backend = InMemoryAuthStorageBackend::default();
            backend
                .with_lock_async(replacing(
                    &std::sync::Mutex::new(Vec::new()),
                    Ok(Some("a".into())),
                ))
                .await
                .unwrap();
            let (first_waits, second_done) = (AtomicBool::new(false), AtomicBool::new(false));
            let first = backend.with_lock_async(handshake(&second_done, &first_waits, "first"));
            let second = backend.with_lock_async(handshake(&first_waits, &second_done, "second"));
            let (first, second) = tokio::join!(first, second);
            first.unwrap();
            second.unwrap();
            assert_eq!(memory_text(&backend), Some("first".into()));
        });
    }

    #[test]
    fn async_file_roundtrip_preserves_no_write_and_empty_write() {
        block_on(async {
            let dir = TempDir::without_forks("async-roundtrip");
            let path = dir.path("nested/deeper/auth.json");
            let backend = FileAuthStorageBackend::new(&path);
            let seen = std::sync::Mutex::new(Vec::new());
            backend
                .with_lock_async(replacing(&seen, Ok(None)))
                .await
                .unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
            std::fs::write(&path, "kept").unwrap();
            backend
                .with_lock_async(replacing(&seen, Ok(None)))
                .await
                .unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
            backend
                .with_lock_async(replacing(&seen, Ok(Some(String::new()))))
                .await
                .unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
            backend
                .with_lock_async(replacing(&seen, Ok(Some("text".into()))))
                .await
                .unwrap();
            assert_eq!(
                *seen.lock().unwrap(),
                [
                    Some("{}".to_owned()),
                    Some("kept".to_owned()),
                    Some("kept".to_owned()),
                    Some(String::new())
                ]
            );
            #[cfg(unix)]
            assert_eq!(mode(&path), 0o600);
        });
    }

    /// Run one async operation while another holder releases the sidecar at `release_ms`.
    async fn release_at(path: &str, release_ms: u64) -> (Result<(), bool>, Duration, usize) {
        let holder = hold_lock(path);
        let backend = FileAuthStorageBackend::new(path);
        let calls = AtomicUsize::new(0);
        let started = tokio::time::Instant::now();
        let operation = async {
            let result = backend
                .with_lock_async(counting(&calls))
                .await
                .map_err(|error| {
                    matches!(
                        error.downcast_ref::<TryLockError>(),
                        Some(TryLockError::WouldBlock)
                    )
                });
            (result, started.elapsed())
        };
        let release = async {
            tokio::time::sleep(Duration::from_millis(release_ms)).await;
            drop(holder);
        };
        let ((result, elapsed), ()) = tokio::join!(operation, release);
        (result, elapsed, calls.into_inner())
    }

    #[test]
    fn async_file_contention_obeys_bounded_schedule() {
        block_on_paused(async {
            let dir = TempDir::without_forks("async-schedule");
            let path = dir.path("auth.json");
            let deadlines = [
                100, 300, 700, 1500, 3100, 6300, 12_700, 22_700, 32_700, 42_700,
            ];
            let mut previous = 0;
            for deadline in deadlines {
                let (result, elapsed, calls) = release_at(&path, previous + 1).await;
                assert_eq!(result, Ok(()), "release at {}", previous + 1);
                assert_eq!(elapsed, Duration::from_millis(deadline));
                assert_eq!(calls, 1);
                previous = deadline;
            }
            let (result, elapsed, calls) = release_at(&path, 42_701).await;
            assert_eq!(result, Err(true));
            assert_eq!(elapsed, Duration::from_millis(42_700));
            assert_eq!(calls, 0);
        });
    }

    #[test]
    fn async_file_non_contention_errors_return_immediately() {
        block_on_paused(async {
            let dir = TempDir::without_forks("async-immediate");
            let path = dir.path("auth.json");
            std::fs::create_dir(format!("{path}.lock")).unwrap();
            let backend = FileAuthStorageBackend::new(&path);
            let started = tokio::time::Instant::now();
            let result = backend
                .with_lock_async(Box::new(|_| panic!("callback must not run")))
                .await;
            assert!(result.is_err());
            assert_eq!(started.elapsed(), Duration::ZERO);
        });
    }

    #[test]
    fn async_file_failures_release_the_lock() {
        block_on(async {
            let dir = TempDir::without_forks("async-release");
            let path = dir.path("auth.json");
            let backend = FileAuthStorageBackend::new(&path);
            let free = || {
                let sidecar = File::options()
                    .write(true)
                    .open(format!("{path}.lock"))
                    .unwrap();
                assert!(sidecar.try_lock().is_ok());
            };
            let error = backend
                .with_lock_async(failing("callback failed"))
                .await
                .unwrap_err();
            assert_eq!(error.to_string(), "callback failed");
            free();
            let result = backend.with_lock_async(breaking_write(&path)).await;
            assert!(result.is_err());
            free();
            let result = backend
                .with_lock_async(Box::new(|_| panic!("read failure precedes the callback")))
                .await;
            assert!(result.is_err());
            free();
            std::fs::remove_dir(&path).unwrap();
            backend
                .with_lock_async(replacing(&std::sync::Mutex::default(), Ok(None)))
                .await
                .unwrap();
        });
    }

    /// Poll `future` once with a waker that does nothing.
    fn poll_once<F: std::future::Future + Unpin>(future: &mut F) -> std::task::Poll<F::Output> {
        std::pin::Pin::new(future)
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    }

    /// Credential file holding one expired OAuth record for `id`.
    fn expired_file(dir: &TempDir, id: &str) -> String {
        let path = dir.path("auth.json");
        let record = serde_json::json!({id: {
            "type": "oauth", "refresh": "r", "access": "old", "expires": 1000, "extra": [],
        }});
        std::fs::write(&path, record.to_string()).unwrap();
        path
    }

    /// Whether the sidecar of `path` can be locked right now.
    fn sidecar_is_free(path: &str) -> bool {
        let sidecar = File::options()
            .write(true)
            .open(format!("{path}.lock"))
            .unwrap();
        sidecar.try_lock().is_ok()
    }

    /// Sets a flag when dropped.
    struct DropWitness(Arc<AtomicBool>);

    impl Drop for DropWitness {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// Refresh that pends forever on its first call, then succeeds; dropping the first call's
    /// future sets `dropped`.
    fn pending_first_refresh(
        started: &Arc<AtomicBool>,
        dropped: &Arc<AtomicBool>,
    ) -> RefreshBehavior {
        let (started, dropped, calls) = (started.clone(), dropped.clone(), AtomicUsize::new(0));
        Box::new(move |_| refresh_attempt(&started, &dropped, calls.fetch_add(1, Ordering::SeqCst)))
    }

    /// The refresh future for call number `call`.
    fn refresh_attempt(
        started: &AtomicBool,
        dropped: &Arc<AtomicBool>,
        call: usize,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        if call > 0 {
            return Box::pin(std::future::ready(Ok(credentials("new", FUTURE))));
        }
        started.store(true, Ordering::SeqCst);
        Box::pin(pend_with_witness(DropWitness(dropped.clone())))
    }

    /// Never completes; the witness is dropped with the future.
    async fn pend_with_witness(witness: DropWitness) -> Result<OAuthCredentials, OAuthError> {
        let _witness = witness;
        std::future::pending().await
    }

    /// A request dropped while waiting for the sidecar never runs its callback.
    async fn dropped_while_waiting() {
        let operations = NoOperations;
        let provider = ControlledProvider::new("f6-wait").register();
        let _cleanup = Unregister("f6-wait".into());
        let dir = TempDir::without_forks("pending-waiter");
        let path = expired_file(&dir, "f6-wait");
        let storage = AuthStorage::create(&path);
        let holder = hold_lock(&path);
        let idle = Arc::strong_count(&provider);
        let mut request = storage.get_api_key("f6-wait", true, &operations);
        assert!(poll_once(&mut request).is_pending());
        assert_eq!(Arc::strong_count(&provider), idle + 1);
        drop(request);
        assert_eq!(Arc::strong_count(&provider), idle);
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        let released = Arc::new(AtomicBool::new(false));
        let witness = DropWitness(released.clone());
        let backend = FileAuthStorageBackend::new(&path);
        let mut waiting = backend.with_lock_async(Box::new(move |_| {
            let _witness = &witness;
            Box::pin(async { Ok(None) })
        }));
        assert!(poll_once(&mut waiting).is_pending());
        assert!(!released.load(Ordering::SeqCst));
        drop(waiting);
        assert!(released.load(Ordering::SeqCst));
        drop(holder);
        let key = storage.get_api_key("f6-wait", true, &operations).await;
        assert_eq!(key.unwrap().as_deref(), Some("new"));
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
    }

    /// A request dropped while its provider refresh is pending releases the lock.
    async fn dropped_while_refreshing() {
        let operations = NoOperations;
        let (started, dropped) = (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let behavior = pending_first_refresh(&started, &dropped);
        let provider = ControlledProvider::refreshing("f6-refresh", behavior).register();
        let _cleanup = Unregister("f6-refresh".into());
        let dir = TempDir::without_forks("pending-refresh");
        let path = expired_file(&dir, "f6-refresh");
        let storage = AuthStorage::create(&path);
        let mut request = storage.get_api_key("f6-refresh", true, &operations);
        assert!(poll_once(&mut request).is_pending());
        assert!(started.load(Ordering::SeqCst) && !dropped.load(Ordering::SeqCst));
        assert!(!sidecar_is_free(&path));
        drop(request);
        assert!(dropped.load(Ordering::SeqCst));
        assert!(sidecar_is_free(&path));
        let key = storage.get_api_key("f6-refresh", true, &operations).await;
        assert_eq!(key.unwrap().as_deref(), Some("new"));
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 2);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"access\": \"new\""));
    }

    #[test]
    fn maestro_pending_auth_observer_releases_waiter() {
        block_on_paused(async {
            dropped_while_waiting().await;
            dropped_while_refreshing().await;
        });
    }

    /// Raise `release` once `started` is set.
    async fn release_when_started(started: &AtomicBool, release: &AtomicBool) {
        while !started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        release.store(true, Ordering::SeqCst);
    }

    #[test]
    fn async_file_contenders_reuse_completed_refresh() {
        block_on_paused(async {
            let started = Arc::new(AtomicBool::new(false));
            let release = Arc::new(AtomicBool::new(false));
            let provider = gated("f7", &started, &release);
            let _cleanup = Unregister("f7".into());
            let dir = TempDir::without_forks("contenders");
            let path = expired_file(&dir, "f7");
            let (first, second) = (AuthStorage::create(&path), AuthStorage::create(&path));
            let operations = NoOperations;
            let (a, b, ()) = tokio::join!(
                first.get_api_key("f7", true, &operations),
                second.get_api_key("f7", true, &operations),
                release_when_started(&started, &release)
            );
            assert_eq!(
                (a.unwrap().as_deref(), b.unwrap().as_deref()),
                (Some("new"), Some("new"))
            );
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
            assert!(first.drain_errors().is_empty() && second.drain_errors().is_empty());
        });
    }
}
