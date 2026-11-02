//! Reclamation after cancelling an operation while its temporary directory is in the table.
use super::*;
use crate::{guest_calls::Callbacks, lifecycle::replies::Replies, local_resources::Pump};
use futures_util::future::{Either, select};
use wasmtime_wasi::WasiView;

/// Uses the real instance state without entering a component or invoking any application port.
fn state() -> State {
    let runtime = maestro_extensions::create_extension_runtime::<maestro_extensions::Local>();
    let (_owner, api) = crate::tests::api::api(runtime);
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump.registry().api(api).unwrap();
    let (calls, _incoming) = tokio::sync::mpsc::unbounded_channel();
    State::new(
        endpoint,
        Callbacks::new(calls.downgrade(), Replies::default()),
        "/unused.wasm",
        crate::Installation {
            version: "test".into(),
            agent_dir: "/test".into(),
        },
        None,
    )
    .unwrap()
}

/// Each store-access boundary reclaims cancelled placements, without accessing the store in Drop.
#[test]
fn cancelled_placement_reclaims_at_both_store_boundaries() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let engine = crate::component_engine(None).unwrap();
            let mut store = wasmtime::Store::new(&engine, state());
            let fd = store
                .data_mut()
                .root_view()
                .directories()
                .unwrap()
                .remove(0)
                .0;
            store
                .run_concurrent(async |store| {
                    observe_cancellations(store, fd.rep()).await;
                })
                .await
                .unwrap();
        });
}

/// Cancels after insertion and alternates the two real table-access boundaries.
async fn observe_cancellations(store: &Accessor<State>, fd: u32) {
    let rooted = store.with_getter::<Root>(State::root_view);
    for iteration in 0..64 {
        let (sent, received) = tokio::sync::oneshot::channel();
        let mut operation = Box::pin(async {
            let placed = place(&rooted, Resource::new_borrow(fd), ".".into(), false)
                .await
                .unwrap();
            sent.send(()).unwrap();
            std::future::pending::<()>().await;
            drop(placed);
        });
        match select(received, &mut operation).await {
            Either::Left((ready, _)) => ready.unwrap(),
            Either::Right(_) => panic!("operation must remain pending"),
        }
        drop(operation);
        let count = store.with(|mut access| {
            if iteration % 2 == 0 {
                access.data_mut().root_view().fs.table.len()
            } else {
                access.data_mut().ctx().table.len()
            }
        });
        assert_eq!(count, 1);
    }
}
