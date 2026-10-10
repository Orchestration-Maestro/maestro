//! Typed progress events and the replaceable callback slot.
use super::{DefaultPackageManager, PackageOperations};
use std::{future::Future, io, rc::Rc};

/// The phase an event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressEventType {
    /// An operation is starting.
    Start,
    /// An operation reports intermediate progress.
    Progress,
    /// An operation finished.
    Complete,
    /// An operation failed.
    Error,
}
/// The operation an event belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressAction {
    /// Installing a source.
    Install,
    /// Removing a source.
    Remove,
    /// Updating a source.
    Update,
    /// Cloning a repository.
    Clone,
    /// Pulling a repository.
    Pull,
}
/// One notification about an acquisition operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressEvent {
    /// The phase reported.
    pub r#type: ProgressEventType,
    /// The operation reported.
    pub action: ProgressAction,
    /// The source spelling the caller supplied.
    pub source: String,
    /// The start text or the failure text; absent on completion.
    pub message: Option<String>,
}
/// A caller callback; an error it returns fails the emitting operation.
pub type ProgressCallback = Rc<dyn Fn(&ProgressEvent) -> io::Result<()>>;

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Replaces or clears the callback used by the next emission.
    pub(super) fn replace_progress_callback(&self, callback: Option<ProgressCallback>) {
        *self.progress.borrow_mut() = callback;
    }
    /// Calls the current callback, if any, without holding the slot.
    fn emit(
        &self,
        r#type: ProgressEventType,
        action: ProgressAction,
        source: &str,
        message: Option<String>,
    ) -> io::Result<()> {
        let callback = self.progress.borrow().clone();
        callback.map_or(Ok(()), |callback| {
            callback(&ProgressEvent {
                r#type,
                action,
                source: source.to_owned(),
                message,
            })
        })
    }
    /// Reports start, runs the operation, then reports completion or its failure.
    ///
    /// A failing start callback stops before the operation and reports nothing further;
    /// any later failure, including the completion callback's, is reported as an error
    /// event and returned unless the error callback itself fails.
    pub(super) async fn with_progress(
        &self,
        action: ProgressAction,
        source: &str,
        message: String,
        operation: impl Future<Output = io::Result<()>>,
    ) -> io::Result<()> {
        self.emit(ProgressEventType::Start, action, source, Some(message))?;
        let result = match operation.await {
            Ok(()) => self.emit(ProgressEventType::Complete, action, source, None),
            Err(error) => Err(error),
        };
        if let Err(error) = &result {
            let text = Some(error.to_string());
            self.emit(ProgressEventType::Error, action, source, text)?;
        }
        result
    }
}
