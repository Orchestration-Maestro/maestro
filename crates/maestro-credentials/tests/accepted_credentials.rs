//! Accepted credential storage through the public interface.
#[cfg(test)]
mod support;
#[cfg(test)]
mod tests {

    use super::support::{TempDir, is_child, run_child};
    use indexmap::IndexMap;
    use maestro_credentials::{
        ApiKeyCredential, AsyncLockUpdate, AuthCredential, AuthSource, AuthStatus, AuthStorage,
        AuthStorageBackend, AuthStorageData, AuthStorageError, AuthStorageFuture,
        InMemoryAuthStorageBackend, LockUpdate, OAuthCredential,
    };
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Stored API key credential.
    fn api_key(key: &str) -> AuthCredential {
        AuthCredential::ApiKey(ApiKeyCredential {
            key: key.to_owned(),
        })
    }

    /// OAuth credential with an enterprise URL.
    fn oauth(refresh: &str) -> AuthCredential {
        let extra = json!({"enterpriseUrl": "https://example.test"});
        AuthCredential::OAuth(OAuthCredential {
            refresh: refresh.to_owned(),
            access: "access".to_owned(),
            expires: 1_730_000_000_000.0,
            extra: extra.as_object().unwrap().clone(),
        })
    }

    /// Pretty text of a document whose records are API keys.
    fn pretty_keys(records: &[(&str, &str)]) -> String {
        let document: serde_json::Map<String, Value> = records
            .iter()
            .map(|(name, key)| ((*name).to_owned(), json!({"type": "api_key", "key": key})))
            .collect();
        format!("{:#}", Value::Object(document))
    }

    /// Write `text` to the file at `path`.
    fn write_file(path: &str, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    /// Read the file at `path`.
    fn read_file(path: &str) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    /// Backend that refuses every write and keeps the text in an inner memory backend.
    struct RefusingWrites(Arc<InMemoryAuthStorageBackend>);

    impl AuthStorageBackend for RefusingWrites {
        fn with_lock(
            &self,
            update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
        ) -> Result<(), maestro_credentials::AuthStorageError> {
            self.0.with_lock(&mut |current| match update(current)? {
                Some(_) => Err("write refused".into()),
                None => Ok(None),
            })
        }

        fn with_lock_async<'a>(
            &'a self,
            update: AsyncLockUpdate<'a>,
        ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
            self.0
                .with_lock_async(Box::new(|current| Box::pin(refuse_writes(update, current))))
        }
    }

    /// Run `update` and refuse any replacement text.
    async fn refuse_writes(update: AsyncLockUpdate<'_>, current: Option<String>) -> LockUpdate {
        match update(current).await? {
            Some(_) => Err("write refused".into()),
            None => Ok(None),
        }
    }

    /// Current text of a memory backend.
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

    #[test]
    fn maestro_credentials_publish_before_failed_write() {
        let inner = Arc::new(InMemoryAuthStorageBackend::default());
        let storage = AuthStorage::from_storage(RefusingWrites(Arc::clone(&inner)));
        storage.set("openai", api_key("one"));
        assert_eq!(storage.get("openai"), Some(api_key("one")));
        assert_eq!(storage.list(), ["openai"]);
        assert_eq!(memory_text(&inner), None);
        let errors = storage.drain_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].to_string(), "write refused");
        storage.set("openai", api_key("two"));
        assert_eq!(storage.drain_errors().len(), 1);
        storage.remove("openai");
        assert!(!storage.has("openai"));
        assert_eq!(storage.drain_errors().len(), 1);
        assert_eq!(memory_text(&inner), None);
    }

    #[test]
    fn maestro_credential_listing_preserves_record_order() {
        let dir = TempDir::new("listing");
        let path = dir.path("auth.json");
        write_file(
            &path,
            &pretty_keys(&[("zeta", "z"), ("alpha", "a"), ("mid", "m")]),
        );
        let storage = AuthStorage::create(&path);
        assert_eq!(storage.list(), ["zeta", "alpha", "mid"]);
        storage.set("alpha", api_key("a2"));
        assert_eq!(storage.list(), ["zeta", "alpha", "mid"]);
        storage.remove("zeta");
        storage.set("zeta", api_key("z2"));
        assert_eq!(storage.list(), ["alpha", "mid", "zeta"]);
        assert_eq!(
            read_file(&path),
            pretty_keys(&[("alpha", "a2"), ("mid", "m"), ("zeta", "z2")])
        );
        write_file(
            &path,
            &pretty_keys(&[("mid", "m"), ("beta", "b"), ("zeta", "z")]),
        );
        storage.reload();
        assert_eq!(storage.list(), ["mid", "beta", "zeta"]);
        assert_eq!(
            storage.get_all().keys().collect::<Vec<_>>(),
            ["mid", "beta", "zeta"]
        );
        assert!(storage.drain_errors().is_empty());
    }

    #[test]
    fn unrelated_external_records_survive_without_import() {
        let dir = TempDir::new("external");
        let path = dir.path("auth.json");
        write_file(
            &path,
            &pretty_keys(&[("anthropic", "old"), ("openai", "o")]),
        );
        let storage = AuthStorage::create(&path);
        write_file(
            &path,
            &pretty_keys(&[("anthropic", "old"), ("openai", "o"), ("google", "g")]),
        );
        storage.set("anthropic", api_key("new"));
        assert_eq!(
            read_file(&path),
            pretty_keys(&[("anthropic", "new"), ("openai", "o"), ("google", "g")])
        );
        storage.remove("anthropic");
        assert_eq!(
            read_file(&path),
            pretty_keys(&[("openai", "o"), ("google", "g")])
        );
        assert_eq!(storage.list(), ["openai"]);
        assert!(!storage.has("google"));
    }

    #[test]
    fn rewritten_file_keeps_reread_order_while_listing_keeps_accepted_order() {
        let dir = TempDir::new("reordered");
        let path = dir.path("auth.json");
        write_file(&path, &pretty_keys(&[("a", "1"), ("b", "2")]));
        let storage = AuthStorage::create(&path);
        write_file(&path, &pretty_keys(&[("b", "2"), ("a", "1")]));
        storage.set("a", api_key("3"));
        assert_eq!(read_file(&path), pretty_keys(&[("b", "2"), ("a", "3")]));
        assert_eq!(storage.list(), ["a", "b"]);
    }

    #[test]
    fn preserved_float_expiry_keeps_its_fraction_on_unrelated_rewrite() {
        let dir = TempDir::new("float-expiry");
        let path = dir.path("auth.json");
        write_file(
            &path,
            "{\"o\":{\"type\":\"oauth\",\"refresh\":\"r\",\"access\":\"a\",\"expires\":1730000000000.0}}",
        );
        let storage = AuthStorage::create(&path);
        storage.set("other", api_key("x"));
        assert!(read_file(&path).contains("\"expires\": 1730000000000.0"));
    }

    #[test]
    fn malformed_document_blocks_writes_until_reload() {
        let dir = TempDir::new("malformed");
        let path = dir.path("auth.json");
        write_file(&path, "{invalid-json");
        let storage = AuthStorage::create(&path);
        assert!(storage.list().is_empty());
        storage.set("openai", api_key("o"));
        storage.remove("missing");
        assert_eq!(storage.list(), ["openai"]);
        assert_eq!(read_file(&path), "{invalid-json");
        assert_eq!(storage.drain_errors().len(), 1);
        write_file(&path, &pretty_keys(&[("google", "g")]));
        storage.reload();
        assert_eq!(storage.list(), ["google"]);
        assert!(storage.drain_errors().is_empty());
        storage.set("openai", api_key("o"));
        assert_eq!(
            read_file(&path),
            pretty_keys(&[("google", "g"), ("openai", "o")])
        );
    }

    #[test]
    fn failed_reload_keeps_records_and_drains_errors_in_order() {
        let dir = TempDir::new("failed-reload");
        let path = dir.path("auth.json");
        write_file(&path, &pretty_keys(&[("anthropic", "a")]));
        let storage = AuthStorage::create(&path);
        write_file(&path, "{invalid-json");
        storage.reload();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        storage.reload();
        assert_eq!(storage.list(), ["anthropic"]);
        assert_eq!(storage.get("anthropic"), Some(api_key("a")));
        let errors = storage.drain_errors();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].downcast_ref::<serde_json::Error>().is_some());
        assert!(errors[1].downcast_ref::<std::io::Error>().is_some());
        assert!(storage.drain_errors().is_empty());
    }

    #[test]
    fn newline_terminated_text_loads_untouched_and_rewrites_without_newline() {
        let dir = TempDir::new("newline");
        let path = dir.path("auth.json");
        let text = format!("{}\n", pretty_keys(&[("a", "1")]));
        write_file(&path, &text);
        let storage = AuthStorage::create(&path);
        assert_eq!(storage.list(), ["a"]);
        assert_eq!(read_file(&path), text);
        storage.set("b", api_key("2"));
        assert_eq!(read_file(&path), pretty_keys(&[("a", "1"), ("b", "2")]));
        assert!(storage.drain_errors().is_empty());
    }

    #[test]
    fn non_object_documents_are_load_errors() {
        let documents = [
            "null",
            "[]",
            "5",
            "\"ab\"",
            "true",
            "\u{feff}{}",
            " \n",
            "{\"a\":1e400}",
            "{\"a\":\"\\ud800\"}",
        ];
        for document in documents {
            let dir = TempDir::new("non-object");
            let path = dir.path("auth.json");
            write_file(&path, document);
            let storage = AuthStorage::create(&path);
            assert_eq!(storage.drain_errors().len(), 1, "{document:?}");
            assert!(storage.list().is_empty(), "{document:?}");
            storage.set("openai", api_key("o"));
            assert_eq!(read_file(&path), document, "{document:?}");
        }
    }

    #[test]
    fn stored_records_decode_strictly_and_keep_unknown_payloads() {
        let dir = TempDir::new("decode");
        let path = dir.path("auth.json");
        let source = r#"{
  "dup": {"type": "api_key", "key": "first"},
  "other": {"type": "future", "value": 1},
  "apiNoKey": {"type": "api_key"},
  "apiNumberKey": {"type": "api_key", "key": 5},
  "oauthPartial": {"type": "oauth", "access": "a", "expires": 1},
  "oauthStringExpires": {"type": "oauth", "refresh": "r", "access": "a", "expires": "1"},
  "oauthNullExpires": {"type": "oauth", "refresh": "r", "access": "a", "expires": null},
  "positional": ["api_key", "k"],
  "nul": null,
  "zero": 0,
  "apiExtra": {"type": "api_key", "key": "k", "note": "kept"},
  "o": {"type": "oauth", "refresh": "r", "access": "a", "expires": 1730000000000, "enterpriseUrl": "u", "nested": {"b": 1, "a": [2]}},
  "dup": {"type": "api_key", "key": "last"}
}"#;
        write_file(&path, source);
        let storage = AuthStorage::create(&path);
        assert!(storage.drain_errors().is_empty());
        let typed = ["apiExtra", "o", "dup"];
        for provider in storage.list() {
            assert!(storage.has(&provider));
            assert_eq!(
                storage.get(&provider).is_some(),
                typed.contains(&provider.as_str()),
                "{provider}"
            );
        }
        assert_eq!(storage.list().len(), 12);
        assert_eq!(storage.get("apiExtra"), Some(api_key("k")));
        assert_eq!(storage.get("dup"), Some(api_key("last")));
        let Some(AuthCredential::OAuth(token)) = storage.get("o") else {
            panic!("complete OAuth record decodes");
        };
        assert_eq!(token.expires.to_bits(), 1_730_000_000_000.0_f64.to_bits());
        assert_eq!(
            serde_json::to_string(&token.extra).unwrap(),
            r#"{"enterpriseUrl":"u","nested":{"b":1,"a":[2]}}"#
        );
        assert_eq!(
            storage.get_all().keys().collect::<Vec<_>>(),
            ["dup", "apiExtra", "o"]
        );
        storage.set("new", api_key("n"));
        let rewritten: Value = serde_json::from_str(&read_file(&path)).unwrap();
        let original: Value = serde_json::from_str(source).unwrap();
        let mut expected = original.as_object().unwrap().clone();
        expected.insert("new".to_owned(), json!({"type": "api_key", "key": "n"}));
        assert_eq!(rewritten, Value::Object(expected));
        let text = read_file(&path);
        assert!(text.contains("\"expires\": 1730000000000,"));
        assert!(
            text.contains(
                "\"nested\": {\n      \"b\": 1,\n      \"a\": [\n        2\n      ]\n    }"
            )
        );
        assert!(text.find("\"dup\"").unwrap() < text.find("\"other\"").unwrap());
    }

    /// Fallback resolver that counts calls and answers `key`.
    fn counting_resolver(
        count: &Arc<AtomicUsize>,
        key: &'static str,
    ) -> impl Fn(&str) -> Option<String> + use<> {
        let count = Arc::clone(count);
        move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Some(key.to_owned())
        }
    }

    #[test]
    fn status_reports_sources_without_secrets_or_side_effects() {
        let dir = TempDir::new("status");
        let marker = dir.path("marker");
        let mut data = AuthStorageData::new();
        data.insert("secret".into(), api_key("secret-value"));
        data.insert("command".into(), api_key(&format!("!touch {marker}")));
        data.insert("tokens".into(), oauth("refresh-secret"));
        let storage = AuthStorage::in_memory(data);
        let calls = Arc::new(AtomicUsize::new(0));
        storage.set_fallback_resolver(counting_resolver(&calls, "fallback-key"));
        storage.set_runtime_api_key("secret", "runtime");
        let stored = AuthStatus {
            configured: true,
            source: Some(AuthSource::Stored),
            label: None,
        };
        for provider in ["secret", "command", "tokens"] {
            assert_eq!(storage.get_auth_status(provider), stored);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        storage.set_runtime_api_key("unknown", "");
        assert_eq!(
            storage.get_auth_status("unknown"),
            AuthStatus {
                configured: false,
                source: Some(AuthSource::Runtime),
                label: Some("--api-key".into())
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            storage.get_auth_status("custom-provider"),
            AuthStatus {
                configured: false,
                source: Some(AuthSource::Fallback),
                label: Some("custom provider config".into())
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        storage.set_fallback_resolver(counting_resolver(&calls, ""));
        let none = AuthStatus {
            configured: false,
            source: None,
            label: None,
        };
        assert_eq!(storage.get_auth_status("custom-provider"), none);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(!std::path::Path::new(&marker).exists());
    }

    #[test]
    fn availability_counts_runtime_presence_and_nonempty_fallback() {
        let mut data = AuthStorageData::new();
        data.insert("stored".into(), api_key("k"));
        let storage = AuthStorage::in_memory(data);
        let key = Arc::new(Mutex::new(String::new()));
        let shared = Arc::clone(&key);
        storage.set_fallback_resolver(move |_| Some(shared.lock().unwrap().clone()));
        assert!(storage.has_auth("stored"));
        assert!(!storage.has_auth("custom-provider"));
        key.lock().unwrap().push_str("fallback");
        assert!(storage.has_auth("custom-provider"));
        key.lock().unwrap().clear();
        storage.set_runtime_api_key("custom-provider", "");
        assert!(storage.has_auth("custom-provider"));
        storage.remove_runtime_api_key("custom-provider");
        assert!(!storage.has_auth("custom-provider"));
    }

    #[test]
    fn stored_null_record_counts_as_present() {
        let backend = InMemoryAuthStorageBackend::default();
        backend
            .with_lock(&mut |_| Ok(Some(r#"{"nulled": null}"#.into())))
            .unwrap();
        let storage = AuthStorage::from_storage(backend);
        assert!(storage.has("nulled"));
        assert!(storage.has_auth("nulled"));
        assert_eq!(storage.get("nulled"), None);
    }

    /// Child body of [`environment_status_names_first_populated_key`].
    fn environment_scenario(scenario: &str) {
        let storage = AuthStorage::in_memory(AuthStorageData::new());
        let calls = Arc::new(AtomicUsize::new(0));
        storage.set_fallback_resolver(counting_resolver(&calls, ""));
        match scenario {
            "single" => {
                assert!(storage.has_auth("anthropic"));
                let status = storage.get_auth_status("anthropic");
                assert_eq!(status.source, Some(AuthSource::Environment));
                assert_eq!(status.label.as_deref(), Some("ANTHROPIC_API_KEY"));
            }
            "both" => {
                let status = storage.get_auth_status("anthropic");
                assert_eq!(status.label.as_deref(), Some("ANTHROPIC_OAUTH_TOKEN"));
            }
            _ => {
                assert!(storage.has_auth("amazon-bedrock"));
                let none = AuthStatus {
                    configured: false,
                    source: None,
                    label: None,
                };
                assert_eq!(storage.get_auth_status("amazon-bedrock"), none);
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                return;
            }
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn environment_status_names_first_populated_key() {
        let scenario = std::env::var("MAESTRO_AUTH_SCENARIO").unwrap_or_default();
        if is_child() {
            return environment_scenario(&scenario);
        }
        let dir = TempDir::new("environment");
        let cases: [(&str, &[(&str, &str)]); 3] = [
            ("single", &[("ANTHROPIC_API_KEY", "k")]),
            (
                "both",
                &[("ANTHROPIC_API_KEY", "k"), ("ANTHROPIC_OAUTH_TOKEN", "t")],
            ),
            ("ambient", &[("AWS_PROFILE", "p")]),
        ];
        for (name, variables) in cases {
            let mut envs = variables.to_vec();
            envs.push(("MAESTRO_AUTH_SCENARIO", name));
            run_child(
                "tests::environment_status_names_first_populated_key",
                dir.root(),
                &envs,
            );
        }
    }

    #[test]
    fn in_memory_seed_round_trips_typed_records() {
        let mut data: AuthStorageData = IndexMap::new();
        data.insert("b".into(), oauth("r"));
        data.insert("a".into(), api_key("k"));
        let storage = AuthStorage::in_memory(data.clone());
        assert_eq!(storage.list(), ["b", "a"]);
        assert_eq!(storage.get("b"), Some(oauth("r")));
        assert_eq!(storage.get_all(), data);
        assert!(
            AuthStorage::in_memory(AuthStorageData::new())
                .list()
                .is_empty()
        );
        let dir = TempDir::new("seed");
        let path = dir.path("auth.json");
        AuthStorage::create(&path).set("b", oauth("r"));
        assert_eq!(
            read_file(&path),
            "{\n  \"b\": {\n    \"type\": \"oauth\",\n    \"refresh\": \"r\",\n    \"access\": \"access\",\n    \"expires\": 1730000000000,\n    \"enterpriseUrl\": \"https://example.test\"\n  }\n}"
        );
    }

    /// Drive one store through the shared operations and report what it observed.
    fn drive(storage: &AuthStorage) -> Vec<String> {
        storage.set("one", api_key("1"));
        storage.set("two", api_key("2"));
        storage.logout("one");
        vec![
            format!("{:?}", storage.list()),
            format!("{:?}", storage.get("two")),
            format!("{}{}", storage.has("one"), storage.has("two")),
            format!("{}", storage.drain_errors().len()),
        ]
    }

    #[test]
    fn backends_swap_without_caller_changes() {
        let dir = TempDir::new("swap");
        let memory = AuthStorage::from_storage(InMemoryAuthStorageBackend::default());
        let file = AuthStorage::create(&dir.path("auth.json"));
        let expected = drive(&memory);
        assert_eq!(expected[0], "[\"two\"]");
        std::thread::scope(|scope| {
            let observed = scope.spawn(|| drive(&file)).join().unwrap();
            assert_eq!(observed, expected);
        });
    }

    #[test]
    fn memory_backend_runs_callbacks_on_owned_copies() {
        let backend = InMemoryAuthStorageBackend::default();
        let mut first = Some("unset".to_owned());
        backend
            .with_lock(&mut |current| {
                first = current.map(str::to_owned);
                Ok(Some("one".into()))
            })
            .unwrap();
        assert_eq!(first, None);
        let mut seen_inside = None;
        backend
            .with_lock(&mut |_| {
                backend.with_lock(&mut |current| {
                    seen_inside = current.map(str::to_owned);
                    Ok(Some("inner".into()))
                })?;
                Ok(Some("outer".into()))
            })
            .unwrap();
        assert_eq!(seen_inside.as_deref(), Some("one"));
        assert_eq!(memory_text(&backend).as_deref(), Some("outer"));
        let error = backend
            .with_lock(&mut |_| Err("callback failed".into()))
            .unwrap_err();
        assert_eq!(error.to_string(), "callback failed");
        assert_eq!(memory_text(&backend).as_deref(), Some("outer"));
    }
}
