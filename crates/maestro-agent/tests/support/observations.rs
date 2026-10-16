//! Lossless public event projections for recorded scenarios.
use maestro_agent::{AgentEvent, AgentMessage, AgentToolResult, CustomAgentMessages};
use serde_json::{Value, json};

/// Project the event data in source field order.
pub fn event<C: CustomAgentMessages>(
    event: &AgentEvent<C>,
    message: fn(&AgentMessage<C>) -> Value,
) -> Value {
    match event {
        AgentEvent::AgentStart => json!({"type":"agent_start"}),
        AgentEvent::TurnStart => json!({"type":"turn_start"}),
        AgentEvent::AgentEnd { messages } => {
            json!({"type":"agent_end","messages":messages.iter().map(message).collect::<Vec<_>>()})
        }
        AgentEvent::TurnEnd {
            message,
            tool_results,
        } => {
            json!({"type":"turn_end","message":&*message.read().unwrap(),"toolResults":tool_results})
        }
        AgentEvent::MessageStart { message: entry } => {
            json!({"type":"message_start","message":message(entry)})
        }
        AgentEvent::MessageEnd { message: entry } => {
            json!({"type":"message_end","message":message(entry)})
        }
        AgentEvent::MessageUpdate {
            message,
            assistant_message_event,
        } => {
            json!({"type":"message_update","assistantMessageEvent":assistant_message_event,"message":&*message.read().unwrap()})
        }
        AgentEvent::ToolExecutionStart { tool_call } => {
            json!({"type":"tool_execution_start","toolCallId":tool_call.id,"toolName":tool_call.name,"args":tool_call.arguments})
        }
        AgentEvent::ToolExecutionUpdate {
            tool_call,
            partial_result,
        } => {
            json!({"type":"tool_execution_update","toolCallId":tool_call.id,"toolName":tool_call.name,"args":tool_call.arguments,"partialResult":tool_result(partial_result)})
        }
        AgentEvent::ToolExecutionEnd {
            tool_call,
            result,
            is_error,
        } => {
            json!({"type":"tool_execution_end","toolCallId":tool_call.id,"toolName":tool_call.name,"result":tool_result(result),"isError":is_error})
        }
    }
}
/// Project optional termination without inserting absent fields.
pub fn tool_result(result: &AgentToolResult) -> Value {
    let mut value = json!({"content":result.content,"details":result.details});
    if let Some(terminate) = result.terminate {
        value["terminate"] = json!(terminate);
    }
    value
}
