//! Per-frame output reaches each original borrowed sink without another request pump.
use crate::local_resources::Pump;
use maestro_extensions::types::ExtensionRuntimeState;
use tokio::sync::oneshot;

#[test]
fn overlapping_output_frames_preserve_chunks_results_and_retirement() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let registry = pump.registry();
            let (one, mut first) = registry.bash_output().unwrap();
            let (two, mut second) = registry.bash_output().unwrap();
            let (done1, result1) = oneshot::channel();
            let (done2, result2) = oneshot::channel();
            let (finished, retired) = oneshot::channel();
            let worker = std::thread::spawn(move || {
                one.on_data(vec![1, 0, 255]).unwrap();
                two.on_data(vec![2, 0, 255]).unwrap();
                one.on_data(vec![]).unwrap();
                two.on_data(vec![0xf0, 0x9f]).unwrap();
                one.on_data(vec![10]).unwrap();
                two.on_data(vec![0x92, 0xa9]).unwrap();
                done1.send(Ok(Some(7.5))).unwrap();
                done2.send(Err(String::new())).unwrap();
                retired.blocking_recv().unwrap();
                let stale = ExtensionRuntimeState::new();
                stale.invalidate(None);
                let expected = stale.assert_active().unwrap_err().to_string();
                assert_eq!(one.on_data(vec![99]).unwrap_err(), expected);
                assert_eq!(two.on_data(vec![99]).unwrap_err(), expected);
            });
            let consumers = async {
                let mut chunks1 = Vec::new();
                let mut chunks2 = Vec::new();
                let mut sink1 = |bytes: &[u8]| chunks1.push(bytes.to_vec());
                let mut sink2 = |bytes: &[u8]| chunks2.push(bytes.to_vec());
                let (one, two) = futures_util::future::join(
                    first.finish(async { result1.await.unwrap() }, &mut sink1),
                    second.finish(async { result2.await.unwrap() }, &mut sink2),
                )
                .await;
                drop((first, second));
                finished.send(()).unwrap();
                assert_eq!(one.unwrap(), Some(7.5));
                assert_eq!(two.unwrap_err(), "");
                assert_eq!(chunks1, [vec![1, 0, 255], vec![], vec![10]]);
                assert_eq!(
                    chunks2,
                    [vec![2, 0, 255], vec![0xf0, 0x9f], vec![0x92, 0xa9]]
                );
            };
            drop(admission);
            futures_util::future::join(pump.serve(), consumers).await;
            worker.join().unwrap();
        });
}

#[test]
fn dropping_an_output_consumer_retires_its_retained_guest_key() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (endpoint, output) = pump.registry().bash_output().unwrap();
            drop(output);
            let worker = std::thread::spawn(move || {
                let state = ExtensionRuntimeState::new();
                state.invalidate(None);
                assert_eq!(
                    endpoint.on_data(vec![0]).unwrap_err(),
                    state.assert_active().unwrap_err().to_string()
                );
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
        });
}
