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
            Self::Component(call) => attach_component(call, completion),
            Self::Synchronous(call) => attach(&call.completion, completion),
            Self::TerminalInput(call) => attach(&call.reply, completion),
            Self::Factory(call) => attach(&call.reply, completion),
            Self::Custom(call) => attach_custom(call, completion),
            Self::ToolRender(call) => attach(&call.reply, completion),
            Self::Widget(call) => attach(&call.reply, completion),
            Self::EditorFactory(call) => attach(&call.reply, completion),
            Self::Editor(call) => attach(&call.reply, completion),
            Self::AutocompleteFactory(call) => attach(&call.reply, completion),
            Self::Autocomplete(call) => attach(&call.reply, completion),
            Self::Paint(crate::guest_calls::paints::PaintCall::Highlight(_, _, _, reply)) => {
                attach(reply, completion);
            }
            Self::Stream(call) => attach(&call.reply, completion),
            Self::OAuth(call) => attach(&call.reply, completion),
            Self::Provider(call) => attach(&call.reply, completion),
            Self::Assistant(call) => attach(&call.reply, completion),
            Self::ModelHook(call) => attach(&call.reply, completion),
            Self::Schema(call) => attach(&call.reply, completion),
            Self::Credential(call) => match call {
                crate::guest_calls::credentials::CredentialCall::Backend(call) => {
                    attach(&call.reply, completion);
                }
                crate::guest_calls::credentials::CredentialCall::Fallback(call) => {
                    attach(&call.reply, completion);
                }
            },
            Self::Event(call) => attach(&call.reply, completion),
            Self::Bash(call) => attach(&call.reply, completion),
            Self::Read(call) => attach(&call.reply, completion),
            Self::Write(call) => attach(&call.reply, completion),
            Self::Edit(call) => attach(&call.reply, completion),
            Self::Find(call) => attach(&call.reply, completion),
            Self::Ls(call) => attach(&call.reply, completion),
            Self::Grep(call) => attach(&call.reply, completion),
            Self::Registered(call) => match call {
                RegisteredCall::Command(_, _, _, reply) | RegisteredCall::Shortcut(_, _, reply) => {
                    attach(reply, completion);
                }
                RegisteredCall::Completion(_, _, reply) => attach(reply, completion),
            },
            Self::Tool(ToolCall::Prepare { reply, .. }) => attach(reply, completion),
            Self::Tool(ToolCall::Execute { reply, .. })
            | Self::Paint(crate::guest_calls::paints::PaintCall::Style(_, _, reply)) => {
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

/// Attaches completion to a custom factory prefix's or continuation's reply.
fn attach_custom(call: &crate::guest_calls::custom::Invocation, completion: &Completion) {
    match call {
        crate::guest_calls::custom::Invocation::Prefix(call) => attach(&call.reply, completion),
        crate::guest_calls::custom::Invocation::Future(_, reply) => attach(reply, completion),
    }
}

/// Retains only a started reply's once-only completion action.
fn attach<T: Send + 'static>(reply: &super::replies::Reply<T>, completion: &Completion) {
    let completion = completion.clone();
    reply.before(move || completion.finish());
}

/// Attaches completion to the original retained component reply.
fn attach_component(call: &ComponentCall, completion: &Completion) {
    match call {
        ComponentCall::Render(_, _, _, reply) => attach(reply, completion),
        ComponentCall::Unit(_, _, _, reply) => attach(reply, completion),
        ComponentCall::WantsKeyRelease(_, _, reply) => attach(reply, completion),
        ComponentCall::Focused(_, _, reply) => attach(reply, completion),
    }
}
