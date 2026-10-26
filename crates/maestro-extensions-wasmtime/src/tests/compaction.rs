//! Compaction preserves option presence and calls its real native notification operands.
use crate::lifecycle::replies::Replies;
use crate::read_imports::ReadEndpoint;
use crate::{
    compact_calls::{Compact, Data},
    context_reads::{Endpoint, Owner, Request},
    guest_calls::{Call, Callbacks, Handle, SynchronousKind},
    local_resources::Pump,
};
use maestro_extensions::{
    CompactOptions, ExtensionContextActions, ExtensionExecution, Local, types::ProviderActions,
};
use std::{cell::RefCell, rc::Rc};

#[test]
fn compaction_keeps_absent_empty_and_full_options_and_original_callbacks() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            for command in [false, true] {
                compaction(command).await;
            }
        });
}

/// Exercises both canonical context identities against the same native action owner.
async fn compaction(command: bool) {
    let runner = crate::tests::runner("/compaction");
    let requests = Rc::new(RefCell::new(Vec::new()));
    let observed = requests.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                compact: Rc::new(move |options| {
                    observed.borrow_mut().push(options);
                    Ok(())
                }),
                ..ExtensionContextActions::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    let (_owner, api) = super::api::api(maestro_extensions::create_extension_runtime::<Local>());
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let api = registry.api(api).unwrap();
    let context = if command {
        Owner::Command(runner.create_command_context())
    } else {
        Owner::Base(runner.create_context())
    };
    let context = registry.context(context).unwrap();
    let (calls, incoming) = tokio::sync::mpsc::unbounded_channel();
    let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
    let complete = callbacks.allocate().unwrap();
    let complete = callbacks.retain(complete.id, admission.clone()).unwrap();
    let error = callbacks.allocate().unwrap();
    let error = callbacks.retain(error.id, admission.clone()).unwrap();
    let worker = std::thread::spawn(move || submit(&context, &api, complete, error));
    for _ in 0..3 {
        assert!(pump.answer_next().await);
    }
    worker.join().unwrap();
    let full = full_options(std::mem::take(&mut *requests.borrow_mut()));
    let on_error = full.on_error.clone().unwrap();
    routed_failure(&mut pump, move || on_error(" nested error ".into())).await;
    let worker = std::thread::spawn(move || notifications(incoming));
    let complete = (full.on_complete.as_ref().unwrap())(
        serde_json::from_value(
            serde_json::json!({"summary":" raw ","firstKeptEntryId":" id ","tokensBefore":7}),
        )
        .unwrap(),
    );
    let error = (full.on_error.as_ref().unwrap())(" error ".into());
    worker.join().unwrap();
    complete.unwrap();
    error.unwrap();
    drop((full, calls, admission));
    pump.serve().await;
}

/// Answers a failure notification made while the Store waits in a routed request.
async fn routed_failure<E: std::fmt::Debug + 'static>(
    pump: &mut Pump,
    notify: impl FnOnce() -> Result<(), E> + 'static,
) {
    let nested = super::during_routed_request(pump, notify, |call| {
        let Call::Synchronous(call) = call else {
            panic!("missing nested failure callback")
        };
        assert!(matches!(call.kind, SynchronousKind::CompactionError));
        assert_eq!(call.data, " nested error ");
        call.completion.send(Ok(())).unwrap();
    })
    .await;
    nested.unwrap();
}

/// Sends absent, empty and full options through the original context endpoint.
fn submit(context: &Endpoint, api: &crate::api_reads::Endpoint, complete: Handle, error: Handle) {
    for (data, on_complete, on_error) in [
        (None, None, None),
        (
            Some(Data {
                custom_instructions: None,
            }),
            None,
            None,
        ),
        (
            Some(Data {
                custom_instructions: Some(" keep ".into()),
            }),
            Some(complete),
            Some(error),
        ),
    ] {
        context
            .read(|reply| {
                Request::Compact(
                    Compact {
                        data,
                        on_complete,
                        on_error,
                        api: api.clone(),
                    },
                    reply,
                )
            })
            .unwrap();
    }
}

/// Checks independently optional native records before invoking the full callbacks.
fn full_options(requests: Vec<Option<CompactOptions>>) -> CompactOptions {
    let mut requests = requests.into_iter();
    assert!(requests.next().unwrap().is_none());
    let empty = requests.next().unwrap().unwrap();
    assert!(
        empty.custom_instructions.is_none()
            && empty.on_complete.is_none()
            && empty.on_error.is_none()
    );
    let full = requests.next().unwrap().unwrap();
    assert_eq!(full.custom_instructions.as_deref(), Some(" keep "));
    assert!(requests.next().is_none());
    full
}

/// Observes the exact native completion record and unmodified failure message.
fn notifications(mut incoming: tokio::sync::mpsc::UnboundedReceiver<Call>) {
    for (success, expected) in [
        (
            true,
            r#"{"summary":" raw ","firstKeptEntryId":" id ","tokensBefore":7.0}"#,
        ),
        (false, " error "),
    ] {
        let Call::Synchronous(call) = incoming.blocking_recv().unwrap() else {
            panic!("missing callback")
        };
        let observed_kind = matches!(
            (success, call.kind),
            (true, SynchronousKind::CompactionComplete) | (false, SynchronousKind::CompactionError)
        );
        let data = call.data;
        call.completion.send(Ok(())).unwrap();
        assert!(observed_kind);
        if success {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&data).unwrap(),
                serde_json::from_str::<serde_json::Value>(expected).unwrap()
            );
        } else {
            assert_eq!(data, expected);
        }
    }
}
