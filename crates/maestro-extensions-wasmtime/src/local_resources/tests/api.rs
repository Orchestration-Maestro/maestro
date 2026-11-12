//! The admitted calls of the API facade end with its capability.
use super::Pump;
use crate::{
    exec::Request,
    local_resources::Capability,
    tests::{ended_errors, model_support},
};
use maestro_extensions::{
    Local, create_extension_runtime,
    host_operations::{HostOperation, OperationScope},
};

#[test]
fn released_api_rejects_admitted_calls_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let (_owner, api) = crate::tests::api::api(create_extension_runtime::<Local>());
    let lease = pump.registry().insert(Capability::Api(api)).unwrap();
    let api = crate::api_reads::Endpoint(lease.clone());
    let model = model_support::model("ended").read().unwrap().clone();
    let errors = ended_errors(lease, pump, admission, move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            [
                api.set_model(model).await.unwrap_err(),
                api.exec(Request {
                    data: r#"["true",[]]"#.into(),
                    signal: None,
                })
                .await
                .unwrap_err(),
                api.host_operation(HostOperation::ReadSettings(OperationScope::User))
                    .await
                    .unwrap_err(),
            ]
        })
    });
    assert_eq!(errors, ["extension API resource ended"; 3]);
}
