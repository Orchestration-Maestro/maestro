//! Interaction forms observed through the compiled guest and native OAuth interface.
use maestro_models::{
    BoxFuture, Fetch, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthProviderInterface,
};
use serde_json::json;

/// Native probe invokes the callbacks using their actual synchronous and optional forms.
struct Provider;
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &'static str {
        "component-login"
    }
    fn name(&self) -> &'static str {
        "Controlled login"
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async move {
            callbacks.on_auth(maestro_models::OAuthAuthInfo {
                url: "https://authorization.invalid/ literal".into(),
                instructions: Some(" Literal instructions ".into()),
            })?;
            let prompt = callbacks
                .on_prompt(maestro_models::OAuthPrompt {
                    message: " Literal prompt ".into(),
                    placeholder: Some(String::new()),
                    allow_empty: Some(false),
                })
                .await?;
            callbacks.on_progress(" Literal progress ")?;
            let manual = if callbacks.callback_presence().1 {
                Some(
                    callbacks
                        .on_manual_code_input()
                        .expect("declared manual callback must exist")
                        .await?,
                )
            } else {
                None
            };
            let select = match callbacks.on_select(maestro_models::OAuthSelectPrompt {
                message: " Literal selection ".into(),
                options: vec![
                    maestro_models::OAuthSelectOption {
                        id: "repeated".into(),
                        label: " First ".into(),
                    },
                    maestro_models::OAuthSelectOption {
                        id: "repeated".into(),
                        label: " Second ".into(),
                    },
                ],
            }) {
                Some(value) => Some(value.await?),
                None => None,
            };
            serde_json::from_value(json!({"refresh":"controlled-refresh", "access":prompt, "expires":42.5, "manual":manual, "selectionPresent":select.is_some(), "selection":select.flatten(), "aborted":callbacks.signal().map(maestro_models::Cancellation::is_aborted), "open":[null,false,-0.0]})).map_err(|error| maestro_models::DiagnosticErrorInfo::error(error.to_string()).into())
        })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::ready(Ok(credentials)))
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        Ok(std::borrow::Cow::Borrowed(&credentials.access))
    }
}

#[test]
fn oauth_login_preserves_callback_forms_and_errors() {
    super::super::components::local(async {
        maestro_models::register_oauth_provider(std::sync::Arc::new(Provider));
        let bus = super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let all = super::call(&runner, "login all").await;
        assert_eq!(
            all["credentials"],
            json!({"refresh":"controlled-refresh","access":" Literal answer ","expires":42.5,"manual":"manual-code","selectionPresent":true,"selection":null,"aborted":null,"open":[null,false,-0.0]})
        );
        assert_eq!(
            all["events"],
            json!([
                ["auth",{"url":"https://authorization.invalid/ literal","instructions":" Literal instructions "}],
                ["prompt",{"message":" Literal prompt ","placeholder":"","allowEmpty":false}],
                ["progress"," Literal progress "], ["manual"],
                ["select",{"message":" Literal selection ","options":[{"id":"repeated","label":" First "},{"id":"repeated","label":" Second "}]}]
            ])
        );
        let absent = super::call(&runner, "login absent").await;
        assert_eq!(absent["credentials"]["selectionPresent"], false);
        assert_eq!(absent["events"].as_array().unwrap().len(), 2);
        for phase in ["auth", "prompt", "progress", "manual", "select"] {
            let answer = super::call(&runner, &format!("login {phase}-error")).await;
            assert_eq!(answer["error"], format!("literal {phase} failure"));
        }
        maestro_models::unregister_oauth_provider("component-login");
    });
}

/// Retains interaction separately from the login future that supplied it.
struct Retaining(std::sync::Arc<std::sync::Mutex<Option<OAuthCallbacks>>>);
impl OAuthProviderInterface for Retaining {
    fn id(&self) -> &'static str {
        "component-login"
    }
    fn name(&self) -> &'static str {
        "Retaining login"
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        *self.0.lock().unwrap() = Some(callbacks.clone());
        Provider.login(callbacks, fetch)
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::ready(Ok(credentials)))
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        Ok(std::borrow::Cow::Borrowed(&credentials.access))
    }
}

#[test]
fn oauth_login_callbacks_outlive_the_initiating_future() {
    super::super::components::local(async {
        let retained = std::sync::Arc::new(std::sync::Mutex::new(None));
        maestro_models::register_oauth_provider(std::sync::Arc::new(Retaining(retained.clone())));
        let bus = super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        assert!(
            super::call(&runner, "login all")
                .await
                .get("credentials")
                .is_some()
        );
        maestro_models::unregister_oauth_provider("component-login");
        let callbacks = retained.lock().unwrap().take().unwrap();
        assert_eq!(
            callbacks
                .on_prompt(maestro_models::OAuthPrompt {
                    message: "retained prompt".into(),
                    placeholder: None,
                    allow_empty: None
                })
                .await
                .unwrap(),
            " Literal answer "
        );
        assert_eq!(super::call(&runner, "login counts").await, 0);
        drop(callbacks);
        assert_eq!(super::call(&runner, "login counts").await, 1);
    });
}

#[test]
fn oauth_login_retains_live_cancellation_with_the_callbacks() {
    super::super::components::local(async {
        let retained = std::sync::Arc::new(std::sync::Mutex::new(None));
        maestro_models::register_oauth_provider(std::sync::Arc::new(Retaining(retained.clone())));
        let bus = super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let signal = maestro_models::Cancellation::new();
        super::super::abort_observers::bind(&runner, &signal);
        assert!(
            super::call(&runner, "login signal")
                .await
                .get("credentials")
                .is_some()
        );
        let callbacks = retained.lock().unwrap().take().unwrap();
        let original = callbacks.signal().expect("caller signal was omitted");
        assert!(!original.is_aborted());
        signal.abort();
        original.cancelled().await;
        assert!(original.is_aborted());
        maestro_models::unregister_oauth_provider("component-login");
        drop(callbacks);
        assert_eq!(super::call(&runner, "login counts").await, 1);
        maestro_models::register_oauth_provider(std::sync::Arc::new(Provider));
        assert!(
            super::call(&runner, "login pre-aborted").await["credentials"]["aborted"]
                .as_bool()
                .unwrap()
        );
        maestro_models::unregister_oauth_provider("component-login");
    });
}

#[test]
fn oauth_login_pending_prompt_keeps_the_host_available() {
    super::super::components::local(async {
        use std::{cell::RefCell, rc::Rc};
        maestro_models::register_oauth_provider(std::sync::Arc::new(Provider));
        let bus = super::super::components::TracedBus::new();
        let (send, ready) = tokio::sync::oneshot::channel();
        let send = Rc::new(RefCell::new(Some(send)));
        let _subscription = bus.controller.on(
            "oauth-prompt-ready",
            Rc::new(move |_, _| {
                send.borrow_mut().take().unwrap().send(()).unwrap();
                Ok(None)
            }),
        );
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let mut login = std::pin::pin!(super::call(&runner, "login pending"));
        tokio::select! {
            _ = &mut login => panic!("login completed before its awaited prompt was released"),
            ready = ready => ready.unwrap(),
        }
        maestro_models::unregister_oauth_provider("component-login");
        runner.invalidate(Some("initiating login context retired".into()));
        assert_eq!(super::call(&runner, "login counts").await, 0);
        assert_eq!(super::call(&runner, "login resume").await, "resumed");
        assert_eq!(login.await["credentials"]["access"], " Literal answer ");
        assert_eq!(super::call(&runner, "login counts").await, 1);
    });
}

#[test]
fn oauth_credential_login_persists_through_the_native_owner() {
    super::super::components::local(async {
        maestro_models::register_oauth_provider(std::sync::Arc::new(Provider));
        let bus = super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let result = super::call(&runner, "storage all").await;
        let expected = json!({"type":"oauth","refresh":"controlled-refresh","access":" Literal answer ","expires":42.5,"manual":"manual-code","selectionPresent":true,"selection":null,"aborted":null,"open":[null,false,-0.0]});
        assert_eq!(result["storage"]["result"], json!({"ok":true}));
        assert_eq!(result["storage"]["accepted"], expected);
        assert_eq!(result["storage"]["raw"]["component-login"], expected);
        assert_eq!(result["storage"]["reloaded"], expected);
        assert_eq!(result["storage"]["raw"]["neighbor"], json!([null, false]));
        assert_eq!(result["storage"]["errors"], json!([]));
        assert_eq!(result["events"].as_array().unwrap().len(), 5);
        let failed = super::call(&runner, "storage prompt-error").await;
        assert_eq!(
            failed["storage"]["result"]["error"],
            "literal prompt failure"
        );
        assert_eq!(failed["storage"]["accepted"], json!(null));
        assert_eq!(failed["storage"]["raw"], json!({"neighbor":[null,false]}));
        let unknown = super::call(&runner, "storage unknown").await;
        assert_eq!(
            unknown["storage"]["result"]["error"],
            "Unknown OAuth provider: missing-component-login"
        );
        assert_eq!(unknown["events"], json!([]));
        let write = super::call(&runner, "storage write-failure").await;
        assert_eq!(write["storage"]["result"], json!({"ok":true}));
        assert_eq!(write["storage"]["accepted"], expected);
        assert_eq!(write["storage"]["raw"], json!({"neighbor":[null,false]}));
        assert_eq!(write["storage"]["errors"], json!(["write-login"]));
        assert_eq!(write["storage"]["reloaded"], json!(null));
        maestro_models::unregister_oauth_provider("component-login");
    });
}

#[test]
fn oauth_login_provider_branches_on_manual_callback_presence() {
    super::super::components::local(async {
        maestro_models::register_oauth_provider(std::sync::Arc::new(Provider));
        let bus = super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let supplied = super::call(&runner, "login all").await;
        assert_eq!(supplied["credentials"]["manual"], "manual-code");
        assert_eq!(supplied["credentials"]["selectionPresent"], true);
        assert_eq!(supplied["credentials"]["selection"], json!(null));
        let absent = super::call(&runner, "login absent").await;
        assert_eq!(absent["credentials"]["manual"], json!(null));
        assert_eq!(absent["credentials"]["selectionPresent"], false);
        assert_eq!(absent["events"].as_array().unwrap().len(), 2);
        maestro_models::unregister_oauth_provider("component-login");
    });
}
