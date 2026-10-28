//! Decisions before session selection effects through the shared application interface.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::session_harness::{HarnessOptions, harness, model_view, on_task_set};
use maestro_app::agent_session::CycleDirection;
use maestro_models::ModelThinkingLevel;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_settings::{DecisionRequest, SettingsDecider};
use std::sync::{Arc, Mutex};

/// Refuses explicit operations while recording their requested, unclamped values.
#[derive(Default)]
struct Refuse(Mutex<Vec<String>>);
impl SettingsDecider for Refuse {
    fn decide(&self, request: &DecisionRequest<'_>) -> Result<(), String> {
        self.0.lock().unwrap().push(format!("{request:?}"));
        Err("configuration release refuses selection".into())
    }
}

/// An owned history snapshot that does not retain the session-manager borrow.
fn history(session: &maestro_app::AgentSession) -> Vec<serde_json::Value> {
    session
        .session_manager()
        .borrow()
        .get_entries()
        .into_iter()
        .cloned()
        .collect()
}

/// Checks refusal without requiring successful values to be debug-printable.
fn assert_refused<T>(result: Result<T, maestro_models::DiagnosticErrorInfo>) {
    assert_eq!(
        result.err().unwrap().message,
        "configuration release refuses selection"
    );
}

/// Setters and cycles refuse before state, history, preferences or notifications change.
#[test]
fn refusing_decider_preserves_session_selection() {
    on_task_set(|_| async {
        for reasoning in [false, true] {
            let h = harness(HarnessOptions {
                models: vec![("plain", reasoning), ("think", true)],
                ..HarnessOptions::default()
            })
            .await;
            let settings = h.session.settings_manager();
            let decider = Arc::new(Refuse::default());
            settings
                .borrow_mut()
                .register_decider(decider.clone())
                .unwrap();
            let model = model_view(&h.session.model());
            let level = h.session.thinking_level();
            let entries = history(&h.session);
            let preferences = settings.borrow().get_global_settings();
            let events = h.events.borrow().clone();
            assert_refused(h.session.set_model(h.model("think")).await);
            assert_refused(h.session.cycle_model(CycleDirection::Forward).await);
            assert_refused(h.session.set_thinking_level(ModelThinkingLevel::High));
            if reasoning {
                assert_refused(h.session.cycle_thinking_level());
            }
            assert_eq!(model_view(&h.session.model()), model);
            assert_eq!(h.session.thinking_level(), level);
            assert_eq!(history(&h.session), entries);
            assert_eq!(settings.borrow().get_global_settings(), preferences);
            assert_eq!(*h.events.borrow(), events);
            let flush = settings.borrow().flush();
            flush.await;
            assert!(!std::path::Path::new(&h.cwd).exists());
            assert!(
                decider
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|request| request.contains("ThinkingLevel(\"high\")"))
            );
        }
    });
}

/// Protocol model and thinking failures retain the caller's correlation and session bytes.
#[test]
fn rpc_selection_refusals_preserve_history_and_return_the_message() {
    use crate::support::rpc_connection::connect;
    use crate::support::runtime_fixtures::{RuntimeOptions, runtime_with};
    use serde_json::json;
    use std::rc::Rc;
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_with(
            "decisions",
            RuntimeOptions {
                models: vec![("faux-1", true), ("faux-2", true)],
                ..RuntimeOptions::default()
            },
        )
        .await;
        let runtime = Rc::new(runtime);
        let session = runtime.session();
        session
            .settings_manager()
            .borrow_mut()
            .register_decider(Arc::new(Refuse::default()))
            .unwrap();
        session
            .session_manager()
            .borrow_mut()
            .append_message(crate::support::session_harness::assistant(
                "seed",
                0,
                &json!({}),
            ))
            .unwrap();
        let path = session.session_file().unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut client = connect(Rc::clone(&runtime));
        for command in [
            json!({"type":"set_model","provider":"faux","modelId":"faux-2"}),
            json!({"type":"cycle_model"}),
            json!({"type":"set_thinking_level","level":"high"}),
            json!({"type":"cycle_thinking_level"}),
        ] {
            let reply = client.ask(&command.to_string()).await;
            assert_eq!(reply["success"], false);
            assert_eq!(reply["error"], "configuration release refuses selection");
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        client.end(None);
        client.outcome().await.unwrap();
    });
}

/// Extension model refusal returns its unsuccessful result without changing the session.
#[test]
fn extension_selection_refusals_return_unsuccessful_without_effects() {
    use std::{cell::RefCell, rc::Rc};
    on_task_set(|_| async {
        let slot = Rc::new(RefCell::new(None));
        let kept = Rc::clone(&slot);
        let h = harness(HarnessOptions {
            factories: vec![Rc::new(move |api| {
                *kept.borrow_mut() = Some(api);
                Box::pin(std::future::ready(Ok(())))
            })],
            ..HarnessOptions::default()
        })
        .await;
        let api = slot.borrow_mut().take().unwrap();
        h.session
            .settings_manager()
            .borrow_mut()
            .register_decider(Arc::new(Refuse::default()))
            .unwrap();
        let model = model_view(&h.session.model());
        let entries = history(&h.session);
        assert!(!api.set_model(h.model("think")).unwrap().await.unwrap());
        assert!(api.set_thinking_level(ModelThinkingLevel::High).is_err());
        assert_eq!(model_view(&h.session.model()), model);
        assert_eq!(history(&h.session), entries);
        assert!(h.events.borrow().is_empty());
        assert!(
            h.session
                .settings_manager()
                .borrow_mut()
                .drain_errors()
                .iter()
                .any(|error| error.error.to_string() == "configuration release refuses selection")
        );
    });
}

/// An allowing publisher recording the requested thinking level of a model switch.
#[derive(Default)]
struct Allow(Mutex<Vec<String>>);
impl SettingsDecider for Allow {
    fn decide(&self, request: &DecisionRequest<'_>) -> Result<(), String> {
        self.0.lock().unwrap().push(format!("{request:?}"));
        Ok(())
    }
}

/// The active level clamps while a locked requested default stays untouched and silent.
#[test]
fn locked_thinking_default_survives_model_capability_clamp() {
    use maestro_settings::{BaseSettingsLayer, SettingsScope};
    use serde_json::json;
    on_task_set(|_| async {
        for locked in [true, false] {
            let h = harness(HarnessOptions {
                settings: json!({"defaultThinkingLevel":"xhigh"}),
                ..HarnessOptions::default()
            })
            .await;
            let settings = h.session.settings_manager();
            if locked {
                settings.borrow_mut().publish_base_layer(
                    SettingsScope::Global,
                    Some(BaseSettingsLayer {
                        settings: serde_json::from_value(json!({"defaultThinkingLevel":"xhigh"}))
                            .unwrap(),
                        locked: ["defaultThinkingLevel".into()].into(),
                        source: "configuration-release".into(),
                        recorded_at: "2026-10-27".into(),
                    }),
                );
            }
            let decider = Arc::new(Allow::default());
            settings
                .borrow_mut()
                .register_decider(decider.clone())
                .unwrap();
            h.session.set_model(h.model("think")).await.unwrap();
            assert_eq!(h.session.thinking_level(), ModelThinkingLevel::High);
            assert_eq!(
                settings
                    .borrow()
                    .get_default_thinking_level()
                    .unwrap()
                    .as_str(),
                if locked { "xhigh" } else { "high" }
            );
            let raw = settings.borrow().get_global_settings();
            assert_eq!(
                raw.0["defaultThinkingLevel"],
                if locked { "xhigh" } else { "high" }
            );
            assert!(settings.borrow_mut().drain_errors().is_empty());
            assert_eq!(
                crate::session_model_selection::selection_entries(&h),
                vec![
                    json!({"type":"model_change","provider":"faux","modelId":"think"}),
                    json!({"type":"thinking_level_change","thinkingLevel":"high"}),
                ]
            );
            assert!(decider.0.lock().unwrap()[0].contains("thinking_level: \"xhigh\""));
        }
    });
}

/// A host callback verifies it can acquire the target model exclusively.
struct ModelLockProbe(Arc<std::sync::RwLock<maestro_models::Model>>);
impl SettingsDecider for ModelLockProbe {
    fn decide(&self, request: &DecisionRequest<'_>) -> Result<(), String> {
        if matches!(request, DecisionRequest::Model { .. }) {
            assert!(
                self.0.try_write().is_ok(),
                "model lock held across host callback"
            );
        }
        Ok(())
    }
}

/// Admission callbacks receive a snapshot, not a live model guard.
#[test]
fn model_decider_can_acquire_target_write_lock() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let target = h.model("think");
        h.session
            .settings_manager()
            .borrow_mut()
            .register_decider(Arc::new(ModelLockProbe(Arc::clone(&target))))
            .unwrap();
        h.session.set_model(target).await.unwrap();
    });
}

/// Authored diagnostic names never turn a post-selection failure into a refusal.
#[test]
fn extension_selection_propagates_listener_failure_named_configuration_refusal() {
    use maestro_extensions::ModelSelectEvent;
    use maestro_models::DiagnosticErrorInfo;
    use std::{cell::RefCell, rc::Rc};
    on_task_set(|_| async {
        let slot = Rc::new(RefCell::new(None));
        let kept = Rc::clone(&slot);
        let h = harness(HarnessOptions {
            factories: vec![Rc::new(move |api| {
                api.on::<ModelSelectEvent, ()>(
                    "model_select".into(),
                    Rc::new(|_, _| {
                        Box::pin(std::future::ready(Err("selection handler failed".into())))
                    }),
                )
                .unwrap();
                *kept.borrow_mut() = Some(api);
                Box::pin(std::future::ready(Ok(())))
            })],
            ..HarnessOptions::default()
        })
        .await;
        let _unsubscribe = h.runner().on_error(Rc::new(|_| {
            Err(Box::new(DiagnosticErrorInfo::new(
                Some("ConfigurationRefusal".into()),
                "listener failed",
            )))
        }));
        let before = history(&h.session);
        let api = slot.borrow_mut().take().unwrap();
        let result = api.set_model(h.model("think")).unwrap().await;
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "listener failed");
        assert_eq!(
            model_view(&h.session.model()),
            model_view(&h.model("think"))
        );
        assert_ne!(history(&h.session), before);
    });
}
