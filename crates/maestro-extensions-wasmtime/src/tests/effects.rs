//! Retained callback operands use their original native owners.
use crate::local_resources::Pump;
use maestro_extensions::{AgentToolUpdateCallback, Local, create_event_bus_for};
use std::{cell::RefCell, rc::Rc};

#[test]
fn opaque_theme_releases_its_native_owner_after_the_final_capability() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let theme = Rc::new(
                maestro_theme::Theme::new(
                    [],
                    [],
                    maestro_theme::ColorMode::Truecolor,
                    maestro_theme::ThemeOptions::default(),
                )
                .unwrap(),
            );
            let original = Rc::downgrade(&theme);
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump
                .registry()
                .theme(maestro_extensions::LiveTheme::from_theme(theme))
                .unwrap();
            let copy = endpoint.clone();
            drop(endpoint);
            assert!(original.upgrade().is_some());
            drop(copy);
            drop(admission);
            pump.serve().await;
            assert!(original.upgrade().is_none());
        });
}

#[test]
fn tool_updates_decode_before_calling_the_original_observer() {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed = calls.clone();
        let callback: AgentToolUpdateCallback<serde_json::Value, Local> = Rc::new(move |partial| {
            observed.borrow_mut().push(partial);
        });
        let original = Rc::downgrade(&callback);
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let update = pump.registry().tool_update(callback).unwrap();
        let worker = std::thread::spawn(move || {
            assert!(update.update("not JSON".into()).result().is_err());
            update.update(r#"{"content":[{"type":"text","text":" exact "}],"details":{"part":0},"terminate":false}"#.into()).result().unwrap();
            let copy = update.clone();
            drop(update);
            copy.update(r#"{"content":[]}"#.into()).result().unwrap();
        });
        drop(admission);
        pump.serve().await;
        worker.join().unwrap();
        let calls = calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(serde_json::to_value(&calls[0]).unwrap(), serde_json::json!({"content":[{"type":"text","text":" exact "}],"details":{"part":0},"terminate":false}));
        assert_eq!(serde_json::to_value(&calls[1]).unwrap(), serde_json::json!({"content":[]}));
        assert!(original.upgrade().is_none());
    });
}

#[test]
fn dropping_a_subscription_resource_does_not_unsubscribe_the_native_listener() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let bus = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
            let calls = Rc::new(RefCell::new(Vec::new()));
            let observed = calls.clone();
            let listener: maestro_extensions::EventListener<Local> = Rc::new(move |data, _| {
                observed.borrow_mut().push(data);
                Ok(None)
            });
            let retained = bus.on(" exact ", listener.clone());
            let removed = bus.on(" exact ", listener);
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let retained = pump.registry().subscription(retained).unwrap();
            let removed = pump.registry().subscription(removed).unwrap();
            let worker = std::thread::spawn(move || {
                removed.unsubscribe().unwrap();
                removed.unsubscribe().unwrap();
                drop((removed, retained));
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            bus.emit("exact", serde_json::Value::Null).await;
            bus.emit(" exact ", serde_json::json!({"value":0})).await;
            assert_eq!(
                *calls.borrow(),
                [std::sync::Arc::new(serde_json::json!({"value":0}))]
            );
        });
}

#[test]
fn retained_emitter_uses_original_queue_and_returns_native_retirement_error() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let bus =
                create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|error| panic!("{error}")));
            let owner = bus.for_extension();
            let calls = Rc::new(RefCell::new(Vec::new()));
            let observed = calls.clone();
            bus.for_extension().on(
                " exact ",
                Rc::new(move |data, _| {
                    observed.borrow_mut().push(data);
                    Ok(None)
                }),
            );
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let registry = pump.registry();
            let endpoint = owner.invoke_callback(|emitter| {
                emitter.emit(" exact ", serde_json::json!(1));
                let endpoint = drive_emitter(&registry, emitter.frame());
                assert!(calls.borrow().is_empty());
                endpoint
            });
            assert_eq!(
                *calls.borrow(),
                [
                    std::sync::Arc::new(serde_json::json!(1)),
                    std::sync::Arc::new(serde_json::json!(2))
                ]
            );
            let retired = maestro_extensions::types::ExtensionRuntimeState::new();
            retired.invalidate(None);
            let expected = retired.assert_active().unwrap_err().to_string();
            let worker = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        assert_eq!(
                            endpoint.snapshot(" exact ".into(), "3".into()).unwrap_err(),
                            expected
                        );
                    });
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            assert_eq!(calls.borrow().len(), 2);
        });
}

/// Uses the sole synchronous waiter while the Store-side worker submits real emissions.
fn drive_emitter(
    registry: &crate::local_resources::Registry,
    frame: maestro_extensions::callbacks::CallbackFrame<Local>,
) -> crate::local_resources::effects::EmitterEndpoint {
    let endpoint = registry.emitter(frame).unwrap();
    let mut worker = None;
    registry
        .invoke_sync(None, |completion, _| {
            worker = Some(std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        assert!(
                            endpoint
                                .snapshot(" exact ".into(), "not JSON".into())
                                .is_err()
                        );
                        let (snapshot, tags) =
                            endpoint.snapshot(" exact ".into(), "2".into()).unwrap();
                        assert_eq!(tags, [None]);
                        endpoint.segment(snapshot, 0, 1).result().unwrap();
                        endpoint.end_snapshot(snapshot).unwrap();
                    });
                completion.complete(Ok(()));
                endpoint
            }));
            Ok(())
        })
        .unwrap();
    worker.unwrap().join().unwrap()
}
