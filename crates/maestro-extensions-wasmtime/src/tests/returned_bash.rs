//! Native returned operations forward exact operands and separate borrowed output sinks.
use crate::lifecycle::replies::Replies;
use crate::{
    guest_calls::{Call, Callbacks},
    local_resources::Pump,
};
use maestro_extensions::{BashExecOptions, BashOperations, Cancellation};
use std::ffi::OsString;
use wasmtime::{
    Engine, Store,
    component::{Resource, ResourceAny},
};

/// Real Wasmtime host resource identity used only to inspect native endpoint transfers.
struct Identity {
    /// Stable identity, unrelated to a native request lease.
    _id: u32,
}

/// Allocates a Wasmtime resource without claiming this endpoint test invokes a guest method.
pub(super) fn resource() -> (Store<u32>, ResourceAny) {
    let mut store = Store::new(&Engine::default(), 71);
    let resource =
        ResourceAny::try_from_resource(Resource::<Identity>::new_own(71), &mut store).unwrap();
    (store, resource)
}

#[test]
fn returned_operations_overlap_with_exact_operands_and_independent_sinks() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(super::support::credential_operations());
            let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let id = callbacks.allocate().unwrap().id;
            let handle = callbacks.retain(id, admission.clone()).unwrap();
            let (mut store, resource) = resource();
            let operations = handle.bash(resource, pump.registry());
            drop(handle);
            assert!(matches!(incoming.try_recv().unwrap(), Call::Release(_)));
            let signal = Cancellation::new();
            signal.abort();
            let env = [(OsString::from("雪"), OsString::from("literal=\0value"))]
                .into_iter()
                .collect();
            let worker = std::thread::spawn(move || serve(incoming, resource));
            let client = async {
                let (mut first, mut second) = (Vec::new(), Vec::new());
                let mut sink1 = |bytes: &[u8]| first.push(bytes.to_vec());
                let mut sink2 = |bytes: &[u8]| second.push(bytes.to_vec());
                let (one, two) = futures_util::future::join(
                    operations.exec(
                        "raw $command\n",
                        "./unresolved/../雪",
                        BashExecOptions {
                            on_data: &mut sink1,
                            signal: Some(&signal),
                            timeout: Some(-2.5),
                            env: Some(&env),
                        },
                    ),
                    operations.exec(
                        "second",
                        "",
                        BashExecOptions {
                            on_data: &mut sink2,
                            signal: None,
                            timeout: None,
                            env: None,
                        },
                    ),
                )
                .await;
                drop(operations);
                (one, two, first, second)
            };
            drop(admission);
            let ((), (one, two, first, second)) =
                futures_util::future::join(pump.serve(), client).await;
            let released = worker.join().unwrap();
            released.resource_drop_async(&mut store).await.unwrap();
            assert_eq!(one.unwrap().exit_code, Some(7.5));
            assert_eq!(two.unwrap_err().to_string(), "");
            assert_eq!(first, [vec![0, 255], vec![0xf0, 0x9f], vec![0x92, 0xa9]]);
            assert_eq!(second, [vec![], vec![8]]);
        });
}

/// Inspects native operands, delivers exact chunks and returns the final release.
fn serve(
    mut incoming: tokio::sync::mpsc::UnboundedReceiver<Call>,
    resource: ResourceAny,
) -> ResourceAny {
    let one = incoming.blocking_recv().unwrap();
    let two = incoming.blocking_recv().unwrap();
    let (Call::Bash(one), Call::Bash(two)) = (one, two) else {
        panic!("not executions")
    };
    assert_eq!(one.owner.resource, resource);
    assert_eq!(two.owner.resource, resource);
    assert_eq!(one.request.command, "raw $command\n");
    assert_eq!(one.request.cwd, "./unresolved/../雪");
    assert_eq!(one.request.timeout, Some(-2.5));
    assert_eq!(
        one.request.env,
        Some(vec![("雪".into(), Some("literal=\0value".into()))])
    );
    assert!(one.signal.as_ref().unwrap().aborted().unwrap());
    assert_eq!(two.request.command, "second");
    assert_eq!(two.request.cwd, "");
    assert_eq!(two.request.timeout, None);
    assert_eq!(two.request.env, None);
    assert!(two.signal.is_none());
    one.output.on_data(vec![0, 255]).unwrap();
    two.output.on_data(vec![]).unwrap();
    one.output.on_data(vec![0xf0, 0x9f]).unwrap();
    two.output.on_data(vec![8]).unwrap();
    one.output.on_data(vec![0x92, 0xa9]).unwrap();
    one.reply.send(Ok(Some(7.5))).unwrap();
    two.reply.send(Err(String::new())).unwrap();
    drop((
        one.owner, two.owner, one.output, two.output, one.signal, two.signal,
    ));
    let Call::ReleaseResource(released) = incoming.blocking_recv().unwrap() else {
        panic!("not final release")
    };
    assert_eq!(released, resource);
    assert!(incoming.try_recv().is_err());
    released
}

#[cfg(unix)]
#[test]
fn nonunicode_environment_is_rejected_before_guest_admission_without_lossy_conversion() {
    use std::os::unix::ffi::OsStringExt;
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (pump, admission) = Pump::new(super::support::credential_operations());
            let (calls, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let id = callbacks.allocate().unwrap().id;
            let handle = callbacks.retain(id, admission.clone()).unwrap();
            let (mut store, resource) = resource();
            let operations = handle.bash(resource, pump.registry());
            let client = async {
                let invalid = OsString::from_vec(vec![255]);
                for (key, value, expected) in [
                    (invalid.clone(), OsString::from("value"), "key"),
                    (OsString::from("key"), invalid, "value"),
                ] {
                    let env = [(key, value)].into_iter().collect();
                    let mut sink = |_: &[u8]| panic!("invalid operands produced output");
                    let result = operations
                        .exec(
                            "raw",
                            "cwd",
                            BashExecOptions {
                                on_data: &mut sink,
                                signal: None,
                                timeout: None,
                                env: Some(&env),
                            },
                        )
                        .await;
                    assert_eq!(
                        result.unwrap_err().to_string(),
                        format!("extension environment {expected} is not UTF-8")
                    );
                    assert!(incoming.try_recv().is_err());
                }
                drop(operations);
                let Call::ReleaseResource(released) = incoming.recv().await.unwrap() else {
                    panic!("not release")
                };
                released.resource_drop_async(&mut store).await.unwrap();
                drop(handle);
            };
            drop(admission);
            futures_util::future::join(pump.serve(), client).await;
        });
}
