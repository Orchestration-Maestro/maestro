//! A native assistant stream ends with its capability.
use super::Pump;
use crate::{
    local_resources::{Capability, model_streams::Endpoint},
    tests::ended_errors,
};
use maestro_extensions::AssistantMessageEventStream;

#[test]
fn released_assistant_stream_rejects_push_and_end_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let lease = pump
        .registry()
        .insert(Capability::AssistantStream(
            AssistantMessageEventStream::new(),
        ))
        .unwrap();
    let stream = Endpoint(lease.clone());
    let errors = ended_errors(lease, pump, admission, move || {
        [
            stream.push("{}".into()).unwrap_err(),
            stream.end(None).unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension assistant stream resource ended"; 2]);
}
