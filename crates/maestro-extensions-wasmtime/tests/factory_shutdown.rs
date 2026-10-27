//! A process can exit after dropping a runtime that owns multiple component Stores.
#![cfg(test)]
use super::retained_components::{loaded_on, view};
use maestro_extensions::{Local, create_event_bus_for};
use std::rc::Rc;

#[test]
fn multiple_store_workers_exit_with_their_local_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let loads = (0..32).map(|_| async {
            let bus = create_event_bus_for::<Local>(
                Rc::new(|_| panic!("no tails")),
                Rc::new(|error| panic!("{error}")),
            );
            loaded_on(bus.for_extension()).await
        });
        let instances = futures_util::future::join_all(loads).await;
        for (_, runner, events) in &instances {
            let component = view(runner, events, "shutdown").unwrap().unwrap();
            assert_eq!(component.try_render(17).unwrap(), ["shutdown:17"]);
        }
        drop(instances);
    });
}
