//! Retained callables with invocation-associated return types.
use super::{
    AssistantMessageEventStream, Context, DiagnosticErrorInfo, Local, Model, ProviderExecution,
    ProviderSpawner, ProviderStreamOptions, SimpleStreamOptions, StreamOptionsInput, Threaded,
};

/// One callable body whose return follows its invocation family.
pub trait Invoke<E: ProviderExecution, I> {
    /// Result selected by this invocation.
    type Return;
    /// Invoke with the original typed arguments.
    fn invoke(&self, input: I) -> Self::Return;
}
/// Native registry owners remain thread-safe even when handling local arguments.
#[cfg(not(target_arch = "wasm32"))]
pub trait RegistryCapture: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> RegistryCapture for T {}
/// Browser registry owners may retain single-thread objects.
#[cfg(target_arch = "wasm32")]
pub trait RegistryCapture {}
#[cfg(target_arch = "wasm32")]
impl<T> RegistryCapture for T {}

/// One existing options schema projected into either invocation family.
pub trait StreamOptionsFamily: 'static {
    /// Original options for the selected family.
    type Options<E: ProviderExecution>;
}
impl StreamOptionsFamily for ProviderStreamOptions {
    type Options<E: ProviderExecution> = ProviderStreamOptions<E>;
}
impl StreamOptionsFamily for SimpleStreamOptions {
    type Options<E: ProviderExecution> = SimpleStreamOptions<E>;
}
impl StreamOptionsFamily for StreamOptionsInput {
    type Options<E: ProviderExecution> = StreamOptionsInput<E>;
}
/// Original inputs plus family-correct admission, consumed synchronously.
pub struct ProviderCall<E: ProviderExecution, O> {
    /// Requested model.
    pub model: Model,
    /// Conversation input.
    pub context: Context,
    /// Absent or supplied original options.
    pub options: Option<O>,
    /// Producer admission.
    pub submit: E::Submission,
}
/// One typed envelope shared by raw, simple and catalog dispatch.
pub enum ProviderInvocation<K: StreamOptionsFamily> {
    /// Local requests always carry their caller-owned admission.
    Local(Option<K::Options<Local>>, ProviderSpawner<Local>),
    /// Threaded requests may retain the provider's existing default admission.
    Threaded(
        Option<K::Options<Threaded>>,
        Option<ProviderSpawner<Threaded>>,
    ),
}
/// Provider setup result, independent of the selected execution family.
type Output = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
/// Retained provider owner callable in either execution family.
pub trait StreamCallback<K: StreamOptionsFamily>:
    Invoke<Local, ProviderCall<Local, K::Options<Local>>, Return = Output>
    + Invoke<Threaded, ProviderCall<Threaded, K::Options<Threaded>>, Return = Output>
    + RegistryCapture
{
    /// Invoke synchronously, preserving the original options and submission.
    ///
    /// # Errors
    /// Returns the provider's setup failure unchanged.
    fn call(&self, model: Model, context: Context, invocation: ProviderInvocation<K>) -> Output {
        match invocation {
            ProviderInvocation::Local(options, submit) => Invoke::<Local, _>::invoke(
                self,
                ProviderCall {
                    model,
                    context,
                    options,
                    submit,
                },
            ),
            ProviderInvocation::Threaded(options, submit) => Invoke::<Threaded, _>::invoke(
                self,
                ProviderCall {
                    model,
                    context,
                    options,
                    submit,
                },
            ),
        }
    }
}
impl<K: StreamOptionsFamily, T> StreamCallback<K> for T where
    T: Invoke<Local, ProviderCall<Local, K::Options<Local>>, Return = Output>
        + Invoke<Threaded, ProviderCall<Threaded, K::Options<Threaded>>, Return = Output>
        + RegistryCapture
{
}
