//! Generated resource forwarding for typed authored streams.
use super::{Generated, host, route};
use crate::component_adapter::{Exports, streams::StreamImports};
use crate::{EventStream, ExtensionFuture, ExtensionResult};

/// Construct through the generated imports, without a facade receiver or global host.
pub(crate) fn new_event_stream<T: 'static, R: 'static>(
    is_complete: impl Fn(&T) -> bool + 'static,
    extract_result: impl Fn(&T) -> R + 'static,
) -> ExtensionResult<EventStream<T, R>> {
    Exports::new(Generated).event_stream(is_complete, extract_result)
}

impl StreamImports for Generated {
    type Stream = host::EventStream;
    type Value = host::StreamValue;
    fn new_stream(
        &self,
        classifier: &host::Callback,
        extractor: &host::Callback,
    ) -> ExtensionResult<Self::Stream> {
        host::new_event_stream(classifier, extractor)
    }
    fn new_value(&self, value: host::Callback) -> Self::Value {
        host::StreamValue::new(value)
    }
    fn value_id(&self, value: &Self::Value) -> u32 {
        value.id()
    }
    fn stream_push(&self, stream: &Self::Stream, value: Self::Value) -> ExtensionResult<()> {
        route::drive(&stream.push(value))
    }
    fn stream_end(&self, stream: &Self::Stream, value: Option<Self::Value>) -> ExtensionResult<()> {
        route::drive(&stream.end(value))
    }
    fn stream_next<'a>(
        &'a self,
        stream: &'a Self::Stream,
    ) -> ExtensionFuture<'a, Option<Self::Value>> {
        Box::pin(stream.next())
    }
    fn stream_result<'a>(&'a self, stream: &'a Self::Stream) -> ExtensionFuture<'a, Self::Value> {
        Box::pin(stream.result())
    }
}

/// Dispatch the same author body from ordinary exports and nested import continuations.
pub(super) fn invoke(
    handler: u32,
    value: u32,
    operation: host::StreamOperation,
) -> ExtensionResult<host::StreamSelection> {
    match operation {
        host::StreamOperation::Classify => Exports::<Generated>::classify_stream(handler, value)
            .map(host::StreamSelection::Complete),
        host::StreamOperation::Extract => Exports::new(Generated)
            .extract_stream(handler, value)
            .map(host::StreamSelection::Extracted),
    }
}
