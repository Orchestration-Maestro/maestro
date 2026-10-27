//! Labelled registrations whose callbacks report the label that registered them.
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, CommandOptions, ExtensionAPI, ExtensionEventResult, ExtensionResult,
    FlagOptions, FlagType, FlagValue, ShortcutOptions, ToolDefinition,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// Names to register, each reporting `label` when invoked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registrations {
    /// Distinguishes this registration set from another instance's or an earlier one.
    label: String,
    /// Command names.
    #[serde(default)]
    commands: Vec<String>,
    /// Tool names.
    #[serde(default)]
    tools: Vec<String>,
    /// Custom message types to render.
    #[serde(default)]
    renderers: Vec<String>,
    /// Shortcut keys.
    #[serde(default)]
    shortcuts: Vec<String>,
    /// String flags whose default is the label.
    #[serde(default)]
    flags: Vec<String>,
    /// Event names to handle.
    #[serde(default)]
    events: Vec<String>,
}

/// Reports one invocation on the trace channel.
async fn trace(api: ExtensionAPI, value: Value) -> ExtensionResult<()> {
    let events = api.events();
    events.emit("trace", value).await
}

impl Registrations {
    /// Registers every listed name through the public facade.
    pub(super) fn register(self, api: &ExtensionAPI) -> ExtensionResult<()> {
        let label = &self.label;
        for name in &self.commands {
            command(api, name, label)?;
        }
        for name in &self.tools {
            api.register_tool(tool(name, label)?)?;
        }
        for name in &self.renderers {
            renderer(api, name, label)?;
        }
        for key in &self.shortcuts {
            let (owner, label) = (api.clone(), label.clone());
            api.register_shortcut(
                key,
                ShortcutOptions {
                    description: None,
                    handler: Rc::new(move |_| {
                        Box::pin(trace(owner.clone(), json!({ "shortcut": label })))
                    }),
                },
            )?;
        }
        for name in &self.flags {
            api.register_flag(
                name,
                FlagOptions {
                    description: None,
                    type_: FlagType::String,
                    default: Some(FlagValue::String(label.clone())),
                },
            )?;
        }
        for event in &self.events {
            let (owner, label) = (api.clone(), label.clone());
            api.on(
                event,
                Rc::new(move |_, _| Box::pin(handled(owner.clone(), label.clone()))),
            )?;
        }
        Ok(())
    }
}

/// Reports a handled event and returns no result.
async fn handled(
    api: ExtensionAPI,
    label: String,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    trace(api, json!({ "event": label })).await?;
    Ok(None)
}

/// A command that reports its label and arguments.
fn command(api: &ExtensionAPI, name: &str, label: &str) -> ExtensionResult<()> {
    let (owner, label) = (api.clone(), label.to_owned());
    api.register_command(
        name,
        CommandOptions {
            description: Some(label.clone()),
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let value = json!({ "command": label, "args": args });
                Box::pin(trace(owner.clone(), value))
            }),
        },
    )
}

/// A renderer that reports its label and the message type, and renders nothing.
fn renderer(api: &ExtensionAPI, name: &str, label: &str) -> ExtensionResult<()> {
    let label = label.to_owned();
    api.register_message_renderer(
        name,
        Rc::new(move |message, _, _, emitter| {
            let value = json!({ "renderer": label, "message": message.custom_type });
            emitter.emit("trace", value)?;
            Ok(None)
        }),
    )
}

/// A tool that prepares its arguments, reports progress and answers with its label.
fn tool(name: &str, label: &str) -> ExtensionResult<ToolDefinition> {
    let metadata = json!({
        "name": name,
        "label": label,
        "description": label,
        "parameters": { "type": "object", "properties": { "n": { "type": "number" } } },
    });
    let (prepared, executed) = (label.to_owned(), label.to_owned());
    Ok(ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: serde_json::from_value(metadata).map_err(|e| e.to_string())?,
        prepare_arguments: Some(Rc::new(move |args, _| {
            Ok(json!({ "prepared": prepared, "args": args }))
        })),
        execute: Rc::new(move |id, params, _, update, _| {
            let label = executed.clone();
            Box::pin(async move {
                let progress = json!({ "content": [], "details": { "progress": label } });
                (update.ok_or("missing tool update")?)(
                    serde_json::from_value::<AgentToolResult>(progress)
                        .map_err(|e| e.to_string())?,
                )?;
                let result = json!({
                    "content": [{ "type": "text", "text": label }],
                    "details": { "id": id, "params": params },
                });
                serde_json::from_value(result).map_err(|e| e.to_string())
            })
        }),
    })
}
