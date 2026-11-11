//! Register a local extension with lifecycle events, a custom tool and a command.
#[path = "support/stream.rs"]
mod stream;
mod support;
use maestro_extensions::{EventError, ExtensionAPI};
use maestro_models::Local;
use serde_json::{Value, json};
use std::io::Write;
use std::rc::Rc;
use support::{Error, Sample};

/// Run the inline-extension sample through the public SDK.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Invoke the extension command and a model-requested tool call.
async fn run(sample: Sample) -> Result<(), Error> {
    use maestro_app::{ExtensionBindings, PromptOptions, create_agent_session};
    let mut options = sample.options()?;
    if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
        resources
            .extension_factories
            .push(Rc::new(|api| Box::pin(std::future::ready(register(&api)))));
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
    sample.provider.set_responses(responses());
    let finish = stream::subscribe(&created.session);
    created
        .session
        .prompt("Process hello.", PromptOptions::default())
        .await?;
    finish()?;
    writeln!(std::io::stdout())?;
    created.session.dispose();
    Ok(())
}

/// A controlled tool request followed by its completed answer.
fn responses() -> Vec<maestro_models::FauxResponseStep> {
    use maestro_models::{
        FauxAssistantMessageOptions, FauxResponseStep, FauxToolCallOptions, StopReason,
        faux_assistant_message, faux_tool_call,
    };
    let call = faux_tool_call(
        "my_tool",
        json!({"input": "hello"}),
        FauxToolCallOptions::default(),
    );
    vec![
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
    ]
}

/// Print one line from the inline extension and let the event continue.
fn announce<T>(line: &str) -> Result<Option<T>, EventError> {
    writeln!(std::io::stdout(), "[Inline Extension] {line}")?;
    Ok(None)
}

/// Register lifecycle logging and operations on the same retained extension API.
fn register(api: &ExtensionAPI<Local>) -> Result<(), EventError> {
    use maestro_extensions::{
        AgentEndEvent, AgentStartEvent, NativeAgentMessage, ToolCallEvent, ToolCallEventResult,
    };
    api.on::<AgentStartEvent, ()>(
        "agent_start".into(),
        Rc::new(|_, _| Box::pin(std::future::ready(announce("Agent starting")))),
    )?;
    api.on::<ToolCallEvent, ToolCallEventResult>(
        "tool_call".into(),
        Rc::new(|event, _| {
            let line = format!("Tool: {}", event.tool_name);
            Box::pin(std::future::ready(announce(&line)))
        }),
    )?;
    api.on::<AgentEndEvent<NativeAgentMessage>, ()>(
        "agent_end".into(),
        Rc::new(|_, _| Box::pin(std::future::ready(announce("Done")))),
    )?;
    api.register_command(
        "mycommand".into(),
        maestro_extensions::types::RegisteredCommandOptions {
            description: Some("Print the supplied arguments".into()),
            get_argument_completions: None,
            handler: Rc::new(|args, _| {
                let line = format!("Command: {args}");
                Box::pin(std::future::ready(announce::<()>(&line).map(|_| ())))
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
