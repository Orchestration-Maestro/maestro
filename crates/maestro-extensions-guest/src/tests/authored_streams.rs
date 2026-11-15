//! In-process adapter control using the same native event buffer as the production host.
use super::controlled::Controlled;
use crate::{
    ExtensionFuture, ExtensionResult,
    component_adapter::{Exports, Imports, streams::StreamImports},
};
use std::rc::Rc;

/// One host-retained identity, released outside native buffer locks.
pub(crate) struct Value {
    /// Original callback-table key.
    id: u32,
    /// A transferred value's owner; callback identities keep theirs in the table instead.
    _owner: Option<super::controlled::Callback>,
}
impl Value {
    /// Read through the owner so callback closures capture its whole final-release guard.
    fn id(&self) -> u32 {
        self.id
    }
}
impl Drop for Value {
    fn drop(&mut self) {
        crate::component_adapter::release(self.id);
    }
}
/// Only the native owner buffers events and retains results.
pub(crate) struct Stream(maestro_models::EventStream<Rc<Value>, Rc<Value>, maestro_models::Local>);

impl StreamImports for Controlled {
    type Stream = Stream;
    type Value = Rc<Value>;
    fn new_stream(
        &self,
        classifier: &Self::Callback,
        extractor: &Self::Callback,
    ) -> ExtensionResult<Stream> {
        let classifier = Value {
            id: self.callback_id(classifier),
            _owner: None,
        };
        let extractor = Value {
            id: self.callback_id(extractor),
            _owner: None,
        };
        let imports = self.clone();
        Ok(Stream(maestro_models::EventStream::new_local(
            move |event: &Rc<Value>| {
                Exports::<Controlled>::classify_stream(classifier.id(), event.id).unwrap()
            },
            move |event| {
                Exports::new(imports.clone())
                    .extract_stream(extractor.id(), event.id)
                    .unwrap()
            },
        )))
    }
    fn new_value(&self, value: Self::Callback) -> Self::Value {
        Rc::new(Value {
            id: self.callback_id(&value),
            _owner: Some(value),
        })
    }
    fn value_id(&self, value: &Self::Value) -> u32 {
        value.id
    }
    fn stream_push(&self, stream: &Stream, value: Self::Value) -> ExtensionResult<()> {
        stream.0.push(value);
        Ok(())
    }
    fn stream_end(&self, stream: &Stream, value: Option<Self::Value>) -> ExtensionResult<()> {
        stream.0.end(value);
        Ok(())
    }
    fn stream_next<'a>(&'a self, stream: &'a Stream) -> ExtensionFuture<'a, Option<Self::Value>> {
        Box::pin(async move { Ok(stream.0.next().await) })
    }
    fn stream_result<'a>(&'a self, stream: &'a Stream) -> ExtensionFuture<'a, Self::Value> {
        Box::pin(async move { Ok(stream.0.result().await) })
    }
}

#[test]
fn authored_stream_adapter_keeps_local_value_identity() -> ExtensionResult<()> {
    let imports = Controlled::default();
    let stream = Exports::new(imports).event_stream(|_: &Rc<i32>| true, Rc::clone)?;
    let value = Rc::new(7);
    stream.push(value.clone())?;
    super::block_on(async {
        let result = stream.result().await?;
        assert!(Rc::ptr_eq(&value, result.as_ref()));
        let event = stream.next().await?.unwrap();
        assert!(Rc::ptr_eq(&value, event.as_ref()));
        stream.end(None)?;
        assert!(stream.next().await?.is_none());
        Ok(())
    })
}
