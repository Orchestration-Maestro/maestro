//! Typed local values use the callback identity owner; the host alone buffers events.
use super::{Exports, Imports, callbacks};
use crate::{EventStream, EventStreamPort, ExtensionFuture, ExtensionResult};
use std::{any::Any, marker::PhantomData, rc::Rc};

/// An arbitrary retained author value in the existing identity table.
pub(super) type Value = Rc<dyn Any>;
/// Synchronous author classifier, cloned out of callback storage before invocation.
#[derive(Clone)]
pub(super) struct Classifier(pub(super) Rc<dyn Fn(u32) -> ExtensionResult<bool>>);
/// Synchronous author extractor returning a distinct retained result type.
#[derive(Clone)]
pub(super) struct Extractor(pub(super) Rc<dyn Fn(u32) -> ExtensionResult<Value>>);

/// Typed stream resource imports, independent of unrelated context capabilities.
pub(crate) trait StreamImports: Imports {
    /// The native event-buffer resource.
    type Stream: 'static;
    /// A retained author value with independently released native ownership.
    type Value: 'static;
    /// Construct the native buffer with the original callback identities.
    fn new_stream(
        &self,
        classifier: &Self::Callback,
        extractor: &Self::Callback,
    ) -> ExtensionResult<Self::Stream>;
    /// Transfer a local value to native ownership.
    fn new_value(&self, value: Self::Callback) -> Self::Value;
    /// Read the callback-table identity of a retained value.
    fn value_id(&self, value: &Self::Value) -> u32;
    /// Admit through the native buffer and synchronous callback route.
    fn stream_push(&self, stream: &Self::Stream, value: Self::Value) -> ExtensionResult<()>;
    /// End through the native result owner.
    fn stream_end(&self, stream: &Self::Stream, value: Option<Self::Value>) -> ExtensionResult<()>;
    /// Await one native competing reader.
    fn stream_next<'a>(
        &'a self,
        stream: &'a Self::Stream,
    ) -> ExtensionFuture<'a, Option<Self::Value>>;
    /// Await one native independent result observation.
    fn stream_result<'a>(&'a self, stream: &'a Self::Stream) -> ExtensionFuture<'a, Self::Value>;
}

/// One typed facade over the original resource and host adapter.
struct Port<I: StreamImports, T, R> {
    /// Original adapter.
    imports: I,
    /// Shared resource keeps observations alive after an alias is dropped.
    stream: Rc<I::Stream>,
    /// Value types never impose serialization or Clone bounds.
    types: PhantomData<(T, R)>,
}

/// Transfer a local value without encoding or cloning it.
fn retain<I: StreamImports>(imports: &I, value: Value) -> I::Value {
    let callback = callbacks::Identity::new(imports).transfer(value);
    imports.new_value(callback)
}
/// Clone only the Rc owner, checking the type fixed by the stream facade.
fn value<T: 'static>(id: u32) -> ExtensionResult<Rc<T>> {
    callbacks::find::<Value>(id)?
        .downcast::<T>()
        .map_err(|_| "extension stream value type mismatch".into())
}

impl<I: StreamImports, T: 'static, R: 'static> EventStreamPort<T, R> for Port<I, T, R> {
    fn push(&self, event: T) -> ExtensionResult<()> {
        self.imports
            .stream_push(&self.stream, retain(&self.imports, Rc::new(event)))
    }
    fn end(&self, result: Option<R>) -> ExtensionResult<()> {
        let result = result.map(|result| retain(&self.imports, Rc::new(result)));
        self.imports.stream_end(&self.stream, result)
    }
    fn next(&self) -> ExtensionFuture<'static, Option<Rc<T>>> {
        let (imports, stream) = (self.imports.clone(), self.stream.clone());
        Box::pin(async move {
            imports
                .stream_next(&stream)
                .await?
                .map(|retained| value(imports.value_id(&retained)))
                .transpose()
        })
    }
    fn result(&self) -> ExtensionFuture<'static, Rc<R>> {
        let (imports, stream) = (self.imports.clone(), self.stream.clone());
        Box::pin(async move {
            let retained = imports.stream_result(&stream).await?;
            value(imports.value_id(&retained))
        })
    }
}

impl<I: StreamImports> Exports<I> {
    /// Create a typed facade using only the host's buffer and existing identity table.
    pub(crate) fn event_stream<T: 'static, R: 'static>(
        &self,
        is_complete: impl Fn(&T) -> bool + 'static,
        extract_result: impl Fn(&T) -> R + 'static,
    ) -> ExtensionResult<EventStream<T, R>> {
        let classifier = callbacks::Identity::new(&self.imports);
        classifier.stage(Classifier(Rc::new(move |id| {
            Ok(is_complete(value::<T>(id)?.as_ref()))
        })));
        let extractor = callbacks::Identity::new(&self.imports);
        extractor.stage(Extractor(Rc::new(move |id| {
            Ok(Rc::new(extract_result(value::<T>(id)?.as_ref())))
        })));
        let stream = self
            .imports
            .new_stream(&classifier.handle, &extractor.handle);
        // The native stream now owns both callbacks, including after the facade is dropped.
        classifier.settle(stream.is_ok());
        extractor.settle(stream.is_ok());
        let stream = stream?;
        Ok(EventStream::from_port(Rc::new(Port {
            imports: self.imports.clone(),
            stream: Rc::new(stream),
            types: PhantomData,
        })))
    }
    /// Run a classifier outside table borrows before returning to native admission.
    pub(crate) fn classify_stream(handler: u32, event: u32) -> ExtensionResult<bool> {
        (callbacks::find::<Classifier>(handler)?.0)(event)
    }
    /// Run an extractor outside table borrows, retaining its actual returned value.
    pub(crate) fn extract_stream(&self, handler: u32, event: u32) -> ExtensionResult<I::Value> {
        Ok(retain(
            &self.imports,
            (callbacks::find::<Extractor>(handler)?.0)(event)?,
        ))
    }
}
