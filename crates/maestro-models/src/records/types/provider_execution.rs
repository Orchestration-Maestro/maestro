//! Family-correct request hooks and producer submission.
use super::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, Execution, Local, Model, PayloadHookInput,
    ProviderInvocation, ProviderResponse, StreamOptionSlots, StreamOptionsFamily, Threaded,
};
use crate::providers::http::Fetch;
use std::sync::Arc;

/// Request-hook futures and submission associated with an execution family.
pub trait ProviderExecution:
    Execution
    + Clone
    + Default
    + StreamOptionSlots<
        Signal = Cancellation,
        PayloadHook = OnPayload<Self>,
        ResponseHook = OnResponse<Self>,
        Fetch = Fetch,
    >
{
    /// Payload selector retained for this invocation.
    type Payload: Fn(
            PayloadHookInput,
            Arc<Model>,
        ) -> Self::Future<'static, Result<PayloadHookInput, DiagnosticErrorInfo>>
        + ?Sized;
    /// Response observer retained for this invocation.
    type Response: Fn(ProviderResponse, Arc<Model>) -> Self::Future<'static, Result<(), DiagnosticErrorInfo>>
        + ?Sized;
    /// Caller-owned producer admission.
    type Spawner: Fn(Self::Future<'static, ()>) -> bool + ?Sized;
    /// Mandatory local admission or optional threaded override.
    type Submission: Clone;
    /// Retain supplied admission in this family's representation.
    fn provided(spawn: ProviderSpawner<Self>) -> Self::Submission;
    /// Package original options with their producer admission.
    fn submission<K: StreamOptionsFamily>(
        options: Option<K::Options<Self>>,
        submission: Self::Submission,
    ) -> ProviderInvocation<K>;
    /// Submit once using supplied admission or the provider's existing default.
    ///
    /// # Errors
    /// Returns the admission failure without retrying or installing a runtime.
    fn submit(
        task: Self::Future<'static, ()>,
        submission: Self::Submission,
        fallback: fn(BoxFuture<()>) -> Result<(), DiagnosticErrorInfo>,
    ) -> Result<(), DiagnosticErrorInfo>;
}
/// Retained payload selector for an invocation family.
pub type OnPayload<E = Threaded> = <E as Execution>::Shared<<E as ProviderExecution>::Payload>;
/// Retained response observer for an invocation family.
pub type OnResponse<E = Threaded> = <E as Execution>::Shared<<E as ProviderExecution>::Response>;
/// Caller-owned producer admission for an invocation family.
pub type ProviderSpawner<E> = <E as Execution>::Shared<<E as ProviderExecution>::Spawner>;

/// Preserve the existing producer-admission failure text.
pub(crate) fn admitted(accepted: bool) -> Result<(), DiagnosticErrorInfo> {
    if accepted {
        Ok(())
    } else {
        Err(DiagnosticErrorInfo::error(
            "Streaming requires a running Tokio runtime.",
        ))
    }
}

/// Concrete hook pointer and future erasures; no invocation policy is duplicated.
macro_rules! hooks {
    ([$($bound:tt)*]) => {
        type Payload = dyn Fn(PayloadHookInput, Arc<Model>) -> Self::Future<'static, Result<PayloadHookInput, DiagnosticErrorInfo>> $($bound)*;
        type Response = dyn Fn(ProviderResponse, Arc<Model>) -> Self::Future<'static, Result<(), DiagnosticErrorInfo>> $($bound)*;
        type Spawner = dyn Fn(Self::Future<'static, ()>) -> bool $($bound)*;
    };
}
impl ProviderExecution for Local {
    hooks!([]);
    type Submission = ProviderSpawner<Self>;
    fn provided(spawn: ProviderSpawner<Self>) -> Self::Submission {
        spawn
    }
    fn submission<K: StreamOptionsFamily>(
        options: Option<K::Options<Self>>,
        submission: Self::Submission,
    ) -> ProviderInvocation<K> {
        ProviderInvocation::Local(options, submission)
    }
    fn submit(
        task: Self::Future<'static, ()>,
        submission: Self::Submission,
        _fallback: fn(BoxFuture<()>) -> Result<(), DiagnosticErrorInfo>,
    ) -> Result<(), DiagnosticErrorInfo> {
        admitted(submission(task))
    }
}
impl ProviderExecution for Threaded {
    #[cfg(not(target_arch = "wasm32"))]
    hooks!([+ Send + Sync]);
    #[cfg(target_arch = "wasm32")]
    hooks!([]);
    type Submission = Option<ProviderSpawner<Self>>;
    fn provided(spawn: ProviderSpawner<Self>) -> Self::Submission {
        Some(spawn)
    }
    fn submission<K: StreamOptionsFamily>(
        options: Option<K::Options<Self>>,
        submission: Self::Submission,
    ) -> ProviderInvocation<K> {
        ProviderInvocation::Threaded(options, submission)
    }
    fn submit(
        task: Self::Future<'static, ()>,
        submission: Self::Submission,
        fallback: fn(BoxFuture<()>) -> Result<(), DiagnosticErrorInfo>,
    ) -> Result<(), DiagnosticErrorInfo> {
        match submission {
            Some(spawn) => admitted(spawn(task)),
            None => fallback(task),
        }
    }
}

/// Native options retain each family's existing hook pointers and transport.
macro_rules! slots {
    ($family:ty) => {
        impl StreamOptionSlots for $family {
            type Signal = Cancellation;
            type PayloadHook = OnPayload<Self>;
            type ResponseHook = OnResponse<Self>;
            type Fetch = Fetch;
        }
    };
}
slots!(Local);
slots!(Threaded);
