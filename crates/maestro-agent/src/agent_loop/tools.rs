//! Tool preparation, concurrent finalization and ordered artifacts.
use super::{Driver, failure, read};
use crate::types::AgentExecution;
use crate::{AgentEvent, AgentMessage, AgentToolResult, CustomAgentMessages, SharedAgentTool};
use maestro_models::{
    AssistantContent, DiagnosticErrorInfo, SharedAssistantMessage, ToolCall, ToolResultMessage,
    timestamp_now,
};
use std::sync::{Arc, RwLock};

/// The chosen inventory record retained through execution and progress settlement.
struct Prepared<E: AgentExecution> {
    /// The captured tool entry, independent of later inventory edits.
    tool: SharedAgentTool<E>,
    /// The prepared and validated arguments.
    arguments: serde_json::Value,
}

impl<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>> Driver<'_, C, E> {
    /// Complete a selected batch with ordered artifacts.
    pub(super) async fn tools(
        &self,
        response: &SharedAssistantMessage,
    ) -> Result<(Vec<Arc<RwLock<ToolResultMessage>>>, bool), DiagnosticErrorInfo> {
        let calls: Vec<_> = read(response)
            .content
            .iter()
            .enumerate()
            .filter_map(|(index, part)| match part {
                AssistantContent::ToolCall(call) => Some((index, call.clone())),
                _ => None,
            })
            .collect();
        let sequential = self.config.tool_execution == Some(crate::ToolExecutionMode::Sequential)
            || calls.iter().any(|(_, call)| {
                self.lookup(&call.name).is_some_and(|tool| {
                    read(&tool).execution_mode == Some(crate::ToolExecutionMode::Sequential)
                })
            });
        if sequential {
            self.sequential_tools(response, calls).await
        } else {
            self.parallel_tools(response, calls).await
        }
    }
    /// Complete each call and artifact before starting the next.
    async fn sequential_tools(
        &self,
        response: &SharedAssistantMessage,
        calls: Vec<(usize, ToolCall)>,
    ) -> Result<(Vec<Arc<RwLock<ToolResultMessage>>>, bool), DiagnosticErrorInfo> {
        let mut results = Vec::new();
        let mut all_terminate = !calls.is_empty();
        for (index, mut call) in calls {
            let preparation = self.preflight(response, (index, &mut call)).await?;
            let (result, is_error) = self.invoke(response, &call, preparation).await?;
            all_terminate &= result.terminate == Some(true);
            let message = self.execution_end(call, result, is_error).await?;
            results.push(self.artifact(message).await?);
        }
        let again = !results.is_empty() && !all_terminate;
        Ok((results, again))
    }
    /// Preflight in source order, settle admitted finalizers, then emit artifacts.
    async fn parallel_tools(
        &self,
        response: &SharedAssistantMessage,
        calls: Vec<(usize, ToolCall)>,
    ) -> Result<(Vec<Arc<RwLock<ToolResultMessage>>>, bool), DiagnosticErrorInfo> {
        use futures_util::{StreamExt, stream::FuturesUnordered};
        let mut ready = Vec::new();
        let mut allowed = Vec::new();
        let mut all_terminate = !calls.is_empty();
        for (index, (position, mut call)) in calls.into_iter().enumerate() {
            let preparation = self.preflight(response, (position, &mut call)).await?;
            match preparation {
                Ok(prepared) => {
                    allowed.push((index, call, prepared));
                    ready.push(None);
                }
                Err(error) => {
                    all_terminate = false;
                    ready.push(Some(
                        self.execution_end(call, error_result(error), true).await?,
                    ));
                }
            }
        }
        let mut work: FuturesUnordered<_> = allowed
            .into_iter()
            .map(|(index, call, prepared)| {
                let execute = read(&prepared.tool).execute.clone();
                let execution = self.execute(&call, &execute, prepared.arguments.clone());
                async move {
                    let (result, is_error) = self
                        .finalize_execution(response, &call, prepared, execution)
                        .await?;
                    let terminate = result.terminate == Some(true);
                    let message = self.execution_end(call, result, is_error).await?;
                    Ok::<_, DiagnosticErrorInfo>((index, message, terminate))
                }
            })
            .collect();
        let mut failure = None;
        while let Some(outcome) = work.next().await {
            match outcome {
                Ok((index, message, terminate)) => {
                    ready[index] = Some(message);
                    all_terminate &= terminate;
                }
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let mut results = Vec::new();
        for message in ready.into_iter().flatten() {
            results.push(self.artifact(message).await?);
        }
        let again = !results.is_empty() && !all_terminate;
        Ok((results, again))
    }
    /// Await the start observation before fallible preparation and hooks.
    async fn preflight(
        &self,
        response: &SharedAssistantMessage,
        (index, call): (usize, &mut ToolCall),
    ) -> Result<Result<Prepared<E>, DiagnosticErrorInfo>, DiagnosticErrorInfo> {
        (self.emit)(AgentEvent::ToolExecutionStart {
            tool_call: call.clone(),
        })
        .await?;
        Ok(self.prepare_with_hook(response, (index, call)).await)
    }
    /// Validate prepared arguments before exposing mutable arguments to the hook.
    async fn prepare_with_hook(
        &self,
        response: &SharedAssistantMessage,
        (index, call): (usize, &mut ToolCall),
    ) -> Result<Prepared<E>, DiagnosticErrorInfo> {
        let mut prepared = self.prepare(response, (index, call))?;
        if let Some(before) = &self.config.before_tool_call
            && let Some(result) = before(
                crate::BeforeToolCallContext {
                    assistant_message: response,
                    tool_call: call,
                    args: &mut prepared.arguments,
                    context: &self.context,
                },
                self.options.signal.clone(),
            )
            .await?
            && result.block
        {
            return Err(failure(
                result
                    .reason
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Tool execution was blocked".into()),
            ));
        }
        Ok(prepared)
    }
    /// Retain the first exactly named tool from the current inventory.
    fn lookup(&self, name: &str) -> Option<SharedAgentTool<E>> {
        self.context.tools.as_ref().and_then(|tools| {
            read(tools)
                .iter()
                .find(|tool| read(tool).definition.name == name)
                .cloned()
        })
    }
    /// Execute the prepared entry, retaining arguments for finalization.
    async fn invoke(
        &self,
        response: &SharedAssistantMessage,
        call: &ToolCall,
        prepared: Result<Prepared<E>, DiagnosticErrorInfo>,
    ) -> Result<(AgentToolResult, bool), DiagnosticErrorInfo> {
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return Ok((error_result(error), true)),
        };
        let execute = read(&prepared.tool).execute.clone();
        let execution = self.execute(call, &execute, prepared.arguments.clone());
        self.finalize_execution(response, call, prepared, execution)
            .await
    }
    /// Settle an admitted execution before observing its after hook.
    async fn finalize_execution(
        &self,
        response: &SharedAssistantMessage,
        call: &ToolCall,
        prepared: Prepared<E>,
        execution: impl std::future::Future<
            Output = Result<Result<AgentToolResult, DiagnosticErrorInfo>, DiagnosticErrorInfo>,
        >,
    ) -> Result<(AgentToolResult, bool), DiagnosticErrorInfo> {
        let result = execution.await?;
        let (result, is_error) = match result {
            Ok(result) => (result, false),
            Err(error) => (error_result(error), true),
        };
        let result = self
            .finalize(response, call, &prepared.arguments, (result, is_error))
            .await;
        drop(prepared.tool);
        Ok(result)
    }
    /// Apply whole-field replacements or convert an after-hook failure.
    async fn finalize(
        &self,
        response: &SharedAssistantMessage,
        call: &ToolCall,
        args: &serde_json::Value,
        executed: (AgentToolResult, bool),
    ) -> (AgentToolResult, bool) {
        let (mut result, mut is_error) = executed;
        if let Some(after) = &self.config.after_tool_call {
            match after(
                crate::AfterToolCallContext {
                    assistant_message: response,
                    tool_call: call,
                    args,
                    result: &result,
                    is_error,
                    context: &self.context,
                },
                self.options.signal.clone(),
            )
            .await
            {
                Ok(Some(overrides)) => apply_overrides(&mut result, &mut is_error, overrides),
                Ok(None) => (),
                Err(error) => return (error_result(error), true),
            }
        }
        (result, is_error)
    }
    /// Apply optional preparation, then delegate schema validation.
    fn prepare(
        &self,
        response: &SharedAssistantMessage,
        (index, call): (usize, &mut ToolCall),
    ) -> Result<Prepared<E>, DiagnosticErrorInfo> {
        let tool = self
            .lookup(&call.name)
            .ok_or_else(|| failure(format!("Tool {} not found", call.name)))?;
        let prepare = read(&tool).prepare_arguments.clone();
        let candidate = match prepare {
            Some(prepare) => {
                let mut raw = call.arguments.clone();
                let prepared = prepare(&mut raw);
                call.arguments = raw;
                if let Some(AssistantContent::ToolCall(retained)) =
                    super::write(response).content.get_mut(index)
                {
                    retained.arguments.clone_from(&call.arguments);
                }
                let prepared = prepared?;
                Some(ToolCall {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    arguments: prepared,
                    thought_signature: call.thought_signature.clone(),
                })
            }
            None => None,
        };
        let definition = read(&tool).definition.clone();
        let arguments = maestro_models::validate_tool_arguments(
            &definition,
            candidate.as_ref().unwrap_or(call),
        )?;
        Ok(Prepared { tool, arguments })
    }
    /// Await execution-end before assigning the tool artifact timestamp.
    async fn execution_end(
        &self,
        call: ToolCall,
        result: AgentToolResult,
        is_error: bool,
    ) -> Result<ToolResultMessage, DiagnosticErrorInfo> {
        let message = ToolResultMessage {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: result.content.clone(),
            details: result.details.clone(),
            is_error,
            timestamp: 0.0,
            usage: None,
        };
        (self.emit)(AgentEvent::ToolExecutionEnd {
            tool_call: call,
            result,
            is_error,
        })
        .await?;
        Ok(message)
    }
    /// Timestamp and observe a shared tool artifact before history insertion.
    async fn artifact(
        &self,
        mut message: ToolResultMessage,
    ) -> Result<Arc<RwLock<ToolResultMessage>>, DiagnosticErrorInfo> {
        message.timestamp = timestamp_now();
        let shared = Arc::new(RwLock::new(message));
        (self.emit)(AgentEvent::MessageStart {
            message: AgentMessage::ToolResult(Arc::clone(&shared)),
        })
        .await?;
        (self.emit)(AgentEvent::MessageEnd {
            message: AgentMessage::ToolResult(Arc::clone(&shared)),
        })
        .await?;
        Ok(shared)
    }
}
/// Construct the model-facing artifact from a preparation or execution failure.
fn error_result(error: DiagnosticErrorInfo) -> AgentToolResult {
    AgentToolResult::text_result(error.message, Some(serde_json::json!({})))
}

/// Replace supplied nonnull fields without merging details or discarding false flags.
fn apply_overrides(
    result: &mut AgentToolResult,
    is_error: &mut bool,
    overrides: crate::AfterToolCallResult,
) {
    if let Some(content) = overrides.content {
        result.content = content;
    }
    if let Some(details) = overrides.details.filter(|value| !value.is_null()) {
        result.details = Some(details);
    }
    if let Some(terminate) = overrides.terminate {
        result.terminate = Some(terminate);
    }
    if let Some(error) = overrides.is_error {
        *is_error = error;
    }
}
