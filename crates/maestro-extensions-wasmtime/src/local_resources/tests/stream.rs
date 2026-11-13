//! A native assistant stream ends with its capability.
use super::{Pump, ended_endpoint, handles};
use crate::{
    local_resources::{Capability, model_streams::Endpoint},
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

#[test]
fn released_assistant_stream_rejects_observations_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::AssistantStream(AssistantMessageEventStream::new()),
        Endpoint,
        |stream| {
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(async {
                    [
                        stream.next().await.unwrap_err(),
                        stream.result().await.unwrap_err(),
                    ]
                })
        },
    );
    assert_eq!(errors, ["extension assistant stream resource ended"; 2]);
}

#[test]
fn released_event_stream_rejects_observations_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let ([classifier, extractor], _releases, _calls) = handles::<2>(&registry);
    let stream = registry.stream(classifier, extractor).unwrap();
    let lease = stream.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                [
                    stream.next().await.err().unwrap(),
                    stream.result().await.err().unwrap(),
                ]
            })
    });
    assert_eq!(errors, ["extension stream resource ended"; 2]);
}
