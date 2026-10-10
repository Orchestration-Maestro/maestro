//! Drive tool progress futures alongside execution without spawning tasks.
use super::Driver;
use crate::{
    AgentEvent, AgentToolResult, AgentToolUpdateCallback, CustomAgentMessages, ExecuteTool,
};
use futures_util::{StreamExt, future::join};
use maestro_models::{BoxFuture, DiagnosticErrorInfo, EventStream, ToolCall};
use std::sync::Arc;

impl<C: CustomAgentMessages + 'static> Driver<'_, C> {
    /// Close progress admission at execution completion and settle admitted effects.
    pub(super) async fn execute(
        &self,
        call: &ToolCall,
        execute: ExecuteTool,
        arguments: serde_json::Value,
    ) -> Result<Result<AgentToolResult, DiagnosticErrorInfo>, DiagnosticErrorInfo> {
        let updates = EventStream::new(
            |_: &BoxFuture<Result<(), DiagnosticErrorInfo>>| false,
            |_| (),
        );
        let admitted = updates.clone();
        let emit = Arc::clone(&self.emit);
        let update_call = call.clone();
        let callback: AgentToolUpdateCallback = Arc::new(move |partial_result| {
            admitted.push(emit(AgentEvent::ToolExecutionUpdate {
                tool_call: update_call.clone(),
                partial_result,
            }));
        });
        let execution = execute(
            call.id.clone(),
            arguments,
            self.options.signal.clone(),
            Some(callback),
        );
        let progress = futures_util::stream::unfold(updates.clone(), |updates| async {
            updates.next().await.map(|future| (future, updates))
        })
        .map(futures_util::stream::once)
        .flatten_unordered(None)
        .collect::<Vec<_>>();
        let (result, progress) = join(
            async {
                let result = execution.await;
                updates.end(Some(()));
                result
            },
            progress,
        )
        .await;
        for settled in progress {
            settled?;
        }
        Ok(result)
    }
}
