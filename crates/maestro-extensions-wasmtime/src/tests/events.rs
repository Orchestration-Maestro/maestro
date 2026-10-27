//! Bus callbacks preserve the native listener, emission frame and independently owned tail.
use crate::lifecycle::replies::Replies;
use crate::{
    event_reads::Request,
    guest_calls::{Call, Callbacks, SynchronousKind},
    local_resources::Pump,
};
use maestro_extensions::{ExtensionAPI, Local, create_event_bus_for, create_extension_runtime};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

#[test]
fn native_listener_returns_its_guest_tail_to_the_original_bus() {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let tails = Rc::new(RefCell::new(Vec::new()));
        let queued = tails.clone();
        let diagnostics = Rc::new(RefCell::new(Vec::new()));
        let reported = diagnostics.clone();
        let bus = create_event_bus_for::<Local>(Rc::new(move |tail| queued.borrow_mut().push(tail)), Rc::new(move |error| reported.borrow_mut().push(error.to_owned())));
        let source = serde_json::from_value(serde_json::json!({"path":"/events", "source":"cli", "scope":"temporary", "origin":"top-level"})).unwrap();
        let owner = Rc::new(RwLock::new(maestro_extensions::Extension::new("/events".into(), "/events".into(), Arc::new(RwLock::new(source)))));
        let api = ExtensionAPI::new(&owner, create_extension_runtime::<Local>(), bus.for_extension(), "/events".into());
        let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
        let registry = pump.registry();
        let endpoint = registry.api(api).unwrap();
        let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
        let callback = callbacks.allocate().unwrap();
        let handler = callbacks.retain(callback.id, admission.clone()).unwrap();
        let tail = callbacks.allocate().unwrap();
        let tail = callbacks.retain(tail.id, admission.clone()).unwrap();
        let worker = std::thread::spawn(move || endpoint.event(|reply| Request::On(" exact ".into(), handler, reply)));
        assert!(pump.answer_next().await);
        let subscription = worker.join().unwrap().unwrap();
        let worker = std::thread::spawn(move || {
            let call = incoming.blocking_recv().unwrap();
            let Call::Synchronous(call) = call else { panic!("missing listener") };
            assert!(matches!(call.kind, SynchronousKind::Listener));
            assert_eq!(call.data, r#"{"value":" raw "}"#);
            call.emitter.start_tail(tail).unwrap();
            call.completion.send(Ok(())).unwrap();
            let Call::Tail(_, reply) = incoming.blocking_recv().unwrap() else { panic!("missing tail") };
            reply.send(Err(" tail error ".into())).unwrap();
        });
        bus.emit("exact", serde_json::Value::Null).await;
        assert!(tails.borrow().is_empty());
        bus.emit(" exact ", serde_json::json!({"value":" raw "})).await;
        let tail = tails.borrow_mut().pop().unwrap();
        tail.await;
        worker.join().unwrap();
        assert_eq!(*diagnostics.borrow(), ["Event handler error ( exact ):  tail error \n"]);
        let worker = std::thread::spawn(move || subscription.unsubscribe().unwrap());
        bus.clear();
        drop((calls, admission));
        pump.serve().await;
        worker.join().unwrap();
    });
}

#[test]
fn ordinary_emission_decodes_before_calling_the_original_native_bus() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (_owner, api) = super::api::api(create_extension_runtime::<Local>());
            let observed = Rc::new(RefCell::new(Vec::new()));
            let listener = observed.clone();
            api.events.on(
                " exact ",
                Rc::new(move |data, _| {
                    listener.borrow_mut().push(data);
                    Ok(None)
                }),
            );
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump.registry().api(api).unwrap();
            let worker = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        assert!(
                            endpoint
                                .emit(" exact ".into(), "not JSON".into())
                                .await
                                .is_err()
                        );
                        endpoint.emit("exact".into(), "null".into()).await.unwrap();
                        endpoint
                            .emit(" exact ".into(), r#"{"data":" raw "}"#.into())
                            .await
                            .unwrap();
                    });
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            assert_eq!(
                *observed.borrow(),
                [Arc::new(serde_json::json!({"data":" raw "}))]
            );
        });
}
