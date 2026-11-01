//! Provider operations exercised through the production component loader.
#![cfg(test)]
use maestro_extensions::types::ExtensionHandle;
use maestro_extensions::{
    ExtensionActions, ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local,
    types::ProviderActions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Calls the retained author command and decodes its observation.
async fn observe(runner: &ExtensionRunner<Local>, request: &str) -> Value {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let result = handler(request.into(), runner.create_command_context()).await;
    let text = result.unwrap_err().to_string();
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// Completes a native context read so queued resource releases precede the next Store call.
async fn store_barrier(runner: &ExtensionRunner<Local>) {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    handler("barrier".into(), runner.create_command_context())
        .await
        .unwrap_err();
}

/// Keeps the loaded owner alive while the original catalog is observed.
async fn loaded() -> (ExtensionHandle<Local>, ExtensionRunner<Local>) {
    let bus = super::components::TracedBus::new();
    let (owner, runner, _) =
        super::retained_components::loaded_on(bus.controller.for_extension()).await;
    (owner, runner)
}

#[test]
fn provider_boundary_rejections_leave_other_resources_usable() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let answer = observe(&runner, "provider boundary").await;
        assert_eq!(answer["key"], json!("surviving-key"));
        assert_eq!(answer["models"], json!([]));
        assert_eq!(
            answer["json"],
            json!("EOF while parsing an object at line 1 column 1")
        );
        assert_eq!(answer["providerJson"], answer["json"]);
        assert_eq!(
            answer["nonfinite"],
            json!("extension wrote a non-finite number (Infinity or NaN)")
        );
        assert_eq!(answer["errors"], json!([]));
        runner.invalidate(Some("literal boundary rejection".into()));
        assert_eq!(
            observe(&runner, "provider stale").await,
            json!({"error":"literal boundary rejection","key":"surviving-key"})
        );
    });
}

#[test]
fn provider_author_component_uses_native_catalog_without_adapter_changes() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (session, _) = super::session::runner_session("/provider-aggregate");
        let native = session.model_registry.clone();
        let runtime = queued_runtime(&json!([
            {"name":"author-provider","config":provider_config("factory")}
        ]));
        let (_owner, runner, _) = super::retained_components::loaded_with_runtime(
            bus.controller.for_extension(),
            session,
            "/provider-aggregate",
            runtime.clone(),
        )
        .await;
        assert!(native.find("author-provider", "factory").is_some());
        assert!(
            runtime
                .pending_provider_registrations
                .read()
                .unwrap()
                .is_empty()
        );
        let answer = observe(&runner, "provider catalog").await;
        assert_eq!(
            answer,
            json!({
                "label":" Literal provider ", "key":"literal-key",
                "configured":true, "status":{"configured":true,"source":"models_json_key"},
                "oauth":false, "queued":["factory"], "before":["first"], "replaced":["second"],
                "refreshed":["second"], "removed":true, "error":null,
                "auth":{"ok":true,"apiKey":"literal-key","headers":{"x-test":"literal","Authorization":"Bearer literal-key"}}
            })
        );
        assert!(native.find("author-provider", "factory").is_none());
        assert!(native.find("author-provider", "first").is_none());
        assert!(native.find("author-provider", "second").is_none());
    });
}

#[test]
fn credential_author_retains_callable_native_owner() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (session, _) = super::session::runner_session_with_config(
            "/credential-owner",
            maestro_extensions::SharedHandle::new(CredentialEffects),
        );
        let (_owner, runner, _) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        assert_eq!(
            observe(&runner, "credential owner").await,
            json!({"key":"original effect key","stored":{"type":"api_key","key":"!checkpoint-credential"},
                "listed":["author-credential"],"has":true,"auth":true,
                "status":{"configured":true,"source":"stored"},"errors":[]})
        );
        assert_eq!(
            observe(&runner, "credential retained").await,
            json!("original effect key")
        );
        assert_eq!(
            observe(&runner, "credential constructors").await,
            json!({"key":"changed-after-construction","alias":"changed-after-construction","errors":null,"stored":[]})
        );
        assert_eq!(
            observe(&runner, "credential release").await,
            json!("released")
        );
        assert_eq!(
            observe(&runner, "credential callbacks").await,
            json!({
                "keys":["changed-after-construction","accepted","original"],"errors":["write-first"],"drained":[],
                "raw":r#"{"shared":{"type":"api_key","key":"original"}}"#,
                "fallback":{"has":true,"status":{"configured":false,"source":"fallback","label":"custom provider config"},"key":"fallback-key"}
            })
        );
        assert_eq!(
            observe(&runner, "credential callbacks retained").await,
            json!("later-key")
        );
        assert_eq!(
            observe(&runner, "credential callbacks pending").await,
            json!({"key":"later-key","captures":[]})
        );
        assert_eq!(
            observe(&runner, "credential callbacks release").await,
            json!("released")
        );
        store_barrier(&runner).await;
        assert_eq!(
            observe(&runner, "credential callbacks counts").await,
            json!(["backend", "fallback"])
        );
        pending_key_cases(&runner).await;
    });
}

/// A supplied credential effect that cannot be reproduced by an ambient shell lookup.
struct CredentialEffects;
impl maestro_extensions::ConfigValueOperations for CredentialEffects {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        assert_eq!(command, "checkpoint-credential");
        Some(b" original effect key ".to_vec())
    }
}

#[test]
fn catalog_construction_forwards_auth_and_path_independently() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        assert_eq!(
            observe(&runner, "credential constructors").await,
            json!({"key":"changed-after-construction", "alias":"changed-after-construction", "errors":null,"stored":[]})
        );
    });
}

#[test]
fn catalog_file_constructors_keep_independent_paths_and_live_credentials() {
    super::components::local(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let (_owner, runner) = loaded().await;
        for auth in ["a", "b"] {
            let path = scratch.path().join(format!("auth-{auth}.json"));
            std::fs::write(
                path,
                json!({"constructor-provider":{"type":"api_key","key":format!("key-{auth}")}})
                    .to_string(),
            )
            .unwrap();
        }
        for model in ["first", "second"] {
            let path = scratch.path().join(format!("models-{model}.json"));
            let mut config = json!({"baseUrl":"https://constructor.invalid", "api":"openai-completions", "models":[{"id":model,"name":model,"reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":16}]});
            config["apiKey"] = json!("file-config-key");
            std::fs::write(
                path,
                json!({"providers":{"constructor-provider":config}}).to_string(),
            )
            .unwrap();
        }
        for (auth, model) in [
            ("a", "first"),
            ("a", "second"),
            ("b", "second"),
            ("b", "first"),
        ] {
            let request = format!(
                "credential files {}",
                json!({"auth":scratch.path().join(format!("auth-{auth}.json")),"models":scratch.path().join(format!("models-{model}.json"))})
            );
            assert_eq!(
                observe(&runner, &request).await,
                json!({"key":format!("key-{auth}"),"models":[model],"after":"later-key","error":null})
            );
        }
    });
}

#[test]
fn credential_reload_on_original_pump_reenters_backend_without_deadlock() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let answer = observe(&runner, "credential backend").await;
        assert_eq!(answer["before"], json!({"type":"api_key","key":"first"}));
        assert_eq!(answer["has"], json!(true));
        assert_eq!(answer["typed"], json!(null));
        assert_eq!(answer["errors"], json!([]));
    });
}

#[test]
fn credential_fallback_reentry_and_replacement_keep_returned_values() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        assert_eq!(
            observe(&runner, "credential fallback").await,
            json!({"values":[
            {"has":false,"status":{"configured":false},"key":null,"without":null},
            {"has":false,"status":{"configured":false},"key":"","without":null},
            {"has":true,"status":{"configured":false,"source":"fallback","label":"custom provider config"},"key":"fallback-key","without":null}],
            "error":"literal fallback failure","recovered":false})
        );
    });
}

/// Controlled OAuth refresh whose native future really waits for the caller's completion.
struct PendingProvider {
    /// Signals that locked refresh reached the native provider.
    started: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// No timer or network supplies this completion.
    released: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<Result<(), &'static str>>>>,
}
impl maestro_models::OAuthProviderInterface for PendingProvider {
    fn id(&self) -> &'static str {
        "author-pending"
    }
    fn name(&self) -> &'static str {
        "Pending provider"
    }
    fn login(
        &self,
        _: maestro_models::OAuthCallbacks,
        _: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > {
        Box::pin(async {
            Err(maestro_models::DiagnosticErrorInfo::error("login not requested").into())
        })
    }
    fn refresh_token(
        &self,
        mut credentials: maestro_models::OAuthCredentials,
        _: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > {
        let started = self.started.lock().unwrap().take().unwrap();
        let released = self.released.lock().unwrap().take().unwrap();
        Box::pin(async move {
            started.send(()).unwrap();
            released
                .await
                .unwrap()
                .map_err(maestro_models::DiagnosticErrorInfo::error)?;
            credentials.access = "refreshed-key".into();
            credentials.expires = 4_102_444_800_000.0;
            Ok(credentials)
        })
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a maestro_models::OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, maestro_models::OAuthError> {
        Ok(std::borrow::Cow::Borrowed(&credentials.access))
    }
}

#[test]
fn credential_pending_key_retains_borrowed_update_and_owner_until_settlement() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        maestro_models::register_oauth_provider(std::sync::Arc::new(PendingProvider {
            started: std::sync::Mutex::new(Some(started)),
            released: std::sync::Mutex::new(Some(released)),
        }));
        assert_eq!(
            observe(&runner, "credential pending create").await,
            json!("retained")
        );
        let key = observe(&runner, "credential pending key");
        let during = async {
            entered.await.unwrap();
            assert_eq!(
                observe(&runner, "credential pending release").await,
                json!("released")
            );
            release.send(Ok(())).unwrap();
        };
        let (answer, ()) = tokio::join!(key, during);
        assert_eq!(answer, json!({"key":"refreshed-key","errors":[]}));
        assert_eq!(
            observe(&runner, "credential pending raw").await,
            json!({"author-pending":{"type":"oauth","refresh":"original-refresh","access":"refreshed-key","expires":4_102_444_800_000_i64},"unknown":[null,false,2]})
        );
        maestro_models::unregister_oauth_provider("author-pending");
    });
}

#[test]
fn credential_backend_callbacks_keep_lock_update_outcomes() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        for (phase, error) in [
            ("acquire", Some("awaited acquisition failed")),
            ("write", Some("awaited write failed")),
            ("reread", None),
        ] {
            backend_outcome_case(&runner, phase, error).await;
        }
    });
}

/// Runs one real native awaited-backend failure or no-write outcome.
async fn backend_outcome_case(runner: &ExtensionRunner<Local>, phase: &str, error: Option<&str>) {
    let (started, entered) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let provider = std::sync::Arc::new(PendingProvider {
        started: std::sync::Mutex::new(Some(started)),
        released: std::sync::Mutex::new(Some(released)),
    });
    maestro_models::register_oauth_provider(provider.clone());
    assert_eq!(
        observe(runner, &format!("credential pending create-{phase}")).await,
        json!("retained")
    );
    let key = observe(runner, "credential pending key");
    let answer = if phase == "write" {
        let during = async {
            entered.await.unwrap();
            release.send(Ok(())).unwrap();
        };
        tokio::join!(key, during).0
    } else {
        let answer = key.await;
        assert!(
            provider.started.lock().unwrap().is_some(),
            "update entered the OAuth provider"
        );
        answer
    };
    assert_eq!(answer["key"], Value::Null);
    let errors = answer["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["message"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(errors, error.into_iter().collect::<Vec<_>>());
    assert_backend_storage(runner, phase).await;
    assert_pending_release(runner).await;
    maestro_models::unregister_oauth_provider("author-pending");
}

/// Checks the final backend and resolver captures only after the native release barrier.
async fn assert_pending_release(runner: &ExtensionRunner<Local>) {
    assert_eq!(
        observe(runner, "credential pending counts").await,
        json!([])
    );
    observe(runner, "credential pending release").await;
    store_barrier(runner).await;
    assert_eq!(
        observe(runner, "credential pending counts").await,
        json!(["backend", "fallback"])
    );
}

/// Compares native accepted state with the independently retained raw backend text.
async fn assert_backend_storage(runner: &ExtensionRunner<Local>, phase: &str) {
    let updates = observe(runner, "credential pending updates").await;
    let raw = observe(runner, "credential pending raw").await;
    let stored = observe(runner, "credential pending stored").await;
    assert_backend_update(runner, phase, &updates, &raw).await;
    if phase == "reread" {
        assert_eq!(raw, json!({"unknown":[null,false,2]}));
        assert_eq!(
            observe(runner, "credential pending text").await,
            json!(" \n{\"unknown\":[null,false,2]} \n")
        );
        assert_eq!(stored, Value::Null);
    } else {
        assert_eq!(raw["author-pending"]["access"], json!("old"));
        assert_eq!(
            credential_value(&stored),
            credential_value(&raw["author-pending"])
        );
    }
}

/// Verifies the actual native replacement before the authored backend returns.
async fn assert_backend_update(
    runner: &ExtensionRunner<Local>,
    phase: &str,
    updates: &Value,
    raw: &Value,
) {
    if phase == "acquire" {
        assert_eq!(*updates, json!([]));
    } else {
        let update = &updates[0];
        assert_eq!(updates.as_array().unwrap().len(), 1);
        assert_eq!(
            update["current"],
            observe(runner, "credential pending text").await
        );
        assert_eq!(
            serde_json::from_str::<Value>(update["current"].as_str().unwrap()).unwrap(),
            *raw
        );
        if phase == "write" {
            assert_eq!(update["accepted"]["access"], json!("refreshed-key"));
            let replacement: Value =
                serde_json::from_str(update["next"].as_str().unwrap()).unwrap();
            assert_eq!(
                credential_value(&replacement["author-pending"]),
                credential_value(&update["accepted"])
            );
            assert_eq!(replacement["unknown"], json!([null, false, 2]));
        } else {
            assert_eq!(update["next"], Value::Null);
            assert_eq!(update["accepted"], Value::Null);
        }
    }
}

/// Reads the declared credential view without equating JSON integer and float variants.
fn credential_value(value: &Value) -> maestro_extensions::AuthCredential {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn credential_pending_key_retains_owner_until_settlement() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        pending_key_cases(&runner).await;
    });
}

/// Independently selected credential and model fixtures.
struct ConstructorFiles {
    /// Default and explicit credential paths.
    auth: [String; 2],
    /// Default and explicit model paths.
    models: [String; 2],
}
/// Writes unequal documents so selecting the wrong path cannot pass.
fn constructor_files(root: &std::path::Path) -> ConstructorFiles {
    let auth = ["default", "explicit"].map(|kind| {
        root.join(format!("{kind}-auth.json"))
            .to_string_lossy()
            .into_owned()
    });
    let models = ["default", "explicit"].map(|kind| {
        root.join(format!("{kind}-models.json"))
            .to_string_lossy()
            .into_owned()
    });
    for (path, key) in [(&auth[0], "default-auth"), (&auth[1], "explicit-auth")] {
        std::fs::write(
            path,
            json!({"constructor-provider":{"type":"api_key","key":key}}).to_string(),
        )
        .unwrap();
    }
    for (path, id) in [
        (&models[0], "default-model"),
        (&models[1], "explicit-model"),
    ] {
        std::fs::write(path,json!({"providers":{"constructor-provider":{"baseUrl":"https://constructor.invalid","api":"openai-completions","apiKey":"unused-model-key","models":[{"id":id,"name":id,"reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":16}]}}}).to_string()).unwrap();
    }
    ConstructorFiles { auth, models }
}
/// Loads a real component with separately counted default-path callbacks.
async fn constructor_case(
    files: &ConstructorFiles,
    choice: [Option<&String>; 2],
) -> (Value, [usize; 2]) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let auth_calls = Arc::new(AtomicUsize::new(0));
    let model_calls = Arc::new(AtomicUsize::new(0));
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let (session, _) = super::session::runner_session("/factory-constructors");
    let (auth_path, models_path) = (files.auth[0].clone(), files.models[0].clone());
    let (calls, path_calls) = (auth_calls.clone(), model_calls.clone());
    runtime.set_catalog_construction(maestro_extensions::CatalogConstruction {
        auth_path: Arc::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(auth_path.clone())
        }),
        models_path: Arc::new(move || {
            path_calls.fetch_add(1, Ordering::SeqCst);
            Ok(models_path.clone())
        }),
        config_values: session.model_registry.config_values().clone(),
    });
    runtime.flag_values.write().unwrap().insert(
        "catalog-construction".into(),
        maestro_extensions::types::FlagValue::Boolean(true),
    );
    for (flag, path) in [
        ("constructor-auth-path", choice[0]),
        ("constructor-model-path", choice[1]),
    ] {
        if let Some(path) = path {
            runtime.flag_values.write().unwrap().insert(
                flag.into(),
                maestro_extensions::types::FlagValue::String(path.clone()),
            );
        }
    }
    let bus = super::components::TracedBus::new();
    let (_owner, runner, _) = super::retained_components::loaded_with_runtime(
        bus.controller.for_extension(),
        session,
        "/factory-constructors",
        runtime,
    )
    .await;
    (
        observe(&runner, "credential factory").await,
        [
            auth_calls.load(Ordering::SeqCst),
            model_calls.load(Ordering::SeqCst),
        ],
    )
}
#[test]
fn catalog_factory_construction_resolves_only_absent_paths_before_bind() {
    super::components::local(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let files = constructor_files(scratch.path());
        for (auth, models, key, id) in [
            (None, None, "default-auth", "default-model"),
            (Some(&files.auth[1]), None, "explicit-auth", "default-model"),
            (
                None,
                Some(&files.models[1]),
                "default-auth",
                "explicit-model",
            ),
            (
                Some(&files.auth[1]),
                Some(&files.models[1]),
                "explicit-auth",
                "explicit-model",
            ),
        ] {
            let (answer, calls) = constructor_case(&files, [auth, models]).await;
            assert_eq!(
                answer,
                json!({"models":[id],"stored":{"type":"api_key","key":key},"key":"changed-in-factory"})
            );
            assert_eq!(
                calls,
                [usize::from(auth.is_none()), usize::from(models.is_none())]
            );
        }
    });
}

#[test]
fn credential_raw_backend_keeps_persisted_field_domains() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let answer = observe(&runner, "credential backend").await;
        assert_eq!(
            answer["raw"],
            json!({"stored":{"type":"api_key","key":"first"},"untyped":null,"string":"opaque","number":1.5,"boolean":true,"object":{"type":"api_key","key":false},"array":[null,false],"oauth":{"type":"oauth","refresh":"r","access":"a","expires":42.5,"extra":["x","x"]},"other":{"type":"api_key","key":"second"}})
        );
        assert_eq!(
            answer["typedDomains"],
            json!([null, null, null, null, null, null])
        );
        assert_eq!(
            answer["presentDomains"],
            json!([true, true, true, true, true, true])
        );
        assert_eq!(
            answer["all"],
            json!({"stored":{"type":"api_key","key":"first"},"oauth":{"type":"oauth","refresh":"r","access":"a","expires":42.5,"extra":["x","x"]},"other":{"type":"api_key","key":"second"}})
        );
        assert_eq!(
            answer["list"],
            json!([
                "stored", "untyped", "string", "number", "boolean", "object", "array", "oauth",
                "other"
            ])
        );
        assert_eq!(answer["missing"], json!([false, null]));
    });
}

/// Supplies complete authored models without borrowing a native registration record.
fn provider_config(id: &str) -> Value {
    json!({"baseUrl":"https://queue.invalid", "apiKey":"queue-key", "api":"openai-completions",
        "models":[{"id":id,"name":id,"reasoning":false,"input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":16}]})
}

/// Supplies literal factory operations before the production loader starts.
fn queued_runtime(operations: &Value) -> std::rc::Rc<maestro_extensions::ExtensionRuntime<Local>> {
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    runtime.flag_values.write().unwrap().insert(
        "provider-operations".into(),
        maestro_extensions::types::FlagValue::String(operations.to_string()),
    );
    runtime
}

#[test]
fn provider_registration_queues_and_removes_before_bind() {
    super::components::local(async {
        let (session, _) = super::session::runner_session("/provider-queue");
        let catalog = session.model_registry.clone();
        let runtime = queued_runtime(&json!([
            {"name":"queue-remove", "config":provider_config("cancel-first")},
            {"name":" Queue-remove ", "config":provider_config("literal")},
            {"name":"queue-other", "config":provider_config("middle")},
            {"name":"queue-remove", "config":provider_config("cancel-second")},
            {"name":"queue-tail", "config":provider_config("last")},
            {"name":"queue-remove"}, {"name":"absent"}
        ]));
        let bus = super::components::TracedBus::new();
        let (_owner, _runner, _) = super::retained_components::loaded_with_runtime(
            bus.controller.for_extension(),
            session,
            "/provider-queue",
            runtime.clone(),
        )
        .await;
        let list = catalog.get_all();
        let actual = list
            .read()
            .unwrap()
            .iter()
            .filter_map(|model| {
                let model = model.read().unwrap();
                (model.provider.contains("queue") || model.provider.contains("Queue"))
                    .then(|| (model.provider.clone(), model.id.clone()))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                (" Queue-remove ".into(), "literal".into()),
                ("queue-other".into(), "middle".into()),
                ("queue-tail".into(), "last".into())
            ]
        );
        assert!(
            runtime
                .pending_provider_registrations
                .read()
                .unwrap()
                .is_empty()
        );
    });
}

/// Executes one author-side operation and returns the retained catalog observation.
async fn provider_call(runner: &ExtensionRunner<Local>, operation: Value) -> Value {
    observe(runner, &format!("provider control {operation}")).await
}

#[test]
fn provider_validation_preserves_field_presence() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let valid = provider_config("original");
        let before = provider_call(
            &runner,
            json!({"op":"register","provider":"validation","config":valid}),
        )
        .await;
        assert_eq!(before["error"], Value::Null);
        let cases = validation_cases(&valid);
        for (config, error) in cases {
            let answer = provider_call(
                &runner,
                json!({"op":"register","provider":"validation","config":config}),
            )
            .await;
            assert_eq!(answer["error"], json!(error));
            for field in ["models", "key", "auth", "label", "status"] {
                assert_eq!(answer[field], before[field], "{field}");
            }
        }
        valid_provider_fields(&runner, valid).await;
    });
}

#[test]
fn provider_bind_errors_keep_source_and_continue() {
    super::components::local(async {
        let (session, _) = super::session::runner_session("/provider-errors");
        let catalog = session.model_registry.clone();
        let runtime = queued_runtime(&json!([
            {"name":"bad-queued", "config":{"models":[provider_config("bad")["models"][0].clone()]}},
            {"name":"good-queued", "config":provider_config("survivor")}
        ]));
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) = super::retained_components::unbound_with_runtime(
            bus.controller.for_extension(),
            session,
            "/provider-errors",
            runtime.clone(),
        )
        .await;
        let runner = Rc::new(runner);
        let errors = Rc::new(RefCell::new(Vec::new()));
        let (seen, calls, actions) = (errors.clone(), runner.clone(), runtime.clone());
        let unsubscribe = runner.on_error(Rc::new(move |error| {
            assert_eq!(
                calls.create_context().get_system_prompt()?,
                "bound before flush"
            );
            let get_tools = actions.actions.read().unwrap().get_active_tools.clone();
            assert_eq!(get_tools()?, ["bound-tool"]);
            seen.borrow_mut().push((
                error.extension_path.clone(),
                error.event.clone(),
                error.error.clone(),
                error.stack.clone(),
            ));
            Ok(())
        }));
        bind_flush_controls(&runner);
        unsubscribe();
        assert_eq!(
            *errors.borrow(),
            [(
                std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
                "register_provider".into(),
                "Provider bad-queued: \"baseUrl\" is required when defining models.".into(),
                None
            )]
        );
        assert!(catalog.find("bad-queued", "bad").is_none());
        assert!(catalog.find("good-queued", "survivor").is_some());
        assert!(
            runtime
                .pending_provider_registrations
                .read()
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn provider_registration_after_bind_is_immediate() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (session, _) = super::session::runner_session("/provider-immediate");
        let catalog = session.model_registry.clone();
        let (_owner, runner, events) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        let runner = Rc::new(runner);
        let calls = Rc::new(RefCell::new(Vec::new()));
        registration_action_case(&runner, &events, &catalog, &calls).await;
        removal_action_case(&runner, &events, &catalog, &calls).await;
        assert_eq!(
            *calls.borrow(),
            [
                "register:immediate",
                "register:immediate",
                "remove:immediate"
            ]
        );
        super::support::bind_prompt(&runner, "release supplied actions");
    });
}

#[test]
fn catalog_local_mutation_does_not_requeue_extension_registration() {
    super::components::local(async {
        let (session, _) = super::session::runner_session("/provider-local");
        let catalog = session.model_registry.clone();
        let runtime = maestro_extensions::create_extension_runtime::<Local>();
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) = super::retained_components::unbound_with_runtime(
            bus.controller.for_extension(),
            session,
            "/provider-local",
            runtime.clone(),
        )
        .await;
        local_mutations(&runner, &runtime).await;
        let queued = provider_call(
            &runner,
            json!({"op":"register","provider":"queued-invalid","config":{"models":provider_config("invalid")["models"]}}),
        )
        .await;
        assert_eq!(queued["error"], Value::Null);
        assert_eq!(queued["models"], json!([]));
        provider_call(
            &runner,
            json!({"op":"register","provider":"queued-valid","config":provider_config("later")}),
        )
        .await;
        assert_eq!(
            runtime.pending_provider_registrations.read().unwrap().len(),
            2
        );
        let errors = Rc::new(RefCell::new(Vec::new()));
        let seen = errors.clone();
        let unsubscribe = runner.on_error(Rc::new(move |error| {
            seen.borrow_mut().push((
                error.extension_path.clone(),
                error.event.clone(),
                error.error.clone(),
            ));
            Ok(())
        }));
        super::support::bind_prompt(&runner, "bind local catalog");
        unsubscribe();
        assert_eq!(
            *errors.borrow(),
            [(
                std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
                "register_provider".into(),
                "Provider queued-invalid: \"baseUrl\" is required when defining models.".into()
            )]
        );
        assert!(catalog.find("local-only", "retained").is_some());
        assert!(catalog.find("queued-valid", "later").is_some());
        assert!(catalog.find("queued-invalid", "invalid").is_none());
        assert!(
            runtime
                .pending_provider_registrations
                .read()
                .unwrap()
                .is_empty()
        );
    });
}

/// A real file catalog whose credentials and command effects remain the supplied owners.
fn file_session(path: &std::path::Path) -> maestro_extensions::runner::ExtensionRunnerSession {
    let (mut session, _) = super::session::runner_session("/provider-file");
    let original = session.model_registry.clone();
    session.model_registry = maestro_extensions::SharedHandle::new(
        maestro_catalog::ModelRegistry::create(
            original.auth_storage().clone(),
            path.to_string_lossy().into_owned(),
            original.config_values().clone(),
        )
        .unwrap(),
    );
    session
}

#[test]
fn catalog_refresh_retains_readers_and_drains_load_error() {
    super::components::local(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let path = scratch.path().join("models.json");
        std::fs::write(&path, r#"{"providers":{"broken":{"apiKey":false}}}"#).unwrap();
        let session = file_session(&path);
        let native = session.model_registry.clone();
        let shipped = maestro_models::get_models("openrouter");
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        let retained = provider_call(&runner, json!({"op":"retain","provider":"openrouter"})).await;
        assert_eq!(
            retained["loadError"],
            json!(format!(
                "Invalid models.json schema:\n  - providers.broken.apiKey: must be string\n\nFile: {}",
                path.display()
            ))
        );
        assert_eq!(retained["models"], serde_json::to_value(&shipped).unwrap());
        assert!(!shipped.is_empty());
        runner.invalidate(Some(
            "context retired, retained owners remain callable".into(),
        ));
        let dynamic = provider_call(&runner, json!({"op":"local-register","provider":"refresh-dynamic","config":provider_config("dynamic"),"retained":true})).await;
        assert_eq!(dynamic["models"][0]["id"], json!("dynamic"));
        native
            .auth_storage()
            .set_runtime_api_key("refresh-dynamic", "changed after callback");
        for missing in [false, true] {
            if missing {
                std::fs::remove_file(&path).unwrap();
            } else {
                std::fs::write(&path, r#"{"providers":{}}"#).unwrap();
            }
            let answer = provider_call(
                &runner,
                json!({"op":"refresh","provider":"refresh-dynamic","retained":true}),
            )
            .await;
            assert_eq!(answer["error"], Value::Null);
            assert_eq!(answer["loadError"], Value::Null);
            assert_eq!(answer["models"][0]["id"], json!("dynamic"));
            assert_eq!(answer["key"], json!("changed after callback"));
            assert_eq!(answer["credentialKey"], json!("changed after callback"));
            let repeat = provider_call(
                &runner,
                json!({"op":"inspect","provider":"refresh-dynamic","retained":true}),
            )
            .await;
            assert_eq!(repeat["loadError"], Value::Null);
        }
    });
}

#[test]
fn provider_display_keeps_empty_and_literal_labels() {
    super::components::local(async {
        let (session, _) = super::session::runner_session("/provider-display");
        let native = session.model_registry.clone();
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        let oauth = std::sync::Arc::new(PendingProvider {
            started: std::sync::Mutex::new(None),
            released: std::sync::Mutex::new(None),
        });
        maestro_models::register_oauth_provider(oauth.clone());
        let current =
            provider_call(&runner, json!({"op":"inspect","provider":"author-pending"})).await;
        assert_eq!(current["label"], json!("Pending provider"));
        native
            .register_provider(
                "registered-oauth",
                maestro_extensions::SharedHandle::new(std::sync::RwLock::new(
                    maestro_extensions::ProviderConfig {
                        callbacks: maestro_extensions::ProviderCallbacks {
                            oauth: Some(oauth),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )),
            )
            .unwrap();
        maestro_models::unregister_oauth_provider("registered-oauth");
        assert_eq!(
            provider_call(
                &runner,
                json!({"op":"inspect","provider":"registered-oauth"})
            )
            .await["label"],
            json!("Pending provider")
        );
        for name in ["", " Literal label "] {
            assert_eq!(
                provider_call(
                    &runner,
                    json!({"op":"register","provider":"registered-oauth","config":{"name":name}})
                )
                .await["label"],
                json!(name)
            );
        }
        for (provider, expected) in [
            ("openrouter", "OpenRouter"),
            (" Unknown Provider ", " Unknown Provider "),
            ("", ""),
        ] {
            assert_eq!(
                provider_call(&runner, json!({"op":"inspect","provider":provider})).await["label"],
                json!(expected)
            );
        }
        maestro_models::unregister_oauth_provider("author-pending");
    });
}

/// Keeps descriptor order, URL and consuming request-auth results in a small observation.
fn provider_projection(answer: &Value) -> Value {
    let models = answer["models"].as_array().unwrap();
    json!([
        models
            .iter()
            .map(|model| model["id"].clone())
            .collect::<Vec<_>>(),
        models
            .iter()
            .map(|model| model["baseUrl"].clone())
            .collect::<Vec<_>>(),
        answer["key"],
        answer["auth"]
    ])
}

#[test]
fn provider_replacement_refresh_and_removal_preserve_owner_policy() {
    super::components::local(async {
        let cases = replay_cases();
        for (patch, immediate, replay) in cases {
            let (_owner, runner) = loaded().await;
            let mut config = provider_config("old");
            config["headers"] = json!({"x-old":"old"});
            config["authHeader"] = json!(true);
            provider_call(
                &runner,
                json!({"op":"register","provider":"replay","config":config}),
            )
            .await;
            provider_call(
                &runner,
                json!({"op":"register","provider":"unrelated","config":provider_config("other")}),
            )
            .await;
            let answer = provider_call(
                &runner,
                json!({"op":"register","provider":"replay","config":patch}),
            )
            .await;
            assert_eq!(answer["error"], Value::Null);
            assert_eq!(provider_projection(&answer), immediate, "incoming {patch}");
            let answer = provider_call(&runner, json!({"op":"refresh","provider":"replay"})).await;
            assert_eq!(provider_projection(&answer), replay, "replayed {patch}");
            let other =
                provider_call(&runner, json!({"op":"inspect","provider":"unrelated"})).await;
            assert_eq!(other["models"][0]["id"], json!("other"));
            assert_eq!(other["key"], json!("queue-key"));
        }
        provider_restoration().await;
    });
}

/// Reads the real catalog's selected identities, retaining its publication order.
fn selected(catalog: &maestro_catalog::ModelRegistry, providers: &[&str]) -> Vec<(String, String)> {
    catalog
        .get_all()
        .read()
        .unwrap()
        .iter()
        .filter_map(|model| {
            let model = model.read().unwrap();
            providers
                .contains(&model.provider.as_str())
                .then(|| (model.provider.clone(), model.id.clone()))
        })
        .collect()
}

/// Replaces complete lists, replays first registration order and restores file/shipped models.
async fn provider_restoration() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("models.json");
    std::fs::write(&path, json!({"providers":{"restored":provider_config("file"),"anthropic":{"baseUrl":"https://file.invalid"}}}).to_string()).unwrap();
    let session = file_session(&path);
    let native = session.model_registry.clone();
    let shipped = selected(&native, &["anthropic"]);
    let bus = super::components::TracedBus::new();
    let (_owner, runner, _) =
        super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
    provider_replacement_order(&runner, &native).await;
    provider_call(
        &runner,
        json!({"op":"register","provider":"anthropic","config":provider_config("override")}),
    )
    .await;
    provider_call(&runner, json!({"op":"remove","provider":"anthropic"})).await;
    assert!(!shipped.is_empty());
    assert_eq!(selected(&native, &["anthropic"]), shipped);
    assert!(
        native
            .get_all()
            .read()
            .unwrap()
            .iter()
            .filter(|model| model.read().unwrap().provider == "anthropic")
            .all(|model| model.read().unwrap().base_url == "https://file.invalid")
    );
    std::fs::write(&path, "{").unwrap();
    let before = native.get_all();
    let answer = provider_call(
        &runner,
        json!({"op":"remove","provider":"never-registered"}),
    )
    .await;
    assert_eq!(answer["loadError"], Value::Null);
    assert!(std::sync::Arc::ptr_eq(&before, &native.get_all()));
    assert!(native.find("other", "other").is_some());
}

/// Observes incoming full replacement and retained insertion order through a reload.
async fn provider_replacement_order(
    runner: &ExtensionRunner<Local>,
    native: &maestro_catalog::ModelRegistry,
) {
    provider_call(
        runner,
        json!({"op":"register","provider":"restored","config":provider_config("first")}),
    )
    .await;
    provider_call(
        runner,
        json!({"op":"register","provider":"other","config":provider_config("other")}),
    )
    .await;
    let mut replacement = provider_config("second");
    replacement["models"]
        .as_array_mut()
        .unwrap()
        .push(provider_config("third")["models"][0].clone());
    let answer = provider_call(
        runner,
        json!({"op":"register","provider":"restored","config":replacement}),
    )
    .await;
    assert_eq!(
        provider_projection(&answer),
        json!([["second","third"],["https://queue.invalid","https://queue.invalid"],"queue-key",{"ok":true,"apiKey":"queue-key","headers":null}])
    );
    assert_eq!(
        selected(native, &["restored", "other"]),
        [
            ("other".into(), "other".into()),
            ("restored".into(), "second".into()),
            ("restored".into(), "third".into())
        ]
    );
    provider_call(runner, json!({"op":"refresh","provider":"restored"})).await;
    assert_eq!(
        selected(native, &["restored", "other"]),
        [
            ("restored".into(), "second".into()),
            ("restored".into(), "third".into()),
            ("other".into(), "other".into())
        ]
    );
    let answer = provider_call(runner, json!({"op":"remove","provider":"restored"})).await;
    assert_eq!(answer["models"][0]["id"], json!("file"));
}

/// Invalid fields in consuming order, including later model protocol failure.
fn validation_cases(valid: &Value) -> Vec<(Value, &'static str)> {
    let url = "Provider validation: \"baseUrl\" is required when defining models.";
    let key = "Provider validation: \"apiKey\" or \"oauth\" is required when defining models.";
    let api = "Provider validation, model original: no \"api\" specified.";
    let mut cases = vec![
        (json!({"models":valid["models"]}), url),
        (
            json!({"baseUrl":"https://queue.invalid","models":valid["models"]}),
            key,
        ),
    ];
    for field in ["baseUrl", "apiKey", "api"] {
        for value in [None, Some(json!(""))] {
            let mut config = valid.clone();
            config.as_object_mut().unwrap().shift_remove(field);
            if let Some(value) = value {
                config[field] = value;
            }
            config["models"][0]["baseUrl"] = json!("https://model.invalid");
            cases.push((
                config,
                match field {
                    "baseUrl" => url,
                    "apiKey" => key,
                    _ => api,
                },
            ));
        }
    }
    let mut later = valid.clone();
    later.as_object_mut().unwrap().shift_remove("api");
    later["models"][0]["api"] = json!("openai-responses");
    let mut invalid_model = valid["models"][0].clone();
    invalid_model["id"] = json!("later");
    later["models"].as_array_mut().unwrap().push(invalid_model);
    cases.push((
        later,
        "Provider validation, model later: no \"api\" specified.",
    ));
    cases
}

/// Empty collections, whitespace and model-level protocol alternatives remain admitted.
async fn valid_provider_fields(runner: &ExtensionRunner<Local>, valid: Value) {
    for config in [
        json!({}),
        json!({"models":[]}),
        json!({"models":[],"baseUrl":"","apiKey":"","api":""}),
    ] {
        assert_eq!(
            provider_call(
                runner,
                json!({"op":"register","provider":"validation","config":config})
            )
            .await["error"],
            Value::Null
        );
    }
    let mut whitespace = valid.clone();
    for field in ["baseUrl", "apiKey", "api"] {
        whitespace[field] = json!(" ");
    }
    let answer = provider_call(
        runner,
        json!({"op":"register","provider":"validation","config":whitespace}),
    )
    .await;
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["models"][0]["baseUrl"], json!(" "));
    assert_eq!(answer["models"][0]["api"], json!(" "));
    let mut empty_model_api = valid.clone();
    empty_model_api["models"][0]["api"] = json!("");
    let answer = provider_call(
        runner,
        json!({"op":"register","provider":"validation","config":empty_model_api}),
    )
    .await;
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["models"][0]["api"], json!("openai-completions"));
    let mut model_api = valid;
    model_api.as_object_mut().unwrap().shift_remove("api");
    model_api["models"][0]["api"] = json!("openai-responses");
    let answer = provider_call(
        runner,
        json!({"op":"register","provider":"validation","config":model_api}),
    )
    .await;
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["models"][0]["api"], json!("openai-responses"));
}

/// Installs independently observable operations before the registration queue flushes.
fn bind_flush_controls(runner: &ExtensionRunner<Local>) {
    runner
        .bind_core(
            ExtensionActions {
                get_active_tools: Rc::new(|| Ok(vec!["bound-tool".into()])),
                ..Local::unbound_actions()
            },
            ExtensionContextActions {
                get_system_prompt: Rc::new(|| Ok("bound before flush".into())),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Ordered observations from supplied registration and removal actions.
type ProviderCalls = Rc<RefCell<Vec<String>>>;

/// Registers and replaces through the supplied action while removal uses the catalog.
async fn registration_action_case(
    runner: &Rc<ExtensionRunner<Local>>,
    events: &maestro_extensions::EventBus<Local>,
    catalog: &maestro_extensions::SharedHandle<maestro_catalog::ModelRegistry>,
    calls: &ProviderCalls,
) {
    let (native, seen, reenter, nested) = (
        catalog.clone(),
        calls.clone(),
        runner.clone(),
        events.clone(),
    );
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions {
                register_provider: Some(Rc::new(move |name, config| {
                    native.register_provider(&name, config)?;
                    let view =
                        super::retained_components::view(&reenter, &nested, "provider-action")?
                            .unwrap();
                    assert_eq!(view.try_render(11).unwrap(), ["provider-action:11"]);
                    seen.borrow_mut().push(format!("register:{name}"));
                    Ok(())
                })),
                unregister_provider: None,
            },
        )
        .unwrap();
    for id in ["first", "replacement"] {
        let answer = provider_call(
            runner,
            json!({"op":"register","provider":"immediate","config":provider_config(id)}),
        )
        .await;
        assert_eq!(answer["error"], Value::Null);
        assert_eq!(answer["models"][0]["id"], json!(id));
        assert!(catalog.find("immediate", id).is_some());
    }
    assert!(
        provider_call(runner, json!({"op":"remove","provider":"immediate"})).await["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// Removes through the supplied action while registration uses the catalog.
async fn removal_action_case(
    runner: &Rc<ExtensionRunner<Local>>,
    events: &maestro_extensions::EventBus<Local>,
    catalog: &maestro_extensions::SharedHandle<maestro_catalog::ModelRegistry>,
    calls: &ProviderCalls,
) {
    let (native, seen, reenter, nested) = (
        catalog.clone(),
        calls.clone(),
        runner.clone(),
        events.clone(),
    );
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions {
                register_provider: None,
                unregister_provider: Some(Rc::new(move |name| {
                    native.unregister_provider(&name)?;
                    let view =
                        super::retained_components::view(&reenter, &nested, "provider-remove")?
                            .unwrap();
                    assert_eq!(view.try_render(13).unwrap(), ["provider-remove:13"]);
                    seen.borrow_mut().push(format!("remove:{name}"));
                    Ok(())
                })),
            },
        )
        .unwrap();
    let answer = provider_call(
        runner,
        json!({"op":"register","provider":"immediate","config":provider_config("fallback")}),
    )
    .await;
    assert_eq!(answer["models"][0]["id"], json!("fallback"));
    let answer = provider_call(runner, json!({"op":"remove","provider":"immediate"})).await;
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["models"], json!([]));
}

/// Direct catalog writes take effect while the facade queue remains empty.
async fn local_mutations(
    runner: &ExtensionRunner<Local>,
    runtime: &maestro_extensions::ExtensionRuntime<Local>,
) {
    let direct = provider_call(
        runner,
        json!({"op":"local-register","provider":"local-only","config":provider_config("direct")}),
    )
    .await;
    assert_eq!(direct["error"], Value::Null);
    assert_eq!(direct["models"][0]["id"], json!("direct"));
    assert!(
        runtime
            .pending_provider_registrations
            .read()
            .unwrap()
            .is_empty()
    );
    let removed = provider_call(runner, json!({"op":"local-remove","provider":"local-only"})).await;
    assert_eq!(removed["models"], json!([]));
    provider_call(
        runner,
        json!({"op":"local-register","provider":"local-only","config":provider_config("retained")}),
    )
    .await;
}

/// Independent incoming and replayed request-auth expectations.
fn replay_cases() -> [(Value, Value, Value); 6] {
    let auth = json!({"ok":true,"apiKey":"queue-key","headers":{"x-old":"old","Authorization":"Bearer queue-key"}});
    let original = json!([["old"], ["https://queue.invalid"], "queue-key", auth]);
    [
        (json!({}), original.clone(), original.clone()),
        (
            json!({"models":[]}),
            original.clone(),
            json!([[], [], "queue-key", null]),
        ),
        (
            json!({"baseUrl":""}),
            original.clone(),
            json!([["old"], [""], "queue-key", auth]),
        ),
        (
            json!({"headers":{"x-new":"new"}}),
            json!([["old"],["https://queue.invalid"],null,{"ok":true,"apiKey":null,"headers":{"x-new":"new"}}]),
            json!([["old"],["https://queue.invalid"],"queue-key",{"ok":true,"apiKey":"queue-key","headers":{"x-new":"new","Authorization":"Bearer queue-key"}}]),
        ),
        (
            json!({"authHeader":false}),
            original.clone(),
            json!([["old"],["https://queue.invalid"],"queue-key",{"ok":true,"apiKey":"queue-key","headers":{"x-old":"old"}}]),
        ),
        (
            json!({"apiKey":""}),
            original,
            json!([["old"],["https://queue.invalid"],null,{"ok":false,"error":"No API key found for \"replay\""}]),
        ),
    ]
}

#[test]
fn credential_fallback_reentry_and_replacement_keep_lifetime() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        assert_eq!(
            observe(&runner, "credential lifetime create").await,
            json!(true)
        );
        assert_eq!(
            observe(&runner, "credential lifetime counts").await,
            json!([])
        );
        assert_eq!(
            observe(&runner, "credential lifetime replace").await,
            json!(true)
        );
        assert_eq!(
            observe(&runner, "credential lifetime counts").await,
            json!([[1, true]])
        );
        assert_eq!(
            observe(&runner, "credential lifetime release").await,
            json!("released")
        );
        store_barrier(&runner).await;
        assert_eq!(
            observe(&runner, "credential lifetime counts").await,
            json!([[1, true], [2, true]])
        );
        assert_eq!(
            observe(&runner, "credential lifetime counts").await,
            json!([[1, true], [2, true]])
        );
    });
}

#[test]
fn credential_session_fallback_releases_after_runner_and_guest_handles() {
    super::components::local(async {
        let (session, _) = super::session::runner_session("/session-fallback");
        let catalog = std::sync::Arc::downgrade(&session.model_registry);
        let credentials = std::sync::Arc::downgrade(session.model_registry.auth_storage());
        let bus = super::components::TracedBus::new();
        let (owner, runner, _) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        assert_eq!(
            observe(&runner, "credential lifetime session").await,
            json!(true)
        );
        assert_eq!(
            observe(&runner, "credential lifetime replace").await,
            json!(true)
        );
        assert_eq!(
            observe(&runner, "credential lifetime counts").await,
            json!([[1, true]])
        );
        assert_eq!(
            observe(&runner, "credential lifetime release").await,
            json!("released")
        );
        assert_eq!(
            observe(&runner, "credential lifetime keep-catalog").await,
            json!("retained")
        );
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        drop(runner);
        drop(owner);
        assert_session_release(&command, catalog, credentials).await;
    });
}

/// Observes final release through a different context, without retaining the original runner.
async fn assert_session_release(
    command: &maestro_extensions::types::CommandHandler<Local>,
    catalog: std::sync::Weak<maestro_catalog::ModelRegistry>,
    credentials: std::sync::Weak<maestro_extensions::AuthStorage>,
) {
    let observer = ExtensionRunner::<Local>::new(
        Rc::default(),
        maestro_extensions::create_extension_runtime::<Local>(),
        super::session::runner_session("/observer").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let answer = command(
        "credential lifetime counts".into(),
        observer.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert_eq!(
        serde_json::from_str::<Value>(&answer).unwrap(),
        json!([[1, true]])
    );
    assert!(catalog.upgrade().is_some(), "retained catalog ended early");
    assert!(
        credentials.upgrade().is_some(),
        "retained credentials ended early"
    );
    command(
        "credential lifetime release-catalog".into(),
        observer.create_command_context(),
    )
    .await
    .unwrap_err();
    command("barrier".into(), observer.create_command_context())
        .await
        .unwrap_err();
    let answer = command(
        "credential lifetime counts".into(),
        observer.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert_eq!(
        serde_json::from_str::<Value>(&answer).unwrap(),
        json!([[1, true], [2, true]])
    );
    assert!(
        catalog.upgrade().is_none(),
        "component API retained the session catalog"
    );
    assert!(
        credentials.upgrade().is_none(),
        "session credentials retained their fallback"
    );
}

#[test]
fn provider_request_auth_uses_shared_credential_owner() {
    if std::env::var_os("MAESTRO_PROVIDER_AUTH_CHILD").is_none() {
        for key in ["", "environment-key"] {
            let output = maestro_test_process::TestProcess::current(
                "provider_request_auth_uses_shared_credential_owner",
            )
            .unwrap()
            .env_clear()
            .env("MAESTRO_PROVIDER_AUTH_CHILD", "1")
            .env(
                "MAESTRO_CONTEXT_COMPONENT",
                std::env::var_os("MAESTRO_CONTEXT_COMPONENT").unwrap(),
            )
            .env("OPENAI_API_KEY", key)
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    super::components::local(async {
        let (session, _) = super::session::runner_session("/provider-auth");
        let native = session.model_registry.clone();
        let mut model = native.get_all().read().unwrap()[0].read().unwrap().clone();
        model.provider = "openai".into();
        model.id = "not-catalog-admitted".into();
        model.headers =
            Some(serde_json::from_value(json!({"x-model":"literal","x-common":"model"})).unwrap());
        assert!(native.find(&model.provider, &model.id).is_none());
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        let input = json!({"op":"register","provider":"openai","model":model,"config":{"apiKey":"configured-key","headers":{"x-common":"provider"},"authHeader":true}});
        provider_call(&runner, input.clone()).await;
        request_key_tiers(&runner, &native, &input).await;
        request_empty_and_fallback(&runner, &native, input).await;
    });
}

/// Checks keys at the consuming guest boundary, including a model absent from the catalog.
async fn request_key_tiers(
    runner: &ExtensionRunner<Local>,
    native: &maestro_catalog::ModelRegistry,
    input: &Value,
) {
    let environment = std::env::var("OPENAI_API_KEY").unwrap();
    let ambient = if environment.is_empty() {
        "configured-key"
    } else {
        &environment
    };
    let auth = native.auth_storage();
    let mut inspect = input.clone();
    inspect["op"] = json!("inspect");
    assert_request_key(&provider_call(runner, inspect.clone()).await, ambient);
    auth.set(
        "openai",
        maestro_extensions::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
            env: None,
            key: "stored-key".into(),
        }),
    );
    assert_request_key(&provider_call(runner, inspect.clone()).await, "stored-key");
    auth.set_runtime_api_key("openai", "runtime-key");
    assert_request_key(&provider_call(runner, inspect.clone()).await, "runtime-key");
    auth.set_runtime_api_key("openai", "");
    assert_request_key(&provider_call(runner, inspect.clone()).await, "stored-key");
    auth.set(
        "openai",
        serde_json::from_value(
            json!({"type":"oauth","refresh":"r","access":"a","expires":4_102_444_800_000_i64}),
        )
        .unwrap(),
    );
    auth.set_runtime_api_key("openai", "runtime-with-oauth");
    let answer = provider_call(runner, inspect.clone()).await;
    assert_request_key(&answer, "runtime-with-oauth");
    assert_eq!(answer["oauth"], json!(true));
    auth.remove("openai");
    auth.remove_runtime_api_key("openai");
    assert_request_key(&provider_call(runner, inspect).await, ambient);
}

/// Empty configured keys count as configured but do not supply request keys or status.
async fn request_empty_and_fallback(
    runner: &ExtensionRunner<Local>,
    native: &maestro_catalog::ModelRegistry,
    mut input: Value,
) {
    input["provider"] = json!("auth-no-environment");
    input["model"]["provider"] = input["provider"].clone();
    input["config"]["apiKey"] = json!("");
    let answer = provider_call(runner, input.clone()).await;
    assert_eq!(answer["configured"], json!(true));
    assert_eq!(answer["status"], json!({"configured":false}));
    assert_eq!(answer["key"], Value::Null);
    assert_eq!(
        answer["auth"],
        json!({"ok":false,"error":"No API key found for \"auth-no-environment\""})
    );
    input["op"] = json!("inspect");
    native
        .auth_storage()
        .set_fallback_resolver(|_| Ok(Some("fallback-key".into())));
    let answer = provider_call(runner, input.clone()).await;
    assert_eq!(answer["credentialKeys"], json!(["fallback-key", null]));
    assert_eq!(answer["key"], Value::Null);
    assert_eq!(
        answer["status"],
        json!({"configured":false,"source":"fallback","label":"custom provider config"})
    );
    assert_eq!(answer["auth"]["ok"], json!(false));
    input["op"] = json!("register");
    input["config"]["authHeader"] = json!(false);
    let answer = provider_call(runner, input).await;
    assert_eq!(
        answer["auth"],
        json!({"ok":true,"apiKey":null,"headers":{"x-model":"literal","x-common":"provider"}})
    );
}

/// Exact headers and selected key must agree, not merely report configured presence.
fn assert_request_key(answer: &Value, key: &str) {
    assert_eq!(answer["key"], json!(key));
    assert_eq!(answer["configured"], json!(true));
    assert_eq!(
        answer["auth"],
        json!({"ok":true,"apiKey":key,"headers":{"x-model":"literal","x-common":"provider","Authorization":format!("Bearer {key}")}})
    );
}

#[test]
fn credentials_retained_owner_forwards_state_errors_and_key_precedence() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let answer = observe(&runner, "credential state").await;
        assert_eq!(
            answer,
            json!({
                "accepted":{"list":["old","new"],"all":{"old":{"type":"api_key","key":"original"},"new":{"type":"api_key","key":"accepted"}},"has":true,"stored":{"type":"api_key","key":"accepted"},"key":"accepted"},
                "retained":["new"], "errors":["write-first","acquire-second","reload-third"],"drained":[],
                "raw":r#"{"old":{"type":"api_key","key":"original"}}"#,
                "recovered":["old"],"logout":[],
                "precedence":{"keys":["fallback","runtime","stored","stored","fallback",null],
                    "empty":{"has":true,"status":{"configured":false,"source":"runtime","label":"--api-key"},"key":"fallback"},"errors":[]}
            })
        );
    });
}

/// Both real native refresh settlements retain callbacks until their selected owner ends.
async fn pending_key_cases(runner: &ExtensionRunner<Local>) {
    for outcome in [Ok(()), Err("literal refresh failure")] {
        pending_key_case(runner, outcome).await;
    }
}

/// One admitted native refresh, with independent guest aliases released while it waits.
async fn pending_key_case(runner: &ExtensionRunner<Local>, outcome: Result<(), &'static str>) {
    let (started, entered) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    maestro_models::register_oauth_provider(std::sync::Arc::new(PendingProvider {
        started: std::sync::Mutex::new(Some(started)),
        released: std::sync::Mutex::new(Some(released)),
    }));
    assert_eq!(
        observe(runner, "credential pending create").await,
        json!("retained")
    );
    assert_eq!(
        observe(runner, "credential pending counts").await,
        json!([])
    );
    let key = observe(runner, "credential pending key");
    let during = async {
        entered.await.unwrap();
        assert_eq!(
            observe(runner, "credential pending release").await,
            json!("released")
        );
        assert_eq!(
            observe(runner, "credential pending counts").await,
            json!([])
        );
        release.send(outcome).unwrap();
    };
    let (answer, ()) = tokio::join!(key, during);
    assert_pending_answer(&answer, outcome.is_ok());
    store_barrier(runner).await;
    assert_eq!(
        observe(runner, "credential pending counts").await,
        json!(["backend", "fallback"])
    );
    assert_eq!(
        observe(runner, "credential pending counts").await,
        json!(["backend", "fallback"])
    );
    maestro_models::unregister_oauth_provider("author-pending");
}

/// Preserves the native refreshed key and its existing failure wrapper.
fn assert_pending_answer(answer: &Value, succeeded: bool) {
    if succeeded {
        assert_eq!(*answer, json!({"key":"refreshed-key","errors":[]}));
    } else {
        assert_eq!(answer["key"], Value::Null);
        assert_eq!(answer["errors"].as_array().unwrap().len(), 1);
        assert_eq!(
            answer["errors"][0]["message"],
            json!("Failed to refresh OAuth token for author-pending")
        );
    }
}
