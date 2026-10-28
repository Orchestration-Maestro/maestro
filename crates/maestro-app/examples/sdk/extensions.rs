//! Register a local extension with lifecycle events, a custom tool and a command.
mod support;
use maestro_extensions::{EventError, ExtensionAPI};
use maestro_models::Local;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use support::{Error, Sample};

/// Run the inline-extension sample through the public SDK.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Invoke the extension command and a model-requested tool call.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::{ExtensionBindings, PromptOptions, create_agent_session};
    use maestro_models::{
        FauxAssistantMessageOptions, FauxResponseStep, FauxToolCallOptions, StopReason,
        faux_assistant_message, faux_tool_call,
    };
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&events);
    let mut options = sample.options()?;
    if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
        resources.extension_factories.push(Rc::new(move |api| {
            Box::pin(std::future::ready(register(&api, &log)))
        }));
    }
    let created = create_agent_session(options).await?;
    created
        .session
        .bind_extensions(ExtensionBindings::default())
        .await?;
    created
        .session
        .prompt("/mycommand demo", PromptOptions::default())
        .await?;
    let call = faux_tool_call(
        "my_tool",
        json!({"input": "hello"}),
        FauxToolCallOptions::default(),
    );
    sample.provider.set_responses(vec![
        FauxResponseStep::Message(Box::new(faux_assistant_message(
            call,
            FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::ToolUse),
                ..FauxAssistantMessageOptions::default()
            },
        ))),
        FauxResponseStep::Message(Box::new(faux_assistant_message(
            "Done.",
            FauxAssistantMessageOptions::default(),
        ))),
    ]);
    created
        .session
        .prompt("Process hello.", PromptOptions::default())
        .await?;
    let messages = serde_json::to_value(
        &*created
            .session
            .messages()
            .read()
            .map_err(|error| error.to_string())?,
    )?;
    let result = json!({"events": *events.borrow(), "tools": created.session.get_active_tool_names(),
        "toolResult": messages[2]["content"][0]["text"], "answer": messages[3]["content"][0]["text"]});
    created.session.dispose();
    Ok(result)
}

/// Register lifecycle logging and operations on the same retained extension API.
fn register(api: &ExtensionAPI<Local>, log: &Rc<RefCell<Vec<String>>>) -> Result<(), EventError> {
    use maestro_extensions::{
        AgentEndEvent, AgentStartEvent, NativeAgentMessage, ToolCallEvent, ToolCallEventResult,
    };
    let starts = Rc::clone(log);
    api.on::<AgentStartEvent, ()>(
        "agent_start".into(),
        Rc::new(move |_, _| {
            starts.borrow_mut().push("agent_start".into());
            Box::pin(std::future::ready(Ok(None)))
        }),
    )?;
    let tools = Rc::clone(log);
    api.on::<ToolCallEvent, ToolCallEventResult>(
        "tool_call".into(),
        Rc::new(move |event, _| {
            tools.borrow_mut().push(format!("tool:{}", event.tool_name));
            Box::pin(std::future::ready(Ok(None)))
        }),
    )?;
    let ends = Rc::clone(log);
    api.on::<AgentEndEvent<NativeAgentMessage>, ()>(
        "agent_end".into(),
        Rc::new(move |_, _| {
            ends.borrow_mut().push("agent_end".into());
            Box::pin(std::future::ready(Ok(None)))
        }),
    )?;
    let commands = Rc::clone(log);
    api.register_command(
        "mycommand".into(),
        maestro_extensions::types::RegisteredCommandOptions {
            description: Some("Record the supplied arguments".into()),
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                commands.borrow_mut().push(format!("command:{args}"));
                Box::pin(std::future::ready(Ok(())))
            }),
        },
    )?;
    api.register_tool(tool())
}

/// A schema-validated tool with executable local behavior and a prompt snippet.
fn tool()
-> maestro_tools::local::ToolDefinition<maestro_extensions::ExtensionContext, Value, Value, ()> {
    maestro_tools::ToolDefinitionRecord {
        definition: maestro_models::Tool {name: "my_tool".into(), description: "Process supplied text".into(),
            parameters: json!({"type": "object", "required": ["input"], "properties": {"input": {"type": "string"}}}).into()},
        label: "My Tool".into(), prompt_snippet: Some("Process text with my_tool".into()),
        prompt_guidelines: None, render_shell: None, prepare_arguments: None, execution_mode: None,
        execute: Rc::new(|_, params: Value, _, _, _| Box::pin(async move {
            let input = params["input"].as_str().ok_or_else(|| maestro_models::DiagnosticErrorInfo::error("input must be text"))?;
            Ok(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(maestro_models::TextContent::new(format!("Processed: {input}")))],
                details: Some(json!({})), terminate: None,
            })
        })),
        render_call: None, render_result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real inline factory receives lifecycle events and executes both registered operations.
    #[test]
    fn extensions_dispatches_commands_events_and_model_requested_tools() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(
            value["events"],
            json!(["command:demo", "agent_start", "tool:my_tool", "agent_end"])
        );
        assert_eq!(value["toolResult"], "Processed: hello");
        assert_eq!(value["answer"], "Done.");
        assert!(
            value["tools"]
                .as_array()
                .ok_or("missing tools")?
                .contains(&json!("my_tool"))
        );
        Ok(())
    }
}
