//! Retained provider setup and returned-stream operations share the Store lifecycle.
use crate::{
    guest_calls::Handle,
    lifecycle::replies::Reply,
    local_resources::{
        Message, Registry, SignalEndpoint,
        provider_controls::{Command, Creation, Owner},
    },
};
use maestro_extensions::{
    AssistantMessageEventStream, Context, DiagnosticErrorInfo, Invoke, Model, ProviderCall,
    ProviderExecution, ProviderInvocation, ProviderSpawner, StreamOptionsInput, Threaded,
};
use wasmtime::component::{ComponentType, Lift, Lower, Resource};

/// Native controls lent to a registered provider independently of setup's stack.
#[derive(Clone)]
pub(crate) struct Input {
    /// Original native options owner.
    pub(crate) controls: crate::local_resources::provider_controls::Endpoint,
    /// Original caller cancellation.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Whether a payload selector was supplied.
    pub(crate) payload: bool,
    /// Whether a response observer was supplied.
    pub(crate) response: bool,
}
/// Canonical native capabilities, never encoded into ordinary option data.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct WireInput {
    /// Original options owner.
    pub(crate) controls: Resource<crate::local_resources::provider_controls::Endpoint>,
    /// Original cancellation.
    pub(crate) signal: Option<Resource<SignalEndpoint>>,
    /// Supplied payload-hook presence.
    pub(crate) payload: bool,
    /// Supplied response-hook presence.
    pub(crate) response: bool,
}
impl Input {
    /// Transfer independent leases before entering the author.
    pub(crate) fn lower(
        &self,
        table: &mut wasmtime::component::ResourceTable,
    ) -> wasmtime::Result<WireInput> {
        Ok(WireInput {
            controls: table.push(self.controls.clone())?,
            signal: self
                .signal
                .clone()
                .map(|signal| table.push(signal))
                .transpose()?,
            payload: self.payload,
            response: self.response,
        })
    }
}
/// Synchronous setup invocation on the original registered callback.
pub(crate) struct Invocation<H = Handle> {
    /// Original callback and its admission.
    pub(crate) handler: H,
    /// Model, conversation and original option variant.
    pub(crate) data: String,
    /// Independently retained native capabilities.
    pub(crate) input: Input,
    /// Returns the original authored stream resource.
    pub(crate) reply: Reply<Handle>,
}
/// One operation on the original returned assistant stream.
#[derive(Clone, Copy, ComponentType, Lift, Lower)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Operation {
    /// Admit an event.
    #[component(name = "push")]
    Push,
    /// End with an optional result.
    #[component(name = "end")]
    End,
    /// Observe a competing reader.
    #[component(name = "next")]
    Next,
    /// Observe the independent result.
    #[component(name = "result")]
    Result,
}
/// Lifecycle-owned stream operation and its ordinary data.
pub(crate) struct Assistant<H = Handle> {
    /// Original returned authored stream.
    pub(crate) handler: H,
    /// Exact operation, without a forwarding queue.
    pub(crate) operation: Operation,
    /// Ordinary event or result encoding.
    pub(crate) data: Option<String>,
    /// Only the native observation may cancel this read or result waiter.
    pub(crate) signal: Option<SignalEndpoint>,
    /// One original completion.
    pub(crate) reply: Reply<Option<String>>,
}
/// Nested setup uses the same author body as its canonical export.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct LocalProvider {
    /// Original callback identity.
    pub(crate) target: u32,
    /// Ordinary setup data.
    pub(crate) data: String,
    /// Original retained native controls.
    pub(crate) input: WireInput,
}
/// Nested stream operation over the same retained resource.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct LocalAssistant {
    /// Original stream identity.
    pub(crate) target: u32,
    /// Exact operation.
    pub(crate) operation: Operation,
    /// Ordinary event or result data.
    pub(crate) data: Option<String>,
    /// Operation-scoped cancellation, unrelated to provider cancellation.
    pub(crate) signal: Option<Resource<SignalEndpoint>>,
}
/// Store-safe provider registration, retaining no catalog or registration facade.
pub(crate) struct Provider {
    /// Retained guest callback.
    pub(crate) handler: Handle,
    /// Original pump admission, with no capability-to-catalog cycle.
    pub(crate) requests: tokio::sync::mpsc::UnboundedSender<Message>,
}
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Provider
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let invocation =
            E::submission::<StreamOptionsInput>(call.options.map(Into::into), call.submit);
        let result = match invocation {
            ProviderInvocation::Local(options, submit) => {
                Registry::on_pump(&self.requests, |registry| {
                    registry.provider_start(
                        &self.handler,
                        call.model,
                        &call.context,
                        Owner::Local(options, submit),
                    )
                })
                .map_err(|_| {
                    "local guest-provider invocation requires the extension's original pump thread"
                        .to_owned()
                })
                .and_then(std::convert::identity)
            }
            ProviderInvocation::Threaded(options, submit) => {
                self.threaded(call.model, call.context, options, submit)
            }
        };
        result.map_err(DiagnosticErrorInfo::error)
    }
}
impl Provider {
    /// Thread-safe callers can reach their original pump without moving local captures.
    fn threaded(
        &self,
        model: Model,
        context: Context,
        options: Option<StreamOptionsInput>,
        submit: Option<ProviderSpawner<Threaded>>,
    ) -> Result<AssistantMessageEventStream, String> {
        let mut operands = Some((model, context, options, submit));
        let local = Registry::on_pump(&self.requests, |registry| {
            operands.take().map(|(model, context, options, submit)| {
                registry.provider_start(
                    &self.handler,
                    model,
                    &context,
                    Owner::Threaded(options, submit),
                )
            })
        });
        if let Ok(Some(result)) = local {
            return result;
        }
        let (model, context, options, submit) =
            operands.ok_or("extension provider operands ended")?;
        let (reply, result) = std::sync::mpsc::channel();
        self.requests
            .send(Message::Provider(Command::Start(Box::new(Creation {
                handler: self.handler.clone(),
                model,
                context,
                options,
                submit,
                reply,
            }))))
            .map_err(|_| "extension resource owner ended")?;
        result
            .recv()
            .map_err(|_| "extension resource reply ended")?
    }
}
