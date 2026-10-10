//! Preparation and asynchronous execution through host imports.
use super::exports::encode_value;
use super::{Exports, Imports, callbacks, contexts};
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::{AgentToolUpdateCallback, ExtensionResult, PrepareArguments, ToolExecute};
use std::rc::Rc;
/// Owned resources delivered to one tool invocation.
pub(crate) struct ToolCapabilities<I: Imports> {
    /// Ordinary context.
    pub(crate) ctx: I::Context,
    /// Optional cancellation resource.
    pub(crate) signal: Option<I::Signal>,
    /// Optional progress resource.
    pub(crate) update: Option<I::Update>,
}
impl<I: Imports> Exports<I> {
    /// Transforms arguments synchronously using the registered preparation.
    pub(crate) fn invoke_prepare(handler: u32, args: &str) -> ExtensionResult<String> {
        let prepare = callbacks::find::<PrepareArguments>(handler)?;
        let args = serde_json::from_str(args).map_err(|error| error.to_string())?;
        encode_value(&prepare(args)?)
    }
    /// Awaits the registered execution with owned resource facades.
    pub(crate) async fn invoke_tool(
        &self,
        handler: u32,
        invocation: ToolInvocation,
        resources: ToolCapabilities<I>,
    ) -> ExtensionResult<String> {
        let execute = callbacks::find::<ToolExecute>(handler)?;
        let params = serde_json::from_str(&invocation.params).map_err(|error| error.to_string())?;
        let signal = resources
            .signal
            .map(|signal| contexts::signal(&self.imports, signal));
        let update = resources.update.map(|update| {
            let imports = self.imports.clone();
            Rc::new(move |partial| imports.tool_update(&update, &encode_value(&partial)?))
                as AgentToolUpdateCallback
        });
        let result = execute(
            invocation.tool_call_id,
            params,
            signal,
            update,
            contexts::context(&self.imports, resources.ctx),
        )
        .await?;
        encode_value(&result)
    }
}
