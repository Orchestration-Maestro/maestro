//! A native local shell ends with its capability.
use super::{Pump, handles};
use crate::{
    guest_calls::bash::BashRequest,
    local_resources::{Capability, local_bash::Execution},
    tests::ended_errors,
};
use maestro_extensions::{Local, create_extension_runtime};

#[test]
fn released_local_shell_rejects_execution_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let (_owner, api) = crate::tests::api::api(create_extension_runtime::<Local>());
    let api = registry.api(api).unwrap();
    let ([output], _releases, _calls) = handles::<1>(&registry);
    let operations =
        maestro_extensions::local::create_local_bash_operations("/unused".into(), None);
    let shell = registry
        .insert(Capability::LocalBash(operations))
        .map(crate::local_resources::local_bash::Endpoint)
        .unwrap();
    let lease = shell.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        let execution = Execution {
            request: BashRequest {
                command: "true".into(),
                cwd: "/".into(),
                timeout: None,
                env: None,
            },
            signal: None,
            output,
            api,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        [runtime.block_on(shell.execute(execution)).unwrap_err()]
    });
    assert_eq!(errors, ["extension local shell resource ended"]);
}
