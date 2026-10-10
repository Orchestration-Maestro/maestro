//! Sequential tool preparation, execution and artifacts.
use super::{Driver, failure, read};
use crate::{AgentEvent, AgentMessage, AgentToolResult, CustomAgentMessages, SharedAgentTool};
use maestro_models::{
    AssistantContent, DiagnosticErrorInfo, SharedAssistantMessage, TextContent, ToolCall,
    ToolResultMessage, UserBlock, timestamp_now,
};
use std::sync::{Arc, RwLock};

impl<C: CustomAgentMessages + 'static> Driver<'_, C> {
    /// Complete calls in content order before exposing the batch to history.
    pub(super) async fn tools(
        &self,
        response: &SharedAssistantMessage,
    ) -> Result<(Vec<Arc<RwLock<ToolResultMessage>>>, bool), DiagnosticErrorInfo> {
        let calls: Vec<_> = read(response)
            .content
            .iter()
            .filter_map(|part| match part {
                AssistantContent::ToolCall(call) => Some(call.clone()),
                _ => None,
            })
            .collect();
        let mut results = Vec::new();
        let mut all_terminate = !calls.is_empty();
        for call in calls {
            (self.emit)(AgentEvent::ToolExecutionStart {
                tool_call: call.clone(),
            })
            .await?;
            let result = self.invoke(&call).await?;
            let (result, is_error) = match result {
                Ok(result) => (result, false),
                Err(error) => (error_result(error), true),
            };
            all_terminate &= result.terminate == Some(true);
            results.push(self.finish(call, result, is_error).await?);
        }
        let again = !results.is_empty() && !all_terminate;
        Ok((results, again))
    }
    /// Retain the first exactly named tool from the current inventory.
    fn lookup(&self, name: &str) -> Option<SharedAgentTool> {
        self.context.tools.as_ref().and_then(|tools| {
            read(tools)
                .iter()
                .find(|tool| read(tool).definition.name == name)
                .cloned()
        })
    }
    /// Prepare one candidate before executing it outside retained-state locks.
    async fn invoke(
        &self,
        call: &ToolCall,
    ) -> Result<Result<AgentToolResult, DiagnosticErrorInfo>, DiagnosticErrorInfo> {
        let (execute, arguments) = match self.prepare(call) {
            Ok(prepared) => prepared,
            Err(error) => return Ok(Err(error)),
        };
        self.execute(call, execute, arguments).await
    }
    /// Apply optional preparation, then delegate schema validation.
    fn prepare(
        &self,
        call: &ToolCall,
    ) -> Result<(crate::ExecuteTool, serde_json::Value), DiagnosticErrorInfo> {
        let tool = self
            .lookup(&call.name)
            .ok_or_else(|| failure(format!("Tool {} not found", call.name)))?;
        let prepare = read(&tool).prepare_arguments.clone();
        let mut candidate = call.clone();
        if let Some(prepare) = prepare {
            candidate.arguments =
                serde_json::from_value(prepare(serde_json::Value::Object(candidate.arguments))?)
                    .map_err(|error| failure(error.to_string()))?;
        }
        let arguments =
            maestro_models::validate_tool_arguments(&read(&tool).definition, &candidate)?;
        let execute = Arc::clone(&read(&tool).execute);
        Ok((execute, serde_json::Value::Object(arguments)))
    }
    /// Await execution-end before assigning the tool artifact timestamp.
    async fn finish(
        &self,
        call: ToolCall,
        result: AgentToolResult,
        is_error: bool,
    ) -> Result<Arc<RwLock<ToolResultMessage>>, DiagnosticErrorInfo> {
        let mut message = ToolResultMessage {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: result.content.clone(),
            details: Some(result.details.clone()),
            is_error,
            timestamp: 0.0,
        };
        (self.emit)(AgentEvent::ToolExecutionEnd {
            tool_call: call,
            result,
            is_error,
        })
        .await?;
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
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent {
            text: error.message,
            text_signature: None,
        })],
        details: serde_json::json!({}),
        terminate: None,
    }
}
