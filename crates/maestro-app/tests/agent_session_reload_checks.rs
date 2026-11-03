//! A reload applies the saved preferences that the live agent also holds, and leaves the
//! providers of the sessions sharing its process registered.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, answer, harness, history_view, on_task_set, script_responses,
};
use maestro_agent::QueueMode;
use maestro_app::PromptOptions;
use maestro_catalog::{ProviderCallbacks, ProviderConfigInput};
use maestro_extensions::{ExtensionFactory, Local};
use maestro_models::{
    AssistantMessageEventStream, DiagnosticErrorInfo, Invoke, ProviderCall, ProviderExecution,
    SharedHandle,
};
use maestro_settings::MessageDeliveryMode;
use serde_json::json;
use std::{rc::Rc, sync::RwLock};

/// The probe protocol the reload-owner test registers through an extension route.
const PROBE_API: &str = "probe-stream";

#[test]
fn reload_applies_saved_queue_modes_to_the_live_agent() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        assert_eq!(h.session.steering_mode(), QueueMode::OneAtATime);
        assert_eq!(h.session.follow_up_mode(), QueueMode::OneAtATime);
        {
            let mut settings = h.session.settings_manager().borrow_mut();
            settings.set_steering_mode(MessageDeliveryMode::All);
            settings.set_follow_up_mode(MessageDeliveryMode::All);
        }
        h.session.reload().await.unwrap();
        assert_eq!(h.session.steering_mode(), QueueMode::All);
        assert_eq!(h.session.follow_up_mode(), QueueMode::All);
    });
}

#[test]
fn reload_keeps_another_sessions_registered_provider() {
    on_task_set(|_| async {
        let elsewhere = harness(HarnessOptions::default()).await;
        drop(script_responses(
            &elsewhere,
            vec![answer("elsewhere", None, None)],
        ));
        let h = harness(HarnessOptions::default()).await;
        drop(script_responses(&h, vec![answer("mine", None, None)]));

        elsewhere.session.reload().await.unwrap();
        h.session
            .prompt("question", PromptOptions::default())
            .await
            .unwrap();

        let answered = history_view(&h).pop().unwrap();
        assert_eq!(answered, json!({"role": "assistant", "text": "mine"}));
    });
}

/// A probe callback whose invocation only reports the probe's diagnostic.
struct Probe;
impl<E: ProviderExecution, O> Invoke<E, ProviderCall<E, O>> for Probe {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, _: ProviderCall<E, O>) -> Self::Return {
        Err(DiagnosticErrorInfo::error("probe"))
    }
}

#[test]
fn reload_retires_a_provider_the_reload_window_does_not_renew() {
    on_task_set(|_| async {
        let register: ExtensionFactory<Local> = Rc::new(|api| {
            Box::pin(async move {
                let config = ProviderConfigInput {
                    base_url: Some("https://probe.invalid/v1".into()),
                    api: Some(PROBE_API.into()),
                    callbacks: ProviderCallbacks {
                        stream_simple: Some(SharedHandle::new(Probe)),
                        oauth: None,
                    },
                    ..ProviderConfigInput::default()
                };
                api.register_provider("probe".into(), SharedHandle::new(RwLock::new(config)))?;
                Ok(())
            })
        });
        let h = harness(HarnessOptions {
            factories: vec![register],
            ..HarnessOptions::default()
        })
        .await;
        assert!(maestro_models::get_api_provider(PROBE_API).is_some());
        h.session.reload().await.unwrap();
        assert!(maestro_models::get_api_provider(PROBE_API).is_none());
        h.registry.refresh().unwrap();
        assert!(maestro_models::get_api_provider(PROBE_API).is_none());
    });
}
