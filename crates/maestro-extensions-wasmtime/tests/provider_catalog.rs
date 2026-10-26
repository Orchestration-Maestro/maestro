//! Provider operations exercised through the production component loader.
#![cfg(test)]
use maestro_extensions::types::ExtensionHandle;
use maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};

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

/// Keeps the loaded owner alive while the original catalog is observed.
async fn loaded() -> (ExtensionHandle<Local>, ExtensionRunner<Local>) {
    let bus = super::components::TracedBus::new();
    let (owner, runner, _) =
        super::retained_components::loaded_on(bus.controller.for_extension()).await;
    (owner, runner)
}

#[test]
fn provider_author_component_uses_native_catalog_without_adapter_changes() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let answer = observe(&runner, "provider catalog").await;
        assert_eq!(
            answer,
            json!({
                "label":" Literal provider ", "key":"literal-key",
                "configured":true, "status":{"configured":true,"source":"models_json_key"},
                "oauth":false, "before":["first"], "replaced":["second"],
                "refreshed":["second"], "removed":true, "error":null,
                "auth":{"ok":true,"apiKey":"literal-key","headers":{"x-test":"literal","Authorization":"Bearer literal-key"}}
            })
        );
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
    released: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
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
            released.await.unwrap();
            credentials.access = "refreshed-key".into();
            credentials.expires = 4_102_444_800_000.0;
            Ok(credentials)
        })
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a maestro_models::OAuthCredentials,
    ) -> Result<&'a str, maestro_models::OAuthError> {
        Ok(&credentials.access)
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
            release.send(()).unwrap();
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
