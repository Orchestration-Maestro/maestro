//! Request key selection through the public interface.
#[cfg(test)]
mod oauth_support;
#[cfg(test)]
mod support;
#[cfg(test)]
mod tests {
    use super::oauth_support::{ControlledProvider, Unregister};
    use super::support::{TempDir, block_on, is_child, run_child};
    use maestro_credentials::{
        AuthStorage, AuthStorageBackend, ConfigValueOperations, InMemoryAuthStorageBackend,
        ProcessConfigValueOperations, clear_config_value_cache,
    };
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Controlled environment and command output with the commands it ran.
    #[derive(Default)]
    struct Operations {
        /// Exact environment names.
        env: RefCell<HashMap<String, String>>,
        /// Output by command; an unlisted command fails.
        output: HashMap<String, Vec<u8>>,
        /// Commands executed in order.
        calls: RefCell<Vec<String>>,
    }

    impl ConfigValueOperations for Operations {
        fn environment(&self, name: &str) -> Option<String> {
            self.env.borrow().get(name).cloned()
        }

        fn execute(&self, command: &str) -> Option<Vec<u8>> {
            self.calls.borrow_mut().push(command.to_owned());
            self.output.get(command).cloned()
        }
    }

    /// Operations whose commands print the given outputs.
    fn printing(outputs: &[(&str, &str)]) -> Operations {
        Operations {
            output: outputs
                .iter()
                .map(|(command, output)| ((*command).to_owned(), output.as_bytes().to_vec()))
                .collect(),
            ..Operations::default()
        }
    }

    /// Storage holding one `api_key` record for provider `p`.
    fn storing(key: &str) -> AuthStorage {
        storage_with(&serde_json::json!({"p": {"type": "api_key", "key": key}}).to_string())
    }

    /// Key selected for provider `p`, with the fallback enabled.
    async fn key_of(
        storage: &AuthStorage,
        operations: &dyn ConfigValueOperations,
    ) -> Option<String> {
        storage.get_api_key("p", true, operations).await.unwrap()
    }

    /// Storage whose accepted document is the raw `text`.
    fn storage_with(text: &str) -> AuthStorage {
        let backend = InMemoryAuthStorageBackend::default();
        backend
            .with_lock(&mut |_| Ok(Some(text.to_owned())))
            .unwrap();
        AuthStorage::from_storage(backend)
    }

    #[test]
    fn request_keys_respect_source_priority() {
        block_on(async {
            let storage = storage_with(
                r#"{"p":{"type":"api_key","key":"!fail"},"q":{"type":"api_key","key":"stored"},"m":{"type":"api_key"}}"#,
            );
            let operations = Operations::default();
            storage.set_runtime_api_key("p", "runtime");
            storage.set_runtime_api_key("m", "runtime-over-malformed");
            storage.set_runtime_api_key("q", "");
            let key = |provider| storage.get_api_key(provider, true, &operations);
            assert_eq!(key("p").await.unwrap().as_deref(), Some("runtime"));
            assert_eq!(
                key("m").await.unwrap().as_deref(),
                Some("runtime-over-malformed")
            );
            assert!(operations.calls.borrow().is_empty());
            assert_eq!(key("q").await.unwrap().as_deref(), Some("stored"));
            storage.set_runtime_api_key("q", "runtime");
            assert_eq!(key("q").await.unwrap().as_deref(), Some("runtime"));
            storage.remove_runtime_api_key("q");
            assert_eq!(key("q").await.unwrap().as_deref(), Some("stored"));
        });
    }

    /// Configured stored values resolve as the resolver defines.
    async fn configured_values_resolve() {
        let operations = printing(&[
            ("k2-plain", "command-key"),
            ("k2-spaced", "  spaced-key \n"),
            ("k2-lines", "line1\nline2\n"),
            ("k2-marks", "\u{feff}marked\u{feff}"),
            ("k2-next-line", "\u{85}next\u{85}"),
        ]);
        operations.env.replace(HashMap::from([
            ("K2_SET".to_owned(), " padded \n".to_owned()),
            ("K2_EMPTY".to_owned(), String::new()),
            (" K2_LEADING".to_owned(), "leading".to_owned()),
        ]));
        let cases = [
            ("literal", "literal"),
            ("", ""),
            ("K2_SET", " padded \n"),
            ("K2_MISSING", "K2_MISSING"),
            ("K2_EMPTY", "K2_EMPTY"),
            (" K2_LEADING", "leading"),
            ("!k2-plain", "command-key"),
            ("!k2-spaced", "spaced-key"),
            ("!k2-lines", "line1\nline2"),
            ("!k2-marks", "marked"),
            ("!k2-next-line", "\u{85}next\u{85}"),
        ];
        for (index, (configured, expected)) in cases.into_iter().enumerate() {
            let storage = storing(configured);
            let key = key_of(&storage, &operations).await;
            assert_eq!(key.as_deref(), Some(expected), "case {index}");
        }
        let duplicate = storage_with(r#"{"p":{"type":"api_key","key":"first","key":"last"}}"#);
        let key = key_of(&duplicate, &operations).await;
        assert_eq!(key.as_deref(), Some("last"));
    }

    /// Real shell commands resolve with the same trimming and pipes.
    async fn native_commands_resolve() {
        let native = ProcessConfigValueOperations::new(|| unreachable!("Unix avoids selection"));
        for (command, expected) in [
            ("!printf 'k2-native\\n\\n'", "k2-native"),
            ("!printf 'hello world' | tr ' ' '-'", "hello-world"),
        ] {
            let storage = storing(command);
            assert_eq!(key_of(&storage, &native).await.as_deref(), Some(expected));
        }
    }

    #[test]
    fn stored_key_values_keep_resolution_semantics() {
        block_on(async {
            configured_values_resolve().await;
            native_commands_resolve().await;
        });
    }

    #[test]
    fn stored_helper_cache_spans_stores_and_clear() {
        block_on(async {
            let operations = printing(&[("k3-a", "key-a"), ("k3-b", "key-b")]);
            let (first, second) = (storing("!k3-a"), storing("!k3-a"));
            for storage in [&first, &second, &first] {
                assert_eq!(key_of(storage, &operations).await.as_deref(), Some("key-a"));
            }
            assert_eq!(*operations.calls.borrow(), ["k3-a"]);
            assert_eq!(
                key_of(&storing("!k3-b"), &operations).await.as_deref(),
                Some("key-b")
            );
            assert_eq!(*operations.calls.borrow(), ["k3-a", "k3-b"]);
            let failing = storing("!k3-fail");
            assert_eq!(key_of(&failing, &operations).await, None);
            assert_eq!(key_of(&failing, &operations).await, None);
            assert_eq!(operations.calls.borrow().len(), 3);
            clear_config_value_cache();
            assert_eq!(key_of(&first, &operations).await.as_deref(), Some("key-a"));
            assert_eq!(key_of(&failing, &operations).await, None);
            assert_eq!(operations.calls.borrow().len(), 5);
            let environment = storing("K3_ENV");
            operations
                .env
                .borrow_mut()
                .insert("K3_ENV".into(), "one".into());
            assert_eq!(
                key_of(&environment, &operations).await.as_deref(),
                Some("one")
            );
            operations
                .env
                .borrow_mut()
                .insert("K3_ENV".into(), "two".into());
            assert_eq!(
                key_of(&environment, &operations).await.as_deref(),
                Some("two")
            );
        });
    }

    /// Raw OAuth record with the given members after the discriminator.
    fn oauth_record(fields: &str) -> String {
        format!(r#"{{"type":"oauth"{fields}}}"#)
    }

    /// Provider whose stored record is `record`, with a fallback that must not be reached.
    fn fallback_storage(provider: &str, record: &str) -> AuthStorage {
        let storage = storage_with(&format!(r#"{{"{provider}":{record}}}"#));
        storage.set_fallback_resolver(|_| Some("fallback".into()));
        storage
    }

    /// Known but malformed records end the lookup without a fallback.
    async fn malformed_records_stop() {
        let _provider = ControlledProvider::new("k4-oauth").register();
        let _cleanup = Unregister("k4-oauth".into());
        let operations = Operations::default();
        let expires = r#""expires":4102444800000"#;
        let keyed = [
            r#"{"type":"api_key"}"#.to_owned(),
            r#"{"type":"api_key","key":null}"#.to_owned(),
            r#"{"type":"api_key","key":7}"#.to_owned(),
            r#"{"type":"api_key","key":{"a":1}}"#.to_owned(),
            r#"{"type":"api_key","key":["k"]}"#.to_owned(),
        ];
        let oauth = [
            oauth_record(&format!(r#","access":"a",{expires}"#)),
            oauth_record(&format!(r#","refresh":null,"access":"a",{expires}"#)),
            oauth_record(&format!(r#","refresh":"r",{expires}"#)),
            oauth_record(&format!(r#","refresh":"r","access":null,{expires}"#)),
            oauth_record(&format!(r#","refresh":7,"access":"a",{expires}"#)),
            oauth_record(&format!(r#","refresh":"r","access":3,{expires}"#)),
            oauth_record(r#","refresh":"r","access":"a""#),
            oauth_record(r#","refresh":"r","access":"a","expires":null"#),
            oauth_record(r#","refresh":"r","access":"a","expires":"wrong""#),
        ];
        for (provider, records) in [("k4-key", &keyed[..]), ("k4-oauth", &oauth[..])] {
            for (index, record) in records.iter().enumerate() {
                let storage = fallback_storage(provider, record);
                let key = storage.get_api_key(provider, true, &operations).await;
                assert_eq!(key.unwrap(), None, "case {index}");
            }
        }
    }

    /// Unrecognized records fall through; complete provider-specific records stay usable.
    async fn unrecognized_records_fall_through() {
        let _provider = ControlledProvider::new("k4-oauth").register();
        let _cleanup = Unregister("k4-oauth".into());
        let operations = Operations::default();
        let unrecognized = [
            r#"{"type":"unknown"}"#,
            r#"{"key":"k"}"#,
            r#"{"type":{"a":1}}"#,
            "7",
            "[1]",
            "null",
        ];
        for (index, record) in unrecognized.into_iter().enumerate() {
            let storage = fallback_storage("k4-other", record);
            let key = storage.get_api_key("k4-other", true, &operations).await;
            assert_eq!(key.unwrap().as_deref(), Some("fallback"), "case {index}");
        }
        let fields = r#","refresh":"r","access":"extension-key","expires":4102444800000,"enterpriseUrl":"u""#;
        let valid = fallback_storage("k4-oauth", &oauth_record(fields));
        let key = valid.get_api_key("k4-oauth", true, &operations).await;
        assert_eq!(key.unwrap().as_deref(), Some("u"));
    }

    #[test]
    fn malformed_credentials_stop_selection_but_unknown_types_fall_through() {
        block_on(async {
            malformed_records_stop().await;
            unrecognized_records_fall_through().await;
        });
    }

    /// Name of the child scenario, empty in the parent.
    fn scenario() -> String {
        std::env::var("MAESTRO_AUTH_SCENARIO").unwrap_or_default()
    }

    /// Storage with a fallback resolver returning `fallback`.
    fn with_fallback(storage: AuthStorage, fallback: &'static str) -> AuthStorage {
        storage.set_fallback_resolver(move |_| Some(fallback.to_owned()));
        storage
    }

    /// Key for `openai` and the number of fallback calls, under the fallback `fallback`
    /// (none when `None`) and `include_fallback`.
    async fn openai_key(
        fallback: Option<&'static str>,
        include_fallback: bool,
    ) -> (Option<String>, usize) {
        let storage = storage_with("{}");
        let calls = Arc::new(AtomicUsize::new(0));
        if let Some(fallback) = fallback {
            let counter = calls.clone();
            storage.set_fallback_resolver(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                Some(fallback.to_owned())
            });
        }
        let operations = Operations::default();
        let key = storage.get_api_key("openai", include_fallback, &operations);
        (key.await.unwrap(), calls.load(Ordering::SeqCst))
    }

    /// Check one environment scenario inside its child process.
    async fn environment_scenario() {
        let populated = scenario() == "populated";
        let expected = |ambient_free: Option<&str>| {
            populated
                .then(|| "env-key".to_owned())
                .or_else(|| ambient_free.map(str::to_owned))
        };
        let ran = usize::from(!populated);
        assert_eq!(
            openai_key(Some("fallback"), true).await,
            (expected(Some("fallback")), ran)
        );
        assert_eq!(
            openai_key(Some("fallback"), false).await,
            (expected(None), 0)
        );
        assert_eq!(openai_key(Some(""), true).await, (expected(Some("")), ran));
        assert_eq!(openai_key(None, true).await, (expected(None), 0));
    }

    #[test]
    fn environment_precedes_optional_fallback() {
        if is_child() {
            return block_on(environment_scenario());
        }
        let dir = TempDir::new("key-environment");
        for (name, variables) in [
            ("populated", vec![("OPENAI_API_KEY", "env-key")]),
            ("empty", vec![("OPENAI_API_KEY", "")]),
            ("absent", vec![]),
        ] {
            let mut envs = variables;
            envs.push(("MAESTRO_AUTH_SCENARIO", name));
            run_child(
                "tests::environment_precedes_optional_fallback",
                dir.root(),
                &envs,
            );
        }
        let record = r#"{"type":"oauth","refresh":"r","access":"a","expires":4102444800000}"#;
        let unregistered = fallback_storage("k5-unregistered", record);
        let key =
            block_on(unregistered.get_api_key("k5-unregistered", true, &Operations::default()));
        assert_eq!(key.unwrap(), None);
    }

    /// Fails the build unless `value` can cross threads.
    fn assert_send<T: Send>(value: &T) -> &T {
        value
    }

    #[test]
    fn request_key_future_is_send_without_sync_operations() {
        block_on(async {
            let operations = printing(&[("k6-key", "resolved")]);
            let storage = storing("!k6-key");
            let future = storage.get_api_key("p", true, &operations);
            assert_send(&future);
            drop(operations);
            assert_eq!(future.await.unwrap().as_deref(), Some("resolved"));
        });
    }

    /// With `OPENAI_API_KEY` set, failing helpers still end the lookup without a key.
    async fn failing_helpers_end_lookup() {
        let native = ProcessConfigValueOperations::new(|| unreachable!("Unix avoids selection"));
        let commands = [
            "!exit 1",
            "!maestro-no-such-helper-k7",
            "!printf ''",
            "!printf '  \\n'",
        ];
        for (index, command) in commands.into_iter().enumerate() {
            let record = serde_json::json!({"openai": {"type": "api_key", "key": command}});
            let storage = with_fallback(storage_with(&record.to_string()), "fallback");
            let key = storage.get_api_key("openai", true, &native).await;
            assert_eq!(key.unwrap(), None, "case {index}");
        }
    }

    #[test]
    fn stored_helper_failure_does_not_consult_ambient_sources() {
        if is_child() {
            return block_on(failing_helpers_end_lookup());
        }
        let dir = TempDir::new("helper-failure");
        run_child(
            "tests::stored_helper_failure_does_not_consult_ambient_sources",
            std::path::Path::new(&dir.path("")),
            &[("OPENAI_API_KEY", "ambient")],
        );
    }
}
