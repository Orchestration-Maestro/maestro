//! Conversation policy over the shared server-sent event reader.
use super::events::Events;
use crate::providers::{
    http::{RequestFailure, SseMessages},
    json_text::{raw_json, try_object_record},
};
use crate::{AssistantMessageEventStream, HttpBody, SharedAssistantMessage};
use futures_util::StreamExt;

/// Reduce complete frames until EOF or the exact completion marker.
pub(super) async fn read(
    mut body: HttpBody,
    model: &crate::Model,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) -> Result<(), RequestFailure> {
    let mut decoder = SseMessages::default();
    let mut events = Events::default();
    while let Some(bytes) = body.next().await {
        let bytes = bytes.map_err(|error| RequestFailure::new(error.to_string()))?;
        for frame in decoder.push(&bytes) {
            if frame.data == "[DONE]" {
                events.complete(stream, output);
                return Ok(());
            }
            if !frame.data.is_empty() {
                let chunk = raw_json(&frame.data)
                    .and_then(try_object_record)
                    .map_err(|error| RequestFailure::new(error.to_string()))?;
                events.apply(chunk, model, stream, output)?;
            }
        }
    }
    events.complete(stream, output);
    Ok(())
}
