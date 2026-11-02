//! Native interaction supplied to an OAuth implementation in the compiled component.
use maestro_models::{
    BoxFuture, OAuthAuthInfo, OAuthError, OAuthLoginCallbacks, OAuthPrompt, OAuthProviderHandle,
    OAuthSelectPrompt,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Original native callbacks, including a same-instance synchronous reentry target.
struct Callbacks {
    /// Independently selected callback presence or failure.
    mode: &'static str,
    /// Calls observed on the original native owner, in delivery order.
    events: Arc<Mutex<Vec<Value>>>,
    /// The very guest implementation invoking this interaction.
    provider: OAuthProviderHandle,
}
impl Callbacks {
    /// Records the actual input before returning its literal success or failure.
    fn answer<T>(&self, phase: &str, input: Value, value: T) -> Result<T, OAuthError> {
        self.events
            .lock()
            .unwrap()
            .push(Value::Array(vec![json!(phase), input]));
        if self.mode.strip_suffix("-error") == Some(phase) {
            Err(
                maestro_models::DiagnosticErrorInfo::error(format!("native {phase} failure"))
                    .into(),
            )
        } else {
            Ok(value)
        }
    }
}
impl OAuthLoginCallbacks for Callbacks {
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        let credentials =
            serde_json::from_value(json!({"access":"nested","refresh":"r","expires":0})).unwrap();
        assert_eq!(
            self.provider.get_api_key(&credentials)?.as_ref(),
            "guest-computed:nested:r"
        );
        self.answer("auth", json!(info), ())
    }
    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        Box::pin(std::future::ready(self.answer(
            "prompt",
            json!(prompt),
            " Native answer ".into(),
        )))
    }
    fn callback_presence(&self) -> (bool, bool, bool) {
        match self.mode {
            "absent" => (false, false, false),
            "progress-only" => (true, false, false),
            "manual-only" => (false, true, false),
            "select-only" => (false, false, true),
            _ => (true, true, true),
        }
    }
    fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
        self.answer("progress", json!(message), ())
    }
    fn on_manual_code_input(&self) -> Option<BoxFuture<Result<String, OAuthError>>> {
        self.callback_presence().1.then(|| {
            Box::pin(std::future::ready(self.answer(
                "manual",
                Value::Null,
                "native-code".into(),
            ))) as _
        })
    }
    fn on_select(
        &self,
        prompt: OAuthSelectPrompt,
    ) -> Option<BoxFuture<Result<Option<String>, OAuthError>>> {
        let selected = (self.mode == "chosen").then(|| "repeated".into());
        self.callback_presence().2.then(|| {
            Box::pin(std::future::ready(self.answer(
                "select",
                json!(prompt),
                selected,
            ))) as _
        })
    }
}

#[test]
fn oauth_guest_login_preserves_native_optional_callbacks_errors_and_reentry() {
    super::super::components::local(async {
        let bus = super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.controller.for_extension()).await;
        assert_eq!(super::call(&runner, "reverse install").await, true);
        let provider = maestro_models::get_oauth_provider("guest-reverse").unwrap();
        for mode in [
            "absent",
            "progress-only",
            "manual-only",
            "select-only",
            "all",
            "chosen",
        ] {
            let callbacks = Arc::new(Callbacks {
                mode,
                events: Arc::default(),
                provider: provider.clone(),
            });
            let result = provider.login(callbacks.clone(), None).await.unwrap();
            let (progress, manual, select) = callbacks.callback_presence();
            assert_eq!(result.access, " Native answer ");
            assert_eq!(result.extra["presence"], json!([progress, manual, select]));
            assert_eq!(
                result.extra["manual"],
                json!(manual.then_some("native-code"))
            );
            assert_eq!(
                result.extra["selection"],
                json!((mode == "chosen").then_some("repeated"))
            );
            assert_eq!(
                *callbacks.events.lock().unwrap(),
                expected_events(callbacks.callback_presence())
            );
        }
        errors(&provider).await;
        assert_eq!(super::call(&runner, "reverse release").await, true);
        maestro_models::unregister_oauth_provider("guest-reverse");
    });
}

/// Literal authored inputs, with no callbacks synthesized for absent options.
fn expected_events((progress, manual, select): (bool, bool, bool)) -> Vec<Value> {
    let mut events = vec![
        json!(["auth", {"url":"https://guest.invalid/ literal", "instructions":" Guest instructions "}]),
        json!(["prompt", {"message":" Guest prompt ", "placeholder":"", "allowEmpty":false}]),
    ];
    if progress {
        events.push(json!(["progress", " Guest progress "]));
    }
    if manual {
        events.push(json!(["manual", null]));
    }
    if select {
        events.push(json!(["select", {"message":" Guest selection ","options":[{"id":"repeated","label":" First "},{"id":"repeated","label":" Second "}]}]));
    }
    events
}

/// Every native failure crosses the guest once and stops later callbacks.
async fn errors(provider: &OAuthProviderHandle) {
    for (index, mode) in [
        "auth-error",
        "prompt-error",
        "progress-error",
        "manual-error",
        "select-error",
    ]
    .into_iter()
    .enumerate()
    {
        let callbacks = Arc::new(Callbacks {
            mode,
            events: Arc::default(),
            provider: provider.clone(),
        });
        assert_eq!(
            provider
                .login(callbacks.clone(), None)
                .await
                .unwrap_err()
                .to_string(),
            format!("native {} failure", mode.trim_end_matches("-error"))
        );
        assert_eq!(callbacks.events.lock().unwrap().len(), index + 1);
    }
}

mod lifetime;
