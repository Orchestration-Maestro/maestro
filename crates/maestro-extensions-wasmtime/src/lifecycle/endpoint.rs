//! One retained Store endpoint owns admission, submission and typed replies.
use super::replies::{Replies, Reply};
use crate::{guest_calls::Call, local_resources::Admission};
use tokio::sync::mpsc::UnboundedSender;

/// Shared Store admission carried by callbacks and guest resources alike.
#[derive(Clone)]
pub(crate) struct Endpoint {
    /// Original Store receiver, strong only while native work retains it.
    pub(crate) calls: UnboundedSender<Call>,
    /// Original local request owner stays available through final release.
    _admission: Admission,
    /// One fault and settlement owner for this Store.
    pub(crate) replies: Replies,
}
impl Endpoint {
    /// Retains the original two execution owners without creating another driver.
    pub(crate) fn new(
        calls: UnboundedSender<Call>,
        admission: Admission,
        replies: Replies,
    ) -> Self {
        Self {
            calls,
            _admission: admission,
            replies,
        }
    }
    /// Submits exactly one typed request to the original Store receiver.
    pub(crate) fn send(&self, call: Call) -> Result<(), String> {
        self.calls
            .send(call)
            .map_err(|_| "extension Store owner ended".to_owned())
    }
    /// Enters a synchronous target, through the active native operation when one owns
    /// this instance, and services local requests until its typed reply settles.
    pub(crate) fn synchronous<T: Send + 'static>(
        &self,
        registry: &crate::local_resources::Registry,
        make: impl FnOnce(Reply<T>) -> Result<Call, String>,
    ) -> Result<T, String> {
        self.replies
            .synchronous(registry, |reply| registry.dispatch(self, make(reply)?))
    }
    /// Admits an asynchronous target through the active native operation when one owns this
    /// instance, so its prefix runs before that caller continues, and awaits its result.
    pub(crate) async fn call<T: Send + 'static>(
        &self,
        registry: &crate::local_resources::Registry,
        make: impl FnOnce(Reply<T>) -> Call,
    ) -> Result<T, String> {
        self.settled(make, |call| registry.admit(self, call)).await
    }
    /// Registers before submission and returns only the original lifecycle's result.
    pub(crate) async fn submit<T: Send + 'static>(
        &self,
        make: impl FnOnce(Reply<T>) -> Call,
    ) -> Result<T, String> {
        self.settled(make, |call| self.send(call)).await
    }
    /// Registers the reply before the call leaves, then awaits the one lifecycle result.
    async fn settled<T: Send + 'static>(
        &self,
        make: impl FnOnce(Reply<T>) -> Call,
        send: impl FnOnce(Call) -> Result<(), String>,
    ) -> Result<T, String> {
        let (reply, result) = self.replies.reply()?;
        send(make(reply))?;
        result
            .await
            .map_err(|_| "extension guest reply ended".to_owned())?
    }
}
