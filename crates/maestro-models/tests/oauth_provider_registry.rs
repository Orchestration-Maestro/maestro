//! Ordered provider registration and credential dispatch.
#![cfg(test)]
use maestro_models::oauth::*;
use maestro_models::{BoxFuture, Fetch};
use std::sync::{Arc, Mutex, MutexGuard};

/// Serial admission for this binary's process-wide registry.
static ADMISSION: Mutex<()> = Mutex::new(());
/// Reset registry membership even when an assertion fails.
struct Admission {
    /// Keep admission until cleanup completes.
    _guard: MutexGuard<'static, ()>,
}
impl Admission {
    /// Start an isolated registry observation.
    fn enter() -> Self {
        let guard = ADMISSION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_oauth_providers();
        Self { _guard: guard }
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        reset_oauth_providers();
    }
}
/// Controlled provider refresh operation.
type Refresh = dyn Fn(OAuthCredentials, Option<Fetch>) -> BoxFuture<Result<OAuthCredentials, OAuthError>>
    + Send
    + Sync;
/// Controlled credential extractor.
type Key = dyn for<'a> Fn(&'a OAuthCredentials) -> Result<&'a str, OAuthError> + Send + Sync;
/// Provider with independently owned callbacks.
struct Provider {
    /// Literal identifier.
    id: String,
    /// Refresh behavior.
    refresh: Box<Refresh>,
    /// Extraction behavior.
    key: Box<Key>,
    /// Optional identifier observation.
    on_id: Option<Box<dyn Fn() + Send + Sync>>,
    /// Optional final ownership observation.
    on_drop: Option<Box<dyn Fn() + Send + Sync>>,
}
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &str {
        if let Some(callback) = &self.on_id {
            callback();
        }
        &self.id
    }
    fn name(&self) -> &str {
        &self.id
    }
    fn login(
        &self,
        _: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { panic!("registry does not authorize") })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        (self.refresh)(credentials, fetch)
    }
    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        (self.key)(credentials)
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        if let Some(callback) = &self.on_drop {
            callback();
        }
    }
}
/// Provider whose refresh result distinguishes registration versions.
fn provider(id: &str, access: &str) -> Provider {
    let access = access.to_owned();
    Provider {
        id: id.into(),
        refresh: Box::new(move |mut credentials, _| {
            credentials.access.clone_from(&access);
            Box::pin(async move { Ok(credentials) })
        }),
        key: Box::new(|credentials| Ok(&credentials.access)),
        on_id: None,
        on_drop: None,
    }
}
/// Supplied record with distinct access and refresh fields.
fn credentials(expires: f64) -> OAuthCredentials {
    OAuthCredentials {
        refresh: "old-refresh".into(),
        access: "old-access".into(),
        expires,
        extra: maestro_models::JsonObject::default(),
    }
}
/// Observe registry order rather than map equality.
fn ids() -> Vec<String> {
    get_oauth_providers()
        .iter()
        .map(|provider| provider.id().to_owned())
        .collect()
}
/// Run a fully controlled future without external services.
fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn maestro_oauth_registry_replaces_without_reordering() {
    let _admission = Admission::enter();
    assert_eq!(ids(), ["anthropic", "github-copilot", "openai-codex"]);
    register_oauth_provider(Arc::new(provider("z-last", "old")));
    register_oauth_provider(Arc::new(provider("a-last", "a")));
    register_oauth_provider(Arc::new(provider("z-last", "changed-access")));
    assert_eq!(
        ids(),
        [
            "anthropic",
            "github-copilot",
            "openai-codex",
            "z-last",
            "a-last"
        ]
    );
    let selected = get_oauth_provider("z-last").unwrap();
    assert_eq!(
        run(selected.refresh_token(credentials(f64::INFINITY), None))
            .unwrap()
            .access,
        "changed-access"
    );
    unregister_oauth_provider("z-last");
    assert!(get_oauth_provider("z-last").is_none());
    register_oauth_provider(Arc::new(provider("z-last", "reinserted")));
    assert_eq!(
        ids(),
        [
            "anthropic",
            "github-copilot",
            "openai-codex",
            "a-last",
            "z-last"
        ]
    );
}

/// Supplied diagnostic with a cause that must not be rewritten.
fn error(message: &str) -> OAuthError {
    OAuthError {
        diagnostic: Box::new(maestro_models::DiagnosticErrorInfo {
            message: message.into(),
            name: None,
            stack: None,
            code: Some(maestro_models::DiagnosticCode::Text(format!(
                "{message}-code"
            ))),
        }),
        errno: None,
        cause: Some(Box::new(
            maestro_models::DiagnosticErrorInfo {
                message: format!("{message}-cause"),
                name: None,
                stack: None,
                code: None,
            }
            .into(),
        )),
    }
}

#[test]
fn maestro_oauth_refresh_dispatches_selected_provider() {
    let _admission = Admission::enter();
    let supplied_fetch: Fetch =
        Arc::new(|_| panic!("custom refresh only observes transport identity"));
    let expected_fetch = Arc::clone(&supplied_fetch);
    let mut input = credentials(f64::INFINITY);
    input.extra.insert(
        "nested".into(),
        serde_json::json!({"retained": [1, "value"]}),
    );
    let expected = input.clone();
    let mut custom = provider("controlled", "unused");
    custom.key = Box::new(|_| panic!("direct refresh must not extract a key"));
    custom.refresh = Box::new(move |mut selected, fetch| {
        assert_eq!(selected, expected);
        assert!(Arc::ptr_eq(&fetch.unwrap(), &expected_fetch));
        selected.access = "refreshed".into();
        Box::pin(async move { Ok(selected) })
    });
    register_oauth_provider(Arc::new(custom));
    let output = run(refresh_oauth_token(
        "controlled",
        input.clone(),
        Some(supplied_fetch),
    ))
    .unwrap();
    input.access = "refreshed".into();
    assert_eq!(output, input);
    let mut failing = provider("controlled", "unused");
    failing.refresh = Box::new(|_, _| Box::pin(async { Err(error("inner-refresh")) }));
    register_oauth_provider(Arc::new(failing));
    let failure = run(refresh_oauth_token("controlled", input, None)).unwrap_err();
    assert_eq!(failure.to_string(), "inner-refresh");
    assert_eq!(
        failure.diagnostic.code,
        Some(maestro_models::DiagnosticCode::Text(
            "inner-refresh-code".into()
        ))
    );
    assert_eq!(failure.cause.unwrap().to_string(), "inner-refresh-cause");
    assert_eq!(
        run(refresh_oauth_token("missing", credentials(0.0), None))
            .unwrap_err()
            .to_string(),
        "Unknown OAuth provider: missing"
    );
}

#[test]
fn maestro_oauth_key_refreshes_at_expiry() {
    let _admission = Admission::enter();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let refresh_calls = Arc::clone(&calls);
    let mut custom = provider("controlled", "unused");
    custom.key = Box::new(|record| Ok(&record.refresh));
    custom.refresh = Box::new(move |mut record, _| {
        assert_eq!(record.access, "old-access");
        assert_eq!(record.refresh, "old-refresh");
        assert_eq!(
            record.extra["nested"],
            serde_json::json!({"retained": ["value", 2]})
        );
        refresh_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        record.access = "new-access".into();
        record.refresh = "new-refresh".into();
        record.expires = 500.0;
        Box::pin(async move { Ok(record) })
    });
    register_oauth_provider(Arc::new(custom));
    for (expiry, key, access, expected_calls) in [
        (f64::INFINITY, "old-refresh", "old-access", 0),
        (0.0, "new-refresh", "new-access", 1),
    ] {
        let mut record = credentials(expiry);
        record.extra.insert(
            "nested".into(),
            serde_json::json!({"retained": ["value", 2]}),
        );
        let input = indexmap::IndexMap::from([("controlled".into(), record)]);
        let before = input.clone();
        let output = run(maestro_models::get_oauth_api_key(
            "controlled",
            &input,
            None,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(output.api_key, key);
        assert_eq!(output.new_credentials.access, access);
        assert_eq!(output.new_credentials.extra, before["controlled"].extra);
        assert_eq!(
            output.new_credentials.expires.to_bits(),
            (if expected_calls == 0 { expiry } else { 500.0 }).to_bits()
        );
        assert_eq!(input, before);
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            expected_calls
        );
    }
    check_empty_refresh_result();
}

#[test]
fn maestro_oauth_key_preserves_error_boundaries() {
    let _admission = Admission::enter();
    let empty = indexmap::IndexMap::new();
    assert_eq!(
        run(get_oauth_api_key("unknown", &empty, None))
            .err()
            .unwrap()
            .to_string(),
        "Unknown OAuth provider: unknown"
    );
    let mut absent = provider("controlled", "unused");
    absent.refresh = Box::new(|_, _| panic!("absence must not refresh"));
    absent.key = Box::new(|_| panic!("absence must not extract"));
    register_oauth_provider(Arc::new(absent));
    assert!(
        run(get_oauth_api_key("controlled", &empty, None))
            .unwrap()
            .is_none()
    );
    let mut failing = provider("controlled", "unused");
    failing.refresh = Box::new(|_, _| Box::pin(async { Err(error("inner-refresh")) }));
    failing.key = Box::new(|_| panic!("failed refresh must not extract"));
    register_oauth_provider(Arc::new(failing));
    let expired = indexmap::IndexMap::from([("controlled".into(), credentials(0.0))]);
    let failure = run(get_oauth_api_key("controlled", &expired, None))
        .err()
        .unwrap();
    assert_eq!(
        failure.to_string(),
        "Failed to refresh OAuth token for controlled"
    );
    assert!(failure.diagnostic.code.is_none());
    assert!(failure.diagnostic.name.is_none());
    assert!(failure.diagnostic.stack.is_none());
    assert!(failure.cause.is_none());
    assert!(failure.errno.is_none());
    check_extraction_failures();
}

#[test]
fn maestro_oauth_provider_names_are_literal() {
    let _admission = Admission::enter();
    let names = [
        "",
        "anthropic",
        "ANTHROPIC",
        " anthropic ",
        "\u{feff}anthropic",
        "\u{85}anthropic",
        "toString",
        "constructor",
        "__proto__",
        "a\0b",
        "é",
        "e\u{301}",
        "😀",
        "line\nbreak",
    ];
    for id in names {
        let alias: OAuthProvider = id.into();
        let identifier: OAuthProviderId = alias;
        register_oauth_provider(Arc::new(provider(&identifier, "new")));
        let selected = get_oauth_provider(id).unwrap();
        assert_eq!(selected.id(), id);
        let input = indexmap::IndexMap::from([(identifier, credentials(f64::INFINITY))]);
        assert_eq!(
            run(get_oauth_api_key(id, &input, None))
                .unwrap()
                .unwrap()
                .api_key,
            "old-access"
        );
        assert!(
            run(get_oauth_api_key(id, &indexmap::IndexMap::new(), None))
                .unwrap()
                .is_none()
        );
    }
    check_distinct_literal_names();
    check_unknown_literal_names();
}

/// Coexisting literal names are neither normalized nor sorted.
fn check_distinct_literal_names() {
    reset_oauth_providers();
    let distinct = [
        "key",
        " key ",
        "\u{feff}key",
        "\u{85}key",
        "é",
        "e\u{301}",
        "a",
        "a\0b",
        "😀",
        "",
    ];
    for (index, id) in distinct.iter().enumerate() {
        register_oauth_provider(Arc::new(provider(id, &index.to_string())));
    }
    let mut expected = vec!["anthropic", "github-copilot", "openai-codex"];
    expected.extend(distinct);
    assert_eq!(ids(), expected);
    for (index, id) in distinct.iter().enumerate() {
        assert_eq!(
            run(refresh_oauth_token(id, credentials(0.0), None))
                .unwrap()
                .access,
            index.to_string()
        );
    }
}

/// Unknown identifiers retain their supplied text in both errors.
fn check_unknown_literal_names() {
    for id in ["KEY", "missing", "line\nunknown", "☃"] {
        assert!(get_oauth_provider(id).is_none());
        assert_eq!(
            run(get_oauth_api_key(id, &indexmap::IndexMap::new(), None))
                .err()
                .unwrap()
                .to_string(),
            format!("Unknown OAuth provider: {id}")
        );
        assert_eq!(
            run(refresh_oauth_token(id, credentials(0.0), None))
                .unwrap_err()
                .to_string(),
            format!("Unknown OAuth provider: {id}")
        );
    }
    unregister_oauth_provider("");
    assert_eq!(
        run(get_oauth_api_key("", &indexmap::IndexMap::new(), None))
            .err()
            .unwrap()
            .to_string(),
        "Unknown OAuth provider: "
    );
    assert_eq!(
        run(refresh_oauth_token("", credentials(0.0), None))
            .unwrap_err()
            .to_string(),
        "Unknown OAuth provider: "
    );
}

#[test]
fn maestro_oauth_registry_restores_builtin_providers() {
    let _admission = Admission::enter();
    for id in ["anthropic", "github-copilot", "openai-codex"] {
        register_oauth_provider(Arc::new(provider(id, "override")));
        assert_eq!(
            run(refresh_oauth_token(id, credentials(0.0), None))
                .unwrap()
                .access,
            "override"
        );
        unregister_oauth_provider(id);
        assert_eq!(
            get_oauth_provider(id)
                .unwrap()
                .get_api_key(&credentials(0.0))
                .unwrap(),
            "old-access"
        );
        assert_eq!(ids(), ["anthropic", "github-copilot", "openai-codex"]);
    }
    register_oauth_provider(Arc::new(provider("z-custom", "z")));
    register_oauth_provider(Arc::new(provider("a-custom", "a")));
    unregister_oauth_provider("z-custom");
    unregister_oauth_provider("unknown");
    assert_eq!(
        ids(),
        ["anthropic", "github-copilot", "openai-codex", "a-custom"]
    );
    register_oauth_provider(Arc::new(provider("anthropic", "overridden")));
    for _ in 0..2 {
        reset_oauth_providers();
        assert_eq!(ids(), ["anthropic", "github-copilot", "openai-codex"]);
        assert!(get_oauth_provider("a-custom").is_none());
        assert_eq!(
            get_oauth_provider("anthropic")
                .unwrap()
                .get_api_key(&credentials(0.0))
                .unwrap(),
            "old-access"
        );
    }
}

/// Current epoch milliseconds, used only to bracket a real provider clock.
fn now() -> f64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.0)
        .floor()
}
/// Exercise the real selected account provider with controlled transport.
fn check_builtin_refresh(id: &str) {
    use base64::Engine as _;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acct-controlled"}}"#);
    let jwt = format!("h.{payload}.s");
    let (body, key, offset) = match id {
        "anthropic" => (r#"{"access_token":"controlled-subscription","refresh_token":"controlled-refresh","expires_in":3600}"#.to_owned(), "controlled-subscription".to_owned(), 3_300_000.0),
        "github-copilot" => (r#"{"token":"controlled-device","expires_at":1234}"#.to_owned(), "controlled-device".to_owned(), 0.0),
        "openai-codex" => (format!(r#"{{"access_token":"{jwt}","refresh_token":"controlled-refresh","expires_in":3600}}"#), jwt, 3_600_000.0),
        _ => unreachable!(),
    };
    let identifier = id.to_owned();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let request_calls = Arc::clone(&calls);
    let fetch: Fetch = Arc::new(move |request| {
        request_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        check_builtin_request(&identifier, &request);
        let body = body.clone();
        Box::pin(async move {
            Ok(maestro_models::HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::default(),
                body: Box::pin(futures_util::stream::iter([Ok(body.into_bytes())])),
            })
        })
    });
    let before = now();
    let output = run(refresh_oauth_token(id, credentials(0.0), Some(fetch))).unwrap();
    let after = now();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(output.access, key);
    assert_eq!(
        output.refresh,
        if id == "github-copilot" {
            "old-refresh"
        } else {
            "controlled-refresh"
        }
    );
    if id == "github-copilot" {
        assert_eq!(output.expires.to_bits(), 934_000.0_f64.to_bits());
    } else {
        assert!(output.expires >= before + offset && output.expires <= after + offset);
    }
    if id == "openai-codex" {
        assert_eq!(output.extra["accountId"], "acct-controlled");
    }
    assert_eq!(
        get_oauth_provider(id)
            .unwrap()
            .get_api_key(&output)
            .unwrap(),
        key
    );
}

#[test]
fn maestro_oauth_builtin_registry_uses_completed_providers() {
    let _admission = Admission::enter();
    for (id, name, callback) in [
        ("anthropic", "Anthropic (Claude Pro/Max)", Some(true)),
        ("github-copilot", "GitHub Copilot", None),
        (
            "openai-codex",
            "ChatGPT Plus/Pro (Codex Subscription)",
            Some(true),
        ),
    ] {
        let selected = get_oauth_provider(id).unwrap();
        assert_eq!(selected.id(), id);
        assert_eq!(selected.name(), name);
        assert_eq!(selected.uses_callback_server(), callback);
        assert_eq!(
            selected.get_api_key(&credentials(0.0)).unwrap(),
            "old-access"
        );
        check_builtin_refresh(id);
        register_oauth_provider(Arc::new(provider(id, "override")));
        unregister_oauth_provider(id);
        check_builtin_refresh(id);
    }
}

#[test]
fn maestro_oauth_registry_retains_shared_provider_state() {
    let _admission = Admission::enter();
    for transition in ["replace", "remove", "reset"] {
        let state = Arc::new(Mutex::new("before".to_owned()));
        let refresh_state = Arc::clone(&state);
        let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let drop_calls = Arc::clone(&drops);
        let mut custom = provider("shared", "unused");
        custom.refresh = Box::new(move |mut record, _| {
            record.access = refresh_state.lock().unwrap().clone();
            Box::pin(async move { Ok(record) })
        });
        custom.on_drop = Some(Box::new(move || {
            drop_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let external: OAuthProviderHandle = Arc::new(custom);
        register_oauth_provider(Arc::clone(&external));
        let lookup = get_oauth_provider("shared").unwrap();
        let snapshot = get_oauth_providers();
        let saved = snapshot.last().unwrap();
        assert!(Arc::ptr_eq(&external, &lookup));
        assert!(Arc::ptr_eq(&external, saved));
        *state.lock().unwrap() = "after".into();
        match transition {
            "replace" => register_oauth_provider(Arc::new(provider("shared", "replacement"))),
            "remove" => unregister_oauth_provider("shared"),
            "reset" => reset_oauth_providers(),
            _ => unreachable!(),
        }
        assert_eq!(
            snapshot.iter().map(|entry| entry.id()).collect::<Vec<_>>(),
            ["anthropic", "github-copilot", "openai-codex", "shared"]
        );
        for alias in [&external, &lookup, saved] {
            let output = run(alias.refresh_token(credentials(0.0), None)).unwrap();
            assert_eq!(alias.get_api_key(&output).unwrap(), "after");
        }
        if transition == "replace" {
            assert_eq!(
                run(refresh_oauth_token("shared", credentials(0.0), None))
                    .unwrap()
                    .access,
                "replacement"
            );
        } else {
            assert!(get_oauth_provider("shared").is_none());
        }
        drop(external);
        drop(lookup);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        drop(snapshot);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        reset_oauth_providers();
    }
}

#[test]
fn maestro_oauth_registry_drops_callbacks_after_unlocking() {
    let _admission = Admission::enter();
    let ids_read = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let id_calls = Arc::clone(&ids_read);
    let mut reentrant = provider("from-outer", "outer");
    reentrant.on_id = Some(Box::new(move || {
        id_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        register_oauth_provider(Arc::new(provider("from-id", "inner")));
    }));
    register_oauth_provider(Arc::new(reentrant));
    assert_eq!(ids_read.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(get_oauth_provider("from-outer").is_some());
    assert!(get_oauth_provider("from-id").is_some());
    for retained in [false, true] {
        for transition in ["replace", "remove", "restore", "reset"] {
            let id = if transition == "restore" {
                "anthropic"
            } else {
                "retired"
            };
            let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = Arc::clone(&completed);
            let mut custom = provider(id, "unused");
            custom.on_drop = Some(Box::new(move || {
                register_oauth_provider(Arc::new(provider("from-drop", "drop")));
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }));
            register_oauth_provider(Arc::new(custom));
            let survivor = retained.then(|| get_oauth_provider(id).unwrap());
            match transition {
                "replace" => register_oauth_provider(Arc::new(provider(id, "new"))),
                "remove" | "restore" => unregister_oauth_provider(id),
                "reset" => reset_oauth_providers(),
                _ => unreachable!(),
            }
            assert_eq!(
                completed.load(std::sync::atomic::Ordering::SeqCst),
                usize::from(!retained)
            );
            drop(survivor);
            assert_eq!(completed.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert_eq!(
                run(refresh_oauth_token("from-drop", credentials(0.0), None))
                    .unwrap()
                    .access,
                "drop"
            );
            reset_oauth_providers();
        }
    }
}

/// Complete the originally selected provider after a membership transition.
async fn admitted_resolution(transition: &str) {
    let (entered, admitted) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let barrier = Mutex::new(Some((entered, released)));
    let mut custom = provider("controlled", "unused");
    custom.refresh = Box::new(move |mut record, _| {
        let (entered, released) = barrier.lock().unwrap().take().unwrap();
        Box::pin(async move {
            entered.send(()).unwrap();
            released.await.unwrap();
            record.refresh = "original-refresh".into();
            record.access = "original-access".into();
            Ok(record)
        })
    });
    custom.key = Box::new(|record| Ok(&record.refresh));
    register_oauth_provider(Arc::new(custom));
    let input = indexmap::IndexMap::from([("controlled".into(), credentials(0.0))]);
    let operation = tokio::spawn(get_oauth_api_key("controlled", &input, None));
    admitted.await.unwrap();
    match transition {
        "replace" => register_oauth_provider(Arc::new(provider("controlled", "replacement"))),
        "remove" => unregister_oauth_provider("controlled"),
        "reset" => reset_oauth_providers(),
        _ => unreachable!(),
    }
    release.send(()).unwrap();
    let output = operation.await.unwrap().unwrap().unwrap();
    assert_eq!(output.api_key, "original-refresh");
    assert_eq!(output.new_credentials.access, "original-access");
    if transition == "replace" {
        assert_eq!(run_key_async(&input).await.api_key, "replacement");
    } else {
        assert!(get_oauth_provider("controlled").is_none());
    }
}
/// Obtain the same public key operation inside a running runtime.
async fn run_key_async(input: &indexmap::IndexMap<String, OAuthCredentials>) -> OAuthApiKey {
    get_oauth_api_key("controlled", input, None)
        .await
        .unwrap()
        .unwrap()
}
/// Reentrant callbacks mutate membership without redirecting extraction.
async fn reentrant_resolution() {
    let mut custom = provider("controlled", "unused");
    custom.refresh = Box::new(|mut record, _| {
        register_oauth_provider(Arc::new(provider("from-refresh", "side")));
        unregister_oauth_provider("controlled");
        record.access = "reentrant-access".into();
        Box::pin(async move { Ok(record) })
    });
    custom.key = Box::new(|record| {
        register_oauth_provider(Arc::new(provider("from-key", "side")));
        Ok(&record.access)
    });
    register_oauth_provider(Arc::new(custom));
    let input = indexmap::IndexMap::from([("controlled".into(), credentials(0.0))]);
    assert_eq!(run_key_async(&input).await.api_key, "reentrant-access");
    assert!(get_oauth_provider("controlled").is_none());
    assert!(get_oauth_provider("from-refresh").is_some());
    assert!(get_oauth_provider("from-key").is_some());
}
/// Admit two refreshes before allowing either one to finish.
async fn independent_resolutions() {
    let (entered, mut admissions) = tokio::sync::mpsc::unbounded_channel();
    let (release_one, first_barrier) = tokio::sync::oneshot::channel();
    let (release_two, second_barrier) = tokio::sync::oneshot::channel();
    let pending = Mutex::new(std::collections::VecDeque::from([
        (1, first_barrier),
        (2, second_barrier),
    ]));
    let mut custom = provider("controlled", "unused");
    custom.refresh = Box::new(move |mut record, _| {
        let (index, released) = pending.lock().unwrap().pop_front().unwrap();
        let entered = entered.clone();
        Box::pin(async move {
            entered.send(index).unwrap();
            released.await.unwrap();
            record.access = format!("access-{index}");
            Ok(record)
        })
    });
    register_oauth_provider(Arc::new(custom));
    let input = indexmap::IndexMap::from([("controlled".into(), credentials(0.0))]);
    let first = tokio::spawn(get_oauth_api_key("controlled", &input, None));
    assert_eq!(admissions.recv().await, Some(1));
    let second = tokio::spawn(get_oauth_api_key("controlled", &input, None));
    assert_eq!(admissions.recv().await, Some(2));
    release_two.send(()).unwrap();
    release_one.send(()).unwrap();
    assert_eq!(first.await.unwrap().unwrap().unwrap().api_key, "access-1");
    assert_eq!(second.await.unwrap().unwrap().unwrap().api_key, "access-2");
    unregister_oauth_provider("controlled");
    assert_eq!(admissions.recv().await, None);
}

#[test]
fn maestro_oauth_callbacks_outlive_registry_changes() {
    let _admission = Admission::enter();
    run(async {
        for transition in ["replace", "remove", "reset"] {
            admitted_resolution(transition).await;
        }
        reentrant_resolution().await;
        independent_resolutions().await;
    });
}

/// A successful refresh may supply empty tokens and a NaN expiry.
fn check_empty_refresh_result() {
    let mut empty = provider("controlled", "");
    empty.refresh = Box::new(|mut record, _| {
        record.access.clear();
        record.refresh.clear();
        record.expires = f64::NAN;
        Box::pin(async move { Ok(record) })
    });
    register_oauth_provider(Arc::new(empty));
    let input = indexmap::IndexMap::from([("controlled".into(), credentials(0.0))]);
    let output = run(get_oauth_api_key("controlled", &input, None))
        .unwrap()
        .unwrap();
    assert_eq!(output.api_key, "");
    assert_eq!(output.new_credentials.refresh, "");
    assert!(output.new_credentials.expires.is_nan());
}

/// Extraction failures stay outside the refresh catch boundary.
fn check_extraction_failures() {
    for expiry in [f64::INFINITY, 0.0] {
        let mut failing_key = provider("controlled", "new-access");
        failing_key.key = Box::new(move |record| {
            assert_eq!(
                record.access,
                if expiry.is_infinite() {
                    "old-access"
                } else {
                    "new-access"
                }
            );
            Err(error("inner-key"))
        });
        register_oauth_provider(Arc::new(failing_key));
        let input = indexmap::IndexMap::from([("controlled".into(), credentials(expiry))]);
        let failure = run(get_oauth_api_key("controlled", &input, None))
            .err()
            .unwrap();
        assert_eq!(failure.to_string(), "inner-key");
        assert_eq!(
            failure.diagnostic.code,
            Some(maestro_models::DiagnosticCode::Text(
                "inner-key-code".into()
            ))
        );
        assert_eq!(failure.cause.unwrap().to_string(), "inner-key-cause");
    }
}

/// The actual account operation consumes the supplied refresh token.
fn check_builtin_request(id: &str, request: &maestro_models::HttpRequest) {
    match id {
        "anthropic" => {
            assert_eq!(request.method, "POST");
            assert_eq!(request.url, "https://platform.claude.com/v1/oauth/token");
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["refresh_token"], "old-refresh");
            assert_eq!(body["grant_type"], "refresh_token");
        }
        "github-copilot" => {
            assert_eq!(request.method, "GET");
            assert_eq!(
                request.url,
                "https://api.github.com/copilot_internal/v2/token"
            );
            assert_eq!(request.headers["authorization"], "Bearer old-refresh");
            assert!(request.body.is_empty());
        }
        "openai-codex" => {
            assert_eq!(request.method, "POST");
            assert_eq!(request.url, "https://auth.openai.com/oauth/token");
            let form = std::str::from_utf8(&request.body).unwrap();
            assert!(form.contains("refresh_token=old-refresh"));
            assert!(form.contains("grant_type=refresh_token"));
        }
        _ => unreachable!(),
    }
}
