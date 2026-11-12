//! A native assistant stream ends with its capability.
use super::{Pump, ended_endpoint};
use crate::{
    lifecycle::replies::Replies,
    local_resources::{Capability, Registry, model_streams::Endpoint},
    tests::ended_errors,
};
use maestro_extensions::AssistantMessageEventStream;

#[test]
fn released_assistant_stream_rejects_push_and_end_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::AssistantStream(AssistantMessageEventStream::new()),
        Endpoint,
        |stream| {
            [
                stream.push("{}".into()).unwrap_err(),
                stream.end(None).unwrap_err(),
            ]
        },
    );
    assert_eq!(errors, ["extension assistant stream resource ended"; 2]);
}

/// Callback handles that need no Store: their releases go to the returned receiver.
fn handles<const N: usize>(
    registry: &Registry,
) -> (
    [crate::guest_calls::Handle; N],
    tokio::sync::mpsc::UnboundedReceiver<crate::guest_calls::Call>,
    tokio::sync::mpsc::UnboundedSender<crate::guest_calls::Call>,
) {
    let (calls, releases) = tokio::sync::mpsc::unbounded_channel();
    let callbacks = crate::guest_calls::Callbacks::new(calls.downgrade(), Replies::default());
    let handles = std::array::from_fn(|id| {
        let observer = callbacks.observer(u32::try_from(id).unwrap());
        observer.handle(registry.admission().unwrap()).unwrap()
    });
    (handles, releases, calls)
}

#[test]
fn released_event_stream_rejects_push_and_end_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let ([classifier, extractor, value], _releases, _calls) = handles::<3>(&registry);
    let stream = registry.stream(classifier, extractor).unwrap();
    let lease = stream.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        [
            stream.push(value).result().unwrap_err(),
            stream.end(None).result().unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension stream resource ended"; 2]);
}
