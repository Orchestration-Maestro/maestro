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

/// The probe protocol whose provider this test registers through an extension route.
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

/// Several sessions live in one process: every session reloads, and each one keeps its own
/// registered provider while the others reload.
#[test]
fn reloads_across_sessions_keep_each_others_registered_providers() {
    on_task_set(|_| async {
        let left = harness(HarnessOptions::default()).await;
        let middle = harness(HarnessOptions::default()).await;
        let right = harness(HarnessOptions::default()).await;
        drop(script_responses(&left, vec![answer("left", None, None)]));
        drop(script_responses(
            &middle,
            vec![answer("middle", None, None)],
        ));
        drop(script_responses(&right, vec![answer("right", None, None)]));

        left.session.reload().await.unwrap();
        middle.session.reload().await.unwrap();
        right.session.reload().await.unwrap();

        for (session, text) in [(&left, "left"), (&middle, "middle"), (&right, "right")] {
            session
                .session
                .prompt("question", PromptOptions::default())
                .await
                .unwrap();
            let answered = history_view(session).pop().unwrap();
            assert_eq!(answered, json!({"role": "assistant", "text": text}));
        }
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
fn reload_keeps_extension_registered_provider() {
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
        assert!(maestro_models::get_api_provider(PROBE_API).is_some());
        h.registry.refresh().unwrap();
        assert!(maestro_models::get_api_provider(PROBE_API).is_some());
    });
}
