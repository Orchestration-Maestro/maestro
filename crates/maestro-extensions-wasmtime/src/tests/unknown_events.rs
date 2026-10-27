//! Unknown names retain opaque guest ownership without a callable surrogate.
use crate::lifecycle::replies::Replies;
use crate::{
    guest_calls::{Call, Callbacks, Handle},
    local_resources::Pump,
};
use maestro_extensions::{ExtensionRunner, Local, Presence, SessionStartEvent, SessionStartReason};
use std::{rc::Rc, sync::RwLock};

#[test]
fn unknown_event_name_keeps_guest_handle_but_no_native_event_dispatches_it() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runtime = maestro_extensions::create_extension_runtime::<Local>();
            let (owner, api) = super::api::api(runtime.clone());
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let callback = callbacks.allocate().unwrap();
            let id = callback.id;
            let handle = callbacks.retain(id, admission.clone()).unwrap();
            crate::event_registration::register(&api, " unknown ".into(), handle, pump.registry())
                .unwrap();
            {
                let owner = owner.read().unwrap();
                let retained = &owner.handlers[" unknown "][0];
                assert_eq!(retained.downcast_ref::<Handle>().unwrap().id(), id);
                assert_eq!(owner.handlers.len(), 1);
            }
            let runner = ExtensionRunner::<Local>::new(
                Rc::new(RwLock::new(vec![owner.clone()])),
                runtime,
                super::session::runner_session("/unknown").0,
                Rc::new(|_| {}),
                maestro_extensions::LiveTheme::default(),
            );
            assert!(
                runner
                    .emit(&mut SessionStartEvent {
                        reason: SessionStartReason::Startup,
                        previous_session_file: Presence::Missing,
                    })
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(incoming.try_recv().is_err());
            assert!(!runner.has_handlers("session_start"));
            drop((runner, owner, api));
            assert!(matches!(incoming.try_recv().unwrap(), Call::Release(_)));
            assert!(incoming.try_recv().is_err());
            drop((admission, calls));
            pump.serve().await;
        });
}
