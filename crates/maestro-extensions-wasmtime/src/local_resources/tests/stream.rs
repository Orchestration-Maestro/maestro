//! A native assistant stream ends with its capability.
use super::ended_endpoint;
use crate::local_resources::{Capability, model_streams::Endpoint};
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
