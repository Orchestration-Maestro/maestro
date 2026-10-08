//! Example extension: one input handler, one tool, one command and one renderer.
//!
//! The same function runs as a native-async component and under the controlled adapter.

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, AgentToolResult, Component, ComponentView, ExtensionAPI, ExtensionFuture,
    ExtensionHandler, InputReplacement, InputResult, NewSessionCommandOptions, ToolDefinition,
    ToolMetadata,
};
use serde_json::{Value, json};

/// Reports its label to the host when the callback that captured it is released.
struct Released {
    /// Extension handle used to report the release.
    api: ExtensionAPI,
    /// Name reported on release.
    label: &'static str,
}

impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.api.append_entry("released", Some(&json!(self.label)));
    }
}

/// A one-line view of a custom message.
struct NoteView {
    /// The rendered text.
    line: String,
    /// Extension handle used to report invalidation.
    api: ExtensionAPI,
    /// Reports the release of the view.
    _released: Released,
}

impl ComponentView for NoteView {
    fn render(&self, width: u32) -> Vec<String> {
        vec![format!("{width}|{}", self.line)]
    }

    fn invalidate(&self) {
        let _ = self.api.append_entry("invalidated", Some(&json!("note")));
    }
}

/// The extension factory both adapters run.
#[must_use]
pub fn extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        register_input(&api)?;
        register_tool(&api)?;
        register_command(&api)?;
        register_renderer(&api)
    })
}

/// Registers the input handler.
fn register_input(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released {
        api: api.clone(),
        label: "input-handler",
    };
    let log = api.clone();
    api.on(ExtensionHandler::input(move |event, ctx| {
        let _ = &released;
        let log = log.clone();
        Box::pin(async move {
            log.append_entry(
                "input",
                Some(&json!({ "text": event.text, "cwd": ctx.cwd()? })),
            )?;
            if event.text == "reject" {
                event.text.push('!');
                return Err("input rejected".to_owned());
            }
            Ok(Some(InputResult::Transform(InputReplacement {
                text: event.text.to_uppercase(),
                images: event.images.clone(),
            })))
        })
    }))
}

/// Registers the echo tool.
fn register_tool(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released {
        api: api.clone(),
        label: "tool-execute",
    };
    let remembered: Rc<RefCell<Option<AbortSignal>>> = Rc::default();
    let metadata = ToolMetadata {
        name: "echo".to_owned(),
        label: "Echo".to_owned(),
        description: "Echoes its text".to_owned(),
        parameters_schema: json!({ "type": "object" }).to_string(),
    };
    let tool = ToolDefinition::new(metadata, move |call_id, params, signal, update, _ctx| {
        let _ = &released;
        let previous = remembered.borrow().as_ref().map(AbortSignal::aborted);
        *remembered.borrow_mut() = signal;
        Box::pin(async move {
            if let Some(update) = update {
                update(AgentToolResult {
                    content: vec!["working".to_owned()],
                    details: None,
                    terminate: None,
                })?;
            }
            let text = params["text"].as_str().unwrap_or_default().to_owned();
            Ok(AgentToolResult {
                content: vec![format!("echo {text}")],
                details: Some(json!({ "call": call_id, "previous_aborted": previous })),
                terminate: None,
            })
        })
    })
    .with_prepare_arguments(|mut params: Value| {
        params["prepared"] = json!(true);
        Ok(params)
    });
    api.register_tool(tool)
}

/// Registers the command that replaces the session.
fn register_command(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released {
        api: api.clone(),
        label: "command-handler",
    };
    let log = api.clone();
    api.register_command("replace", move |args, ctx| {
        let _ = &released;
        let log = log.clone();
        Box::pin(async move {
            let before = ctx.cwd()?;
            log.append_entry("command", Some(&json!({ "args": args, "cwd": before })))?;
            let outcome = ctx.new_session(Some(replacement_options(&log))).await?;
            if let Err(stale) = ctx.cwd() {
                log.append_entry("stale", Some(&json!(stale)))?;
            }
            log.append_entry(
                "after",
                Some(&json!({ "before": before, "cancelled": outcome.cancelled })),
            )
        })
    })
}

/// Options whose continuation runs against the replacement session.
fn replacement_options(log: &ExtensionAPI) -> NewSessionCommandOptions {
    let continuation = Released {
        api: log.clone(),
        label: "continuation",
    };
    let in_session = log.clone();
    NewSessionCommandOptions {
        parent_session: Some("parent".to_owned()),
        with_session: Some(Box::new(move |replaced| {
            Box::pin(async move {
                let _ = &continuation;
                in_session
                    .append_entry("continuation", Some(&json!({ "cwd": replaced.cwd()? })))?;
                replaced.send_user_message("hello from continuation").await
            })
        })),
    }
}

/// Registers the renderer of the note message type.
fn register_renderer(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released {
        api: api.clone(),
        label: "renderer",
    };
    let log = api.clone();
    api.register_message_renderer("note", move |message, expanded| {
        let _ = &released;
        let line = format!(
            "{}|{}",
            message.content,
            if expanded { "expanded" } else { "collapsed" }
        );
        let released = Released {
            api: log.clone(),
            label: "note-view",
        };
        Ok(Some(Component::new(NoteView {
            line,
            api: log.clone(),
            _released: released,
        })))
    })
}
