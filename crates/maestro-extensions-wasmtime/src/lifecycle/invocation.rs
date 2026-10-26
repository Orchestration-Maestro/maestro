//! Every guest entry shares the Store's completion and fault owner.
use super::completion::Completion;
use crate::{
    guest_calls::{
        Call, Callback, registrations::RegisteredCall, renderers::ComponentCall, tools::ToolCall,
    },
    guest_exports::Exports,
};
use std::sync::Arc;
use wasmtime::component::Accessor;

/// An admitted export call, never a native request poller.
pub(super) struct Invocation {
    /// Original canonical exports from this instance.
    pub(super) exports: Arc<Exports>,
    /// Actual operands and lifecycle-owned reply token.
    pub(super) call: Call<Callback>,
    /// Marks completion even when an export escapes before settlement.
    pub(super) completion: Completion,
}
impl Invocation {
    /// Runs the admitted call before publishing its completion.
    pub(super) async fn run(self, accessor: &Accessor<super::State>) -> wasmtime::Result<()> {
        self.exports.invoke(accessor, self.call).await?;
        self.completion.finish();
        Ok(())
    }
}

impl Call {
    /// Attaches completion to every reply before the driver starts guest work.
    pub(crate) fn before(&self, completion: &Completion) {
        match self {
            Self::Start(reply)
            | Self::Setup(_, _, reply)
            | Self::WithSession(_, _, reply)
            | Self::Tail(_, reply) => attach(reply, completion),
            Self::Renderer(call) => attach(&call.reply, completion),
            Self::Component(call) => match call {
                ComponentCall::Render(_, _, _, reply) => attach(reply, completion),
                ComponentCall::Unit(_, _, _, reply) => attach(reply, completion),
                ComponentCall::WantsKeyRelease(_, _, reply) => attach(reply, completion),
            },
            Self::Synchronous(call) => attach(&call.completion, completion),
            Self::TerminalInput(call) => attach(&call.reply, completion),
            Self::Schema(call) => attach(&call.reply, completion),
            Self::Event(call) => attach(&call.reply, completion),
            Self::Bash(call) => attach(&call.reply, completion),
            Self::Registered(call) => match call {
                RegisteredCall::Command(_, _, _, reply) | RegisteredCall::Shortcut(_, _, reply) => {
                    attach(reply, completion);
                }
                RegisteredCall::Completion(_, _, reply) => attach(reply, completion),
            },
            Self::Tool(ToolCall::Prepare { reply, .. } | ToolCall::Execute { reply, .. }) => {
                attach(reply, completion);
            }
            Self::Release(release) => {
                if let Some(reply) = &release.completion {
                    attach(reply, completion);
                }
            }
            Self::Finished => {}
        }
    }
}

/// Retains only a started reply's once-only completion action.
fn attach<T: Send + 'static>(reply: &super::replies::Reply<T>, completion: &Completion) {
    let completion = completion.clone();
    reply.before(move || completion.finish());
}
