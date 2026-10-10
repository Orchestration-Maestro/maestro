//! Locked refresh, login and provider listing through the public interface.
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
        ControlledProvider, FUTURE, RefreshBehavior, Unregister, credentials, error,
    };
    use super::support::{TempDir, block_on, is_child, run_child};
    use maestro_credentials::{
        AsyncLockUpdate, AuthCredential, AuthStorage, AuthStorageBackend, AuthStorageError,
        AuthStorageFuture, ConfigValueOperations, FileAuthStorageBackend,
        InMemoryAuthStorageBackend, LockUpdate,
    };
    use maestro_models::{
        BoxFuture, OAuthAuthInfo, OAuthCredentials, OAuthError, OAuthLoginCallbacks, OAuthPrompt,
    };
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, Weak};

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

    /// Action run at a backend boundary.
    type Hook = Mutex<Option<Box<dyn Fn() + Send + Sync>>>;

    /// Memory backend with switchable failures and boundary hooks.
    #[derive(Default)]
    struct Probe {
        /// The text store.
        inner: InMemoryAuthStorageBackend,
        /// Replacement text is refused.
        write_fails: AtomicBool,
        /// Asynchronous operations fail after acquiring, instead of reading.
        read_fails: AtomicBool,
        /// Synchronous operations (reloads and setters) fail.
        sync_fails: AtomicBool,
        /// Run after the asynchronous read, before the callback.
        in_lock: Hook,
        /// Run when a replacement is about to be stored.
        before_write: Hook,
        /// Asynchronous operations started.
        async_calls: AtomicUsize,
        /// Replacement texts stored.
        writes: Mutex<Vec<String>>,
    }

    /// Shared handle through which the storage owns a probe.
    struct Handle(Arc<Probe>);

    /// Run the hook, if any.
    fn fire(hook: &Hook) {
        if let Some(action) = hook.lock().unwrap().as_ref() {
            action();
        }
    }

    impl Probe {
        /// Refuse the replacement when writes are switched off, otherwise record it.
        fn checked(&self, next: Option<String>) -> LockUpdate {
            let Some(text) = &next else {
                return Ok(next);
            };
            if self.write_fails.load(Ordering::SeqCst) {
                return Err("write failed".into());
            }
            self.writes.lock().unwrap().push(text.clone());
            Ok(next)
        }

        /// Wrap `update` so it runs with the hooks in place.
        fn hooked<'a>(&'a self, update: AsyncLockUpdate<'a>) -> AsyncLockUpdate<'a> {
            Box::new(move |current| Box::pin(self.locked(update, current)))
        }

        /// Run `update` with the hooks in place.
        async fn locked(&self, update: AsyncLockUpdate<'_>, current: Option<String>) -> LockUpdate {
            if self.read_fails.load(Ordering::SeqCst) {
                return Err("read failed".into());
            }
            fire(&self.in_lock);
            let next = update(current).await?;
            if next.is_some() {
                fire(&self.before_write);
            }
            self.checked(next)
        }
    }

    impl AuthStorageBackend for Handle {
        fn with_lock(
            &self,
            update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
        ) -> Result<(), AuthStorageError> {
            let probe = &self.0;
            if probe.sync_fails.load(Ordering::SeqCst) {
                return Err("sync failed".into());
            }
            probe
                .inner
                .with_lock(&mut |current| probe.checked(update(current)?))
        }

        fn with_lock_async<'a>(
            &'a self,
            update: AsyncLockUpdate<'a>,
        ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
            let probe = &*self.0;
            probe.async_calls.fetch_add(1, Ordering::SeqCst);
            probe.inner.with_lock_async(probe.hooked(update))
        }
    }

    /// Storage over a probe holding `document`.
    fn setup(document: &Value) -> (Arc<Probe>, Arc<AuthStorage>) {
        let probe = Arc::new(Probe::default());
        replace_text(&probe, document);
        let storage = Arc::new(AuthStorage::from_storage(Handle(probe.clone())));
        (probe, storage)
    }

    /// Replace the stored text with `document` without going through the storage.
    fn replace_text(probe: &Probe, document: &Value) {
        probe
            .inner
            .with_lock(&mut |_| Ok(Some(serde_json::to_string_pretty(document).unwrap())))
            .unwrap();
    }

    /// Current stored text.
    fn stored_text(probe: &Probe) -> String {
        let mut text = String::new();
        probe
            .inner
            .with_lock(&mut |current| {
                current.unwrap_or_default().clone_into(&mut text);
                Ok(None)
            })
            .unwrap();
        text
    }

    /// Stored OAuth record with access token `access`.
    fn record(access: &str, expires: f64) -> Value {
        let credentials = credentials(access, expires);
        json!({
            "type": "oauth",
            "refresh": credentials.refresh,
            "access": credentials.access,
            "expires": expires,
            "extra": credentials.extra["extra"],
        })
    }

    /// Stored API key record.
    fn key_record(key: &str) -> Value {
        json!({"type": "api_key", "key": key})
    }

    /// Hook that runs `action` on the storage while it is still alive.
    fn on_storage(
        storage: &Arc<AuthStorage>,
        action: impl Fn(&AuthStorage) + Send + Sync + 'static,
    ) -> Box<dyn Fn() + Send + Sync> {
        let weak: Weak<AuthStorage> = Arc::downgrade(storage);
        Box::new(move || {
            if let Some(storage) = weak.upgrade() {
                action(&storage);
            }
        })
    }

    /// Replace the fallback resolver with one answering `fallback`.
    fn replace_fallback(storage: &AuthStorage) {
        storage.set_fallback_resolver(|_| Some("fallback".into()));
    }

    /// Request key for `provider` with the fallback enabled.
    async fn request(storage: &AuthStorage, provider: &str) -> Result<Option<String>, OAuthError> {
        storage.get_api_key(provider, true, &NoOperations).await
    }

    #[test]
    fn unexpired_tokens_extract_directly_without_refresh() {
        block_on(async {
            let provider = ControlledProvider::new("r1").register();
            let _cleanup = Unregister("r1".into());
            let document = json!({"r1": record("old", FUTURE)});
            let (probe, storage) = setup(&document);
            assert_eq!(
                request(&storage, "r1").await.unwrap().as_deref(),
                Some("old")
            );
            for (access, expected) in [("", Some("")), ("extract-fails", None)] {
                replace_text(&probe, &json!({"r1": record(access, FUTURE)}));
                storage.reload();
                let outcome = request(&storage, "r1").await;
                match expected {
                    Some(key) => assert_eq!(outcome.unwrap().as_deref(), Some(key)),
                    None => assert_eq!(outcome.unwrap_err().to_string(), "extract failed"),
                }
            }
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
            assert_eq!(probe.async_calls.load(Ordering::SeqCst), 0);
            assert!(storage.drain_errors().is_empty());
        });
    }

    #[test]
    fn locked_refresh_reloads_before_expiry_selection() {
        block_on(async {
            let provider = ControlledProvider::new("r2").register();
            let _cleanup = Unregister("r2".into());
            let malformed = json!({"type": "oauth", "refresh": "r", "access": 3, "expires": 1000});
            let cases = [
                (
                    json!({"z": key_record("z"), "r2": record("external", FUTURE)}),
                    "external",
                ),
                (json!({"other": key_record("o")}), "fallback"),
                (json!({"r2": key_record("new-api-key")}), "fallback"),
                (json!({"r2": malformed}), "fallback"),
            ];
            for (document, expected) in cases {
                let (probe, storage) = setup(&json!({"r2": record("old", 1000.0)}));
                storage.set_fallback_resolver(|_| Some("old-fallback".into()));
                probe.sync_fails.store(true, Ordering::SeqCst);
                storage.reload();
                probe.sync_fails.store(false, Ordering::SeqCst);
                replace_text(&probe, &document);
                *probe.in_lock.lock().unwrap() = Some(on_storage(&storage, replace_fallback));
                let key = request(&storage, "r2").await.unwrap();
                assert_eq!(key.as_deref(), Some(expected));
                let accepted: Vec<String> = document.as_object().unwrap().keys().cloned().collect();
                assert_eq!(storage.list(), accepted);
                assert!(probe.writes.lock().unwrap().is_empty());
                storage.set(
                    "later",
                    AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
                        key: "l".into(),
                    }),
                );
                assert_eq!(probe.writes.lock().unwrap().len(), 1);
                probe.in_lock.lock().unwrap().take();
            }
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn locked_reread_with_unextractable_valid_token_fails_without_refresh_or_write() {
        block_on(async {
            let provider = ControlledProvider::new("r2-extract").register();
            let _cleanup = Unregister("r2-extract".into());
            let (probe, storage) = setup(&json!({"r2-extract": record("old", 1000.0)}));
            replace_text(
                &probe,
                &json!({"r2-extract": record("extract-fails", FUTURE)}),
            );
            let outcome = request(&storage, "r2-extract").await;
            assert_eq!(outcome.unwrap_err().to_string(), "extract failed");
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
            assert!(probe.writes.lock().unwrap().is_empty());
            assert!(matches!(
                storage.get("r2-extract"),
                Some(AuthCredential::OAuth(adopted)) if adopted.access == "extract-fails"
            ));
        });
    }

    #[test]
    fn expired_tokens_refresh_once_and_preserve_other_records() {
        block_on(async {
            let provider = ControlledProvider::new("r3").register();
            let _cleanup = Unregister("r3".into());
            let mut document = json!({
                "zulu": key_record("z"),
                "r3": record("old", 1000.0),
                "mystery": {"type": "mystery", "list": [2, 2, 1]},
                "alpha": record("keep", FUTURE),
            });
            let (probe, storage) = setup(&document);
            assert_eq!(
                request(&storage, "r3").await.unwrap().as_deref(),
                Some("new")
            );
            document["r3"] = json!({
                "type": "oauth", "refresh": "refresh-new", "access": "new",
                "expires": 4_102_444_800_000_i64, "extra": ["x", "x", "y"],
            });
            let expected = serde_json::to_string_pretty(&document).unwrap();
            assert_eq!(stored_text(&probe), expected);
            assert_eq!(*probe.writes.lock().unwrap(), [expected]);
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
            assert_eq!(storage.list(), ["zulu", "r3", "mystery", "alpha"]);

            let empty = ControlledProvider::refreshing(
                "r3-empty",
                Box::new(|_| Box::pin(async { Ok(credentials("", FUTURE)) })),
            )
            .register();
            let _cleanup = Unregister("r3-empty".into());
            let (_, storage) = setup(&json!({"r3-empty": record("old", 1000.0)}));
            assert_eq!(
                request(&storage, "r3-empty").await.unwrap().as_deref(),
                Some("")
            );
            assert_eq!(empty.refreshes.load(Ordering::SeqCst), 1);
        });
    }

    /// Provider `openai` whose refresh fails in the provider or in key extraction when `kind`
    /// names that phase, and otherwise succeeds with access token `new`.
    fn provider_failing_in(kind: &str) -> ControlledProvider {
        let refresh: RefreshBehavior = match kind {
            "provider" => Box::new(|_| Box::pin(async { Err(error("remote detail")) })),
            "extract" => Box::new(|_| Box::pin(async { Ok(credentials("extract-fails", FUTURE)) })),
            _ => Box::new(|_| Box::pin(async { Ok(credentials("new", FUTURE)) })),
        };
        ControlledProvider::refreshing("openai", refresh)
    }

    /// Make the read, parse or write phase named `kind` fail.
    fn break_refresh(kind: &str, probe: &Probe) {
        match kind {
            "read" => probe.read_fails.store(true, Ordering::SeqCst),
            "parse" => probe
                .inner
                .with_lock(&mut |_| Ok(Some("not json".into())))
                .unwrap(),
            "write" => probe.write_fails.store(true, Ordering::SeqCst),
            _ => {}
        }
    }

    /// With `OPENAI_API_KEY` and a fallback available, each refresh phase failure ends the
    /// lookup with its own recorded error, and the stored login recovers once the phase works.
    async fn refresh_failures_block_ambient_sources() {
        let expected = [
            ("read", "read failed"),
            ("parse", ""),
            ("write", "write failed"),
            ("provider", "Failed to refresh OAuth token for openai"),
            ("extract", "extract failed"),
        ];
        for (kind, message) in expected {
            let (probe, storage) = setup(&json!({"openai": record("old", 1000.0)}));
            storage.set_fallback_resolver(|_| Some("fallback".into()));
            let _provider = provider_failing_in(kind).register();
            let _cleanup = Unregister("openai".into());
            break_refresh(kind, &probe);
            assert_eq!(request(&storage, "openai").await.unwrap(), None, "{kind}");
            let errors = storage.drain_errors();
            if kind == "parse" {
                assert_eq!(errors.len(), 2);
                assert!(errors[0].downcast_ref::<serde_json::Error>().is_some());
            } else {
                assert_eq!(errors.len(), 1, "{kind}");
                assert_eq!(errors[0].to_string(), message, "{kind}");
                assert!(matches!(
                    storage.get("openai"),
                    Some(AuthCredential::OAuth(_))
                ));
            }
            if matches!(kind, "provider" | "extract") {
                continue;
            }
            probe.read_fails.store(false, Ordering::SeqCst);
            probe.write_fails.store(false, Ordering::SeqCst);
            replace_text(&probe, &json!({"openai": record("old", 1000.0)}));
            let recovered = request(&storage, "openai").await.unwrap();
            assert_eq!(recovered.as_deref(), Some("new"), "{kind}");
        }
    }

    #[test]
    fn maestro_refresh_failure_blocks_ambient_fallback() {
        if is_child() {
            return block_on(refresh_failures_block_ambient_sources());
        }
        let dir = TempDir::new("refresh-failures");
        run_child(
            "tests::maestro_refresh_failure_blocks_ambient_fallback",
            dir.root(),
            &[("OPENAI_API_KEY", "env-key")],
        );
    }

    /// Provider whose refresh stores `recovery` (as another process would) and then fails.
    fn racing_provider(id: &str, probe: &Arc<Probe>, recovery: Value) -> Arc<ControlledProvider> {
        let probe = probe.clone();
        ControlledProvider::refreshing(
            id,
            Box::new(move |_| {
                replace_text(&probe, &recovery);
                Box::pin(async { Err(error("boom")) })
            }),
        )
        .register()
    }

    #[test]
    fn recovery_uses_only_valid_oauth_credentials() {
        block_on(async {
            let malformed =
                json!({"type": "oauth", "refresh": "r", "access": 3, "expires": FUTURE});
            let cases = [
                ("valid", record("recovered", FUTURE), Some("recovered")),
                ("expired", record("stale", 1000.0), None),
                ("non-oauth", key_record("k"), None),
                ("malformed", malformed, None),
                ("extract", record("extract-fails", FUTURE), None),
            ];
            for (name, recovery, expected) in cases {
                let id = format!("r5-{name}");
                let (probe, storage) = setup(&json!({id.clone(): record("old", 1000.0)}));
                storage.set_fallback_resolver(|_| Some("fallback".into()));
                let provider = racing_provider(&id, &probe, json!({id.clone(): recovery}));
                let _cleanup = Unregister(id.clone());
                let outcome = request(&storage, &id).await;
                match name {
                    "extract" => assert_eq!(outcome.unwrap_err().to_string(), "extract failed"),
                    _ => assert_eq!(outcome.unwrap().as_deref(), expected, "{name}"),
                }
                assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
                let errors = storage.drain_errors();
                assert_eq!(errors.len(), 1, "{name}");
                assert_eq!(
                    errors[0].to_string(),
                    format!("Failed to refresh OAuth token for {id}")
                );
            }
            let (probe, storage) = setup(&json!({"r5-missing": record("old", 1000.0)}));
            racing_provider("r5-missing", &probe, json!({}));
            let _cleanup = Unregister("r5-missing".into());
            assert_eq!(request(&storage, "r5-missing").await.unwrap(), None);
        });
    }

    #[test]
    fn failed_recovery_records_errors_without_discarding_accepted_tokens() {
        block_on(async {
            ControlledProvider::new("r6").register();
            let _cleanup = Unregister("r6".into());
            let document = json!({"r6": record("old", 1000.0)});
            let (probe, storage) = setup(&document);
            probe.sync_fails.store(true, Ordering::SeqCst);
            probe.write_fails.store(true, Ordering::SeqCst);
            assert_eq!(
                request(&storage, "r6").await.unwrap().as_deref(),
                Some("new")
            );
            let messages: Vec<String> = storage
                .drain_errors()
                .iter()
                .map(ToString::to_string)
                .collect();
            assert_eq!(messages, ["write failed", "sync failed"]);
            assert!(
                matches!(storage.get("r6"), Some(AuthCredential::OAuth(new)) if new.access == "new")
            );
        });
    }

    /// Storage over a credentials file in `dir` whose lock sidecar becomes a directory, so
    /// the operating system refuses to open it.
    fn storage_with_unopenable_lock(dir: &TempDir, document: &Value) -> (String, AuthStorage) {
        let path = dir.path("auth.json");
        std::fs::write(&path, serde_json::to_string_pretty(document).unwrap()).unwrap();
        let storage = AuthStorage::from_storage(FileAuthStorageBackend::new(&path));
        let sidecar = format!("{path}.lock");
        std::fs::remove_file(&sidecar).unwrap();
        std::fs::create_dir(&sidecar).unwrap();
        (sidecar, storage)
    }

    #[test]
    fn file_lock_failure_blocks_fallback_and_refresh_retries_after_it_clears() {
        block_on(async {
            let provider = ControlledProvider::new("r7").register();
            let _cleanup = Unregister("r7".into());
            let dir = TempDir::new("refresh-lock-failure");
            let (sidecar, storage) =
                storage_with_unopenable_lock(&dir, &json!({"r7": record("old", 1000.0)}));
            storage.set_fallback_resolver(|_| Some("fallback".into()));
            assert_eq!(request(&storage, "r7").await.unwrap(), None);
            let errors = storage.drain_errors();
            assert!(!errors.is_empty());
            for error in &errors {
                let io = error.downcast_ref::<std::io::Error>().unwrap();
                assert_eq!(io.kind(), std::io::ErrorKind::IsADirectory);
            }
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
            assert!(
                matches!(storage.get("r7"), Some(AuthCredential::OAuth(old)) if old.refresh == "refresh-old")
            );
            std::fs::remove_dir(&sidecar).unwrap();
            assert_eq!(
                request(&storage, "r7").await.unwrap().as_deref(),
                Some("new")
            );
            assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
            assert!(storage.drain_errors().is_empty());
        });
    }

    #[test]
    fn refresh_publishes_before_write_is_observed() {
        block_on(async {
            ControlledProvider::new("r8").register();
            let _cleanup = Unregister("r8".into());
            let (probe, storage) = setup(&json!({"r8": record("old", 1000.0)}));
            let observed = Arc::new(Mutex::new(Vec::new()));
            let seen = observed.clone();
            let hook = on_storage(&storage, move |storage| {
                seen.lock().unwrap().push(storage.get("r8"));
            });
            *probe.before_write.lock().unwrap() = Some(hook);
            assert_eq!(
                request(&storage, "r8").await.unwrap().as_deref(),
                Some("new")
            );
            let observed = observed.lock().unwrap();
            assert_eq!(observed.len(), 1);
            assert!(
                matches!(&observed[0], Some(AuthCredential::OAuth(new)) if new.access == "new")
            );
            probe.before_write.lock().unwrap().take();
        });
    }

    /// Request a key and run `edit` once its refresh is pending.
    async fn edited_during_refresh(
        storage: &AuthStorage,
        id: &str,
        gates: (&Arc<AtomicBool>, &Arc<AtomicBool>),
        edit: impl FnOnce(),
    ) {
        let editor = async {
            while !gates.0.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
            edit();
            gates.1.store(true, Ordering::SeqCst);
        };
        let (key, ()) = tokio::join!(request(storage, id), editor);
        assert_eq!(key.unwrap().as_deref(), Some("new"));
    }

    /// Provider ids of a stored document text, in order.
    fn stored_keys(text: &str) -> Vec<String> {
        let document: Value = serde_json::from_str(text).unwrap();
        document.as_object().unwrap().keys().cloned().collect()
    }

    #[test]
    fn concurrent_changes_survive_pending_refresh() {
        block_on(async {
            let gates = (
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            );
            gated("r9", &gates.0, &gates.1);
            let _cleanup = Unregister("r9".into());
            let document = json!({"gone": key_record("g"), "r9": record("old", 1000.0), "keep": key_record("k")});
            let (probe, storage) = setup(&document);
            edited_during_refresh(&storage, "r9", (&gates.0, &gates.1), || {
                storage.set(
                    "added",
                    AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
                        key: "a".into(),
                    }),
                );
                storage.remove("gone");
            })
            .await;
            assert_eq!(storage.list(), ["r9", "keep", "added"]);
            let text = stored_text(&probe);
            assert_eq!(stored_keys(&text), ["r9", "keep", "added"]);
            assert!(text.contains("\"access\": \"new\""));

            let gates = (
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            );
            gated("r9-file", &gates.0, &gates.1);
            let _cleanup = Unregister("r9-file".into());
            let dir = TempDir::new("pending-refresh");
            let path = dir.path("auth.json");
            std::fs::write(
                &path,
                serde_json::to_string_pretty(&json!({"r9-file": record("old", 1000.0)})).unwrap(),
            )
            .unwrap();
            let storage = AuthStorage::create(&path);
            edited_during_refresh(&storage, "r9-file", (&gates.0, &gates.1), || {
                storage.set(
                    "added",
                    AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
                        key: "a".into(),
                    }),
                );
            })
            .await;
            assert_eq!(storage.drain_errors().len(), 1);
            assert_eq!(storage.list(), ["r9-file", "added"]);
            let text = std::fs::read_to_string(&path).unwrap();
            assert_eq!(stored_keys(&text), ["r9-file", "added"]);
            assert!(text.contains("\"access\": \"new\""));
        });
    }

    #[test]
    fn reload_during_refresh_retains_latest_accepted_document() {
        block_on(async {
            let gates = (
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            );
            gated("r10", &gates.0, &gates.1);
            let _cleanup = Unregister("r10".into());
            let (probe, storage) =
                setup(&json!({"old": key_record("o"), "r10": record("old", 1000.0)}));
            edited_during_refresh(&storage, "r10", (&gates.0, &gates.1), || {
                replace_text(
                    &probe,
                    &json!({"latest": key_record("l"), "r10": record("old", 1000.0)}),
                );
                storage.reload();
            })
            .await;
            assert_eq!(storage.list(), ["latest", "r10"]);
            let text = stored_text(&probe);
            assert_eq!(stored_keys(&text), ["latest", "r10"]);
            assert!(text.contains("\"access\": \"new\""));
        });
    }

    /// Callbacks that record progress messages and answer nothing else.
    #[derive(Default)]
    struct Recording {
        /// Messages received.
        messages: Mutex<Vec<String>>,
    }

    impl OAuthLoginCallbacks for Recording {
        fn on_auth(&self, _info: OAuthAuthInfo) -> Result<(), OAuthError> {
            Ok(())
        }

        fn on_prompt(&self, _prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
            Box::pin(async { Ok(String::new()) })
        }

        fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
            self.messages.lock().unwrap().push(message.to_owned());
            Ok(())
        }
    }

    /// Register `id` with the given login outcome.
    fn login_provider(id: &str, outcome: Result<OAuthCredentials, OAuthError>) {
        let provider = ControlledProvider::new(id);
        *provider.login_result.lock().unwrap() = Some(outcome);
        provider.register();
    }

    #[test]
    fn login_stores_current_provider_result() {
        block_on(async {
            let _cleanup = Unregister("r11".into());
            login_provider("r11", Err(error("replaced implementation")));
            login_provider("r11", Ok(credentials("login", FUTURE)));
            let (probe, storage) = setup(&json!({"zulu": key_record("z")}));
            let callbacks = Arc::new(Recording::default());
            storage.login("r11", callbacks.clone()).await.unwrap();
            assert_eq!(*callbacks.messages.lock().unwrap(), ["r11"]);
            let expected = json!({"zulu": key_record("z"), "r11": {
                "type": "oauth", "refresh": "refresh-login", "access": "login",
                "expires": 4_102_444_800_000_i64, "extra": ["x", "x", "y"],
            }});
            assert_eq!(
                stored_text(&probe),
                serde_json::to_string_pretty(&expected).unwrap()
            );

            login_provider("r11", Err(error("denied")));
            let before = stored_text(&probe);
            let failure = storage.login("r11", callbacks.clone()).await.unwrap_err();
            assert_eq!(failure.to_string(), "denied");
            assert_eq!(
                (stored_text(&probe), probe.writes.lock().unwrap().len()),
                (before, 1)
            );
            assert!(storage.drain_errors().is_empty());

            login_provider("r11", Ok(credentials("again", FUTURE)));
            probe.write_fails.store(true, Ordering::SeqCst);
            storage.login("r11", callbacks.clone()).await.unwrap();
            assert!(
                matches!(storage.get("r11"), Some(AuthCredential::OAuth(again)) if again.access == "again")
            );
            assert_eq!(storage.drain_errors()[0].to_string(), "write failed");

            login_provider("r11", Ok(credentials("guarded", FUTURE)));
            probe.write_fails.store(false, Ordering::SeqCst);
            probe.sync_fails.store(true, Ordering::SeqCst);
            storage.reload();
            probe.sync_fails.store(false, Ordering::SeqCst);
            storage.login("r11", callbacks).await.unwrap();
            assert!(
                matches!(storage.get("r11"), Some(AuthCredential::OAuth(kept)) if kept.access == "guarded")
            );
            assert_eq!(probe.writes.lock().unwrap().len(), 1);
        });
    }

    #[test]
    fn unknown_login_preserves_state_and_reports_id() {
        block_on(async {
            let (probe, storage) = setup(&json!({"zulu": key_record("z")}));
            let callbacks = Arc::new(Recording::default());
            let failure = storage
                .login("missing-r12", callbacks.clone())
                .await
                .unwrap_err();
            assert_eq!(failure.to_string(), "Unknown OAuth provider: missing-r12");
            assert!(callbacks.messages.lock().unwrap().is_empty());
            assert_eq!(storage.list(), ["zulu"]);
            assert!(probe.writes.lock().unwrap().is_empty());
            assert!(storage.drain_errors().is_empty());
        });
    }

    #[test]
    fn provider_listing_keeps_live_order_and_handles() {
        let storage = AuthStorage::in_memory(maestro_credentials::AuthStorageData::default());
        let zeta = ControlledProvider::new("r13-zeta").register();
        ControlledProvider::new("r13-alpha").register();
        let _cleanup = (
            Unregister("r13-zeta".into()),
            Unregister("r13-alpha".into()),
        );
        let ours = |storage: &AuthStorage| -> Vec<maestro_models::OAuthProviderHandle> {
            storage
                .get_oauth_providers()
                .into_iter()
                .filter(|p| p.id().starts_with("r13-"))
                .collect()
        };
        let all: Vec<String> = storage
            .get_oauth_providers()
            .iter()
            .map(|p| p.id().to_owned())
            .collect();
        assert_eq!(all[..3], ["anthropic", "github-copilot", "openai-codex"]);
        let before = ours(&storage);
        assert_eq!(
            before.iter().map(|p| p.id()).collect::<Vec<_>>(),
            ["r13-zeta", "r13-alpha"]
        );
        assert!(std::ptr::addr_eq(
            Arc::as_ptr(&before[0]),
            Arc::as_ptr(&zeta)
        ));
        let replacement = ControlledProvider::new("r13-zeta").register();
        let after = ours(&storage);
        assert_eq!(
            after.iter().map(|p| p.id()).collect::<Vec<_>>(),
            ["r13-zeta", "r13-alpha"]
        );
        assert!(std::ptr::addr_eq(
            Arc::as_ptr(&after[0]),
            Arc::as_ptr(&replacement)
        ));
        assert!(std::ptr::addr_eq(
            Arc::as_ptr(&before[0]),
            Arc::as_ptr(&zeta)
        ));
        assert!(!std::ptr::addr_eq(
            Arc::as_ptr(&before[0]),
            Arc::as_ptr(&after[0])
        ));
    }
}
