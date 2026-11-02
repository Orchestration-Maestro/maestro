//! Model, thinking-level and queue-mode selection compared with the recorded cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, case, harness, model_view, on_task_set,
};
use maestro_agent::QueueMode;
use maestro_app::ModelCycleResult;
use maestro_app::agent_session::CycleDirection;
use maestro_extensions::{ExtensionFactory, ModelSelectEvent, ThinkingLevelSelectEvent};
use maestro_models::{DiagnosticErrorInfo, Local, ModelThinkingLevel};
use maestro_session::session_manager::ReadonlySessionManager;
use serde::Serialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Extension events in the order the handlers saw them.
type Log = Rc<RefCell<Vec<Value>>>;

/// An extension recording model and thinking selection events.
fn recorder(log: &Log) -> ExtensionFactory<Local> {
    let log = Rc::clone(log);
    Rc::new(move |api| {
        let models = Rc::clone(&log);
        api.on::<ModelSelectEvent, ()>(
            "model_select".into(),
            Rc::new(move |event, _| {
                models.borrow_mut().push(json!({
                    "type": "model_select",
                    "model": model_view(&event.model),
                    "previousModel": event.previous_model.present().map(model_view),
                    "source": event.source,
                }));
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
        let levels = Rc::clone(&log);
        api.on::<ThinkingLevelSelectEvent, ()>(
            "thinking_level_select".into(),
            Rc::new(move |event, _| {
                levels.borrow_mut().push(json!({
                    "type": "thinking_level_select",
                    "level": event.level,
                    "previousLevel": event.previous_level,
                }));
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

/// A labelled step result: its value, or the thrown message.
fn step(label: &str, result: Result<Value, DiagnosticErrorInfo>) -> Value {
    let result = match result {
        Ok(value) => json!({"ok": value}),
        Err(error) => json!({"error": error.message}),
    };
    json!({"label": label, "result": result})
}

/// The recorded form of an operation without a result.
const UNDEFINED: fn(()) -> Value = |()| json!({"undefined": true});

/// The recorded form of a serializable result.
fn value(result: impl Serialize) -> Value {
    serde_json::to_value(result).unwrap()
}

/// The recorded form of a cycling result.
fn cycle_view(result: Option<ModelCycleResult>) -> Value {
    result.map_or(Value::Null, |result| {
        json!({
            "model": model_view(&result.model),
            "thinkingLevel": result.thinking_level,
            "isScoped": result.is_scoped,
        })
    })
}

/// The recorded form of a thinking cycle, absent when the model cannot reason.
fn level_cycle_view(result: Option<ModelThinkingLevel>) -> Value {
    result.map_or_else(|| UNDEFINED(()), value)
}

/// Runs a recorded selection case: `steps` drives the session, then the final model,
/// level, recorded entries, saved settings and both event streams are compared.
fn selection_case(name: &str, steps: impl AsyncFnOnce(&Harness) -> Vec<Value>) {
    let expected = case("model_selection", name);
    let log = Log::default();
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: expected["settings"].clone(),
            factories: vec![recorder(&log)],
            ..HarnessOptions::default()
        })
        .await;
        let results = steps(&h).await;
        let settings = h.session.settings_manager().borrow();
        let observed = json!({
            "name": name,
            "settings": expected["settings"],
            "results": results,
            "state": {
                "model": model_view(&h.session.model()),
                "thinkingLevel": h.session.thinking_level(),
            },
            "entries": selection_entries(&h),
            "settingsAfter": {
                "defaultProvider": settings.get_default_provider(),
                "defaultModel": settings.get_default_model(),
                "defaultThinkingLevel": settings.get_default_thinking_level().map(Value::from),
            },
            "sessionEvents": *h.events.borrow(),
            "extensionEvents": *log.borrow(),
        });
        assert_eq!(observed, expected);
    });
}

/// The model and thinking entries the session recorded, in order.
pub(crate) fn selection_entries(h: &Harness) -> Vec<Value> {
    let manager = h.session.session_manager().borrow();
    manager
        .get_entries()
        .into_iter()
        .filter_map(|entry| match entry["type"].as_str() {
            Some("model_change") => Some(json!({
                "type": "model_change",
                "provider": entry["provider"],
                "modelId": entry["modelId"],
            })),
            Some("thinking_level_change") => Some(json!({
                "type": "thinking_level_change",
                "thinkingLevel": entry["thinkingLevel"],
            })),
            _ => None,
        })
        .collect()
}

/// The recorded spelling of a queue mode.
fn queue_mode(mode: QueueMode) -> &'static str {
    match mode {
        QueueMode::All => "all",
        QueueMode::OneAtATime => "one-at-a-time",
    }
}

/// The recorded form of setting `id` as the model.
async fn set_model(h: &Harness, label: &str, id: &str) -> Value {
    step(label, h.session.set_model(h.model(id)).await.map(UNDEFINED))
}

/// The recorded form of setting the thinking level named `level`.
fn set_level(h: &Harness, label: &str, level: &str) -> Value {
    let level = serde_json::from_value(json!(level)).unwrap();
    step(label, h.session.set_thinking_level(level).map(UNDEFINED))
}

/// The recorded form of the available levels.
fn levels(h: &Harness) -> Value {
    step(
        "levels",
        Ok(value(h.session.get_available_thinking_levels())),
    )
}

/// The recorded form of cycling models.
async fn cycle(h: &Harness, label: &str, direction: CycleDirection) -> Value {
    step(
        label,
        h.session.cycle_model(direction).await.map(cycle_view),
    )
}

#[test]
fn switching_from_plain_model_starts_at_medium() {
    selection_case("switch from a non-reasoning model uses medium", async |h| {
        vec![set_model(h, "setModel think", "think").await]
    });
}

#[test]
fn switching_from_plain_model_uses_saved_default() {
    selection_case(
        "switch from a non-reasoning model uses the saved default",
        async |h| vec![set_model(h, "setModel think", "think").await],
    );
}

#[test]
fn reselecting_same_model_records_without_select_event() {
    selection_case(
        "reselecting the same model records it without a select event",
        async |h| {
            vec![
                set_model(h, "setModel think", "think").await,
                set_model(h, "setModel think again", "think").await,
            ]
        },
    );
}

#[test]
fn thinking_changes_persist_only_when_effective() {
    selection_case(
        "thinking writes persist only effective changes",
        async |h| {
            vec![
                set_model(h, "setModel think", "think").await,
                set_level(h, "setThinkingLevel high", "high"),
                set_level(h, "setThinkingLevel high again", "high"),
                levels(h),
            ]
        },
    );
}

#[test]
fn unsupported_levels_clamp_upward() {
    selection_case(
        "unsupported levels clamp to the next supported one",
        async |h| {
            let mapped = h.variant("think", "mapped", json!({"minimal": null, "xhigh": "max"}));
            vec![
                step(
                    "setModel mapped",
                    h.session.set_model(mapped).await.map(UNDEFINED),
                ),
                levels(h),
                set_level(h, "setThinkingLevel minimal", "minimal"),
                set_level(h, "setThinkingLevel xhigh", "xhigh"),
                set_model(h, "setModel think", "think").await,
            ]
        },
    );
}

#[test]
fn plain_model_records_off_without_saving_it() {
    selection_case(
        "a non-reasoning model records off without saving it",
        async |h| {
            vec![
                set_model(h, "setModel think", "think").await,
                set_model(h, "setModel plain", "plain").await,
                step("supportsThinking", Ok(value(h.session.supports_thinking()))),
                levels(h),
                step(
                    "cycleThinkingLevel",
                    h.session.cycle_thinking_level().map(level_cycle_view),
                ),
                set_level(h, "setThinkingLevel medium", "medium"),
            ]
        },
    );
}

#[test]
fn unrecognized_saved_level_selects_first_supported() {
    selection_case(
        "an unrecognized saved level selects the first supported level",
        async |h| {
            let offless = h.variant("think", "offless", json!({"off": null}));
            vec![step(
                "setModel offless",
                h.session.set_model(offless).await.map(UNDEFINED),
            )]
        },
    );
}

#[test]
fn thinking_cycle_wraps_through_supported_levels() {
    selection_case(
        "thinking cycles through supported levels with wraparound",
        async |h| {
            let think = set_model(h, "setModel think", "think").await;
            let first = step(
                "cycle 1",
                h.session.cycle_thinking_level().map(level_cycle_view),
            );
            let second = step(
                "cycle 2",
                h.session.cycle_thinking_level().map(level_cycle_view),
            );
            let mapped = h.variant("think", "mapped", json!({"xhigh": "max"}));
            let mapped = step(
                "setModel mapped",
                h.session.set_model(mapped).await.map(UNDEFINED),
            );
            let xhigh = set_level(h, "setThinkingLevel xhigh", "xhigh");
            let wrap = step(
                "cycle wrap",
                h.session.cycle_thinking_level().map(level_cycle_view),
            );
            vec![think, first, second, mapped, xhigh, wrap]
        },
    );
}

#[test]
fn model_without_credentials_is_refused() {
    selection_case("a model without credentials is refused", async |h| {
        let elsewhere = h.elsewhere("think");
        vec![step(
            "setModel elsewhere",
            h.session.set_model(elsewhere).await.map(UNDEFINED),
        )]
    });
}

#[test]
fn queue_modes_are_saved() {
    selection_case("steering and follow-up modes are saved", async |h| {
        h.session.set_steering_mode(QueueMode::All).unwrap();
        let steering = step("setSteeringMode all", Ok(UNDEFINED(())));
        h.session.set_follow_up_mode(QueueMode::All).unwrap();
        let follow_up = step("setFollowUpMode all", Ok(UNDEFINED(())));
        let settings = h.session.settings_manager().borrow();
        let modes = json!({
            "steering": queue_mode(h.session.steering_mode()),
            "followUp": queue_mode(h.session.follow_up_mode()),
            "settings": {
                "steering": Value::from(settings.get_steering_mode()),
                "followUp": Value::from(settings.get_follow_up_mode()),
            },
        });
        vec![steering, follow_up, step("modes", Ok(modes))]
    });
}

#[test]
fn scoped_cycle_skips_unauthenticated_and_wraps() {
    selection_case(
        "scoped cycling skips unauthenticated models and wraps",
        async |h| {
            h.session.set_scoped_models(vec![
                Harness::scoped(h.model("think"), Some("low")),
                Harness::scoped(h.model("plain"), None),
                Harness::scoped(h.elsewhere("think"), None),
                Harness::scoped(h.model("extra"), Some("high")),
            ]);
            let mut steps = vec![
                cycle(
                    h,
                    "forward from the listed current model",
                    CycleDirection::default(),
                )
                .await,
                cycle(h, "forward", CycleDirection::Forward).await,
                cycle(h, "forward wraps", CycleDirection::Forward).await,
                cycle(h, "backward wraps", CycleDirection::Backward).await,
            ];
            let scoped: Vec<Value> = h
                .session
                .scoped_models()
                .iter()
                .map(|scoped| {
                    json!({"model": model_view(&scoped.model), "thinkingLevel": scoped.thinking_level})
                })
                .collect();
            steps.push(step("scoped", Ok(value(scoped))));
            steps
        },
    );
}

#[test]
fn scoped_cycle_needs_two_authenticated_models() {
    selection_case("scoped cycling needs two authenticated models", async |h| {
        h.session.set_scoped_models(vec![
            Harness::scoped(h.model("think"), None),
            Harness::scoped(h.elsewhere("think"), None),
        ]);
        vec![cycle(h, "forward", CycleDirection::default()).await]
    });
}

#[test]
fn unlisted_current_model_cycles_forward_from_first_index() {
    selection_case(
        "an unlisted current model starts from the first scoped index",
        async |h| {
            h.session.set_scoped_models(vec![
                Harness::scoped(h.model("think"), None),
                Harness::scoped(h.model("extra"), None),
            ]);
            vec![cycle(h, "forward", CycleDirection::default()).await]
        },
    );
}

#[test]
fn unlisted_current_model_cycles_backward_from_first_index() {
    selection_case(
        "an unlisted current model cycles backward from the first scoped index",
        async |h| {
            h.session.set_scoped_models(vec![
                Harness::scoped(h.model("think"), None),
                Harness::scoped(h.model("extra"), None),
                Harness::scoped(h.model("plain"), Some("minimal")),
            ]);
            let unlisted = h.variant("think", "unlisted", json!({}));
            h.session.set_model(unlisted).await.unwrap();
            vec![cycle(h, "backward", CycleDirection::Backward).await]
        },
    );
}

#[test]
fn available_cycle_follows_catalog_order() {
    selection_case("available cycling follows catalog order", async |h| {
        vec![
            cycle(h, "forward", CycleDirection::default()).await,
            cycle(h, "forward", CycleDirection::default()).await,
            cycle(h, "forward wraps", CycleDirection::default()).await,
            cycle(h, "backward", CycleDirection::Backward).await,
        ]
    });
}

#[test]
fn available_cycle_needs_two_models() {
    let expected = case("model_selection", "available cycling needs two models");
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: vec![("only", false)],
            ..HarnessOptions::default()
        })
        .await;
        let results = vec![cycle(&h, "forward", CycleDirection::default()).await];
        assert_eq!(
            json!({"name": expected["name"], "results": results}),
            expected
        );
    });
}

#[test]
fn thinking_selection_hook_does_not_delay_the_change() {
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    let (finish, finished) = tokio::sync::oneshot::channel::<ModelThinkingLevel>();
    let parked = Rc::new(RefCell::new(Some((released, finish))));
    let factory: ExtensionFactory<Local> = Rc::new(move |api| {
        let parked = Rc::clone(&parked);
        api.on::<ThinkingLevelSelectEvent, ()>(
            "thinking_level_select".into(),
            Rc::new(move |event, _| {
                let (released, finish) = parked.borrow_mut().take().unwrap();
                let level = event.level.clone();
                Box::pin(async move {
                    released.await.unwrap();
                    finish.send(level).unwrap();
                    Ok(None)
                })
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    });
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: vec![("think", true)],
            factories: vec![factory],
            ..HarnessOptions::default()
        })
        .await;
        h.session
            .set_thinking_level(ModelThinkingLevel::High)
            .unwrap();
        assert_eq!(h.session.thinking_level(), ModelThinkingLevel::High);
        assert_eq!(
            *h.events.borrow(),
            [json!({"type": "thinking_level_changed", "level": "high"})]
        );
        assert_eq!(
            selection_entries(&h),
            [json!({"type": "thinking_level_change", "thinkingLevel": "high"})]
        );
        release.send(()).unwrap();
        assert_eq!(finished.await.unwrap(), ModelThinkingLevel::High);
    });
}
