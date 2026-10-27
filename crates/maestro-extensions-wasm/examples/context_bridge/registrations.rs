//! Authored callbacks exercised by the original native registration owners.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionAPI, ExtensionResult, ToolRenderCall,
    ToolRenderContext, ToolRenderResult,
};
use serde_json::json;
use std::rc::Rc;

/// Registers callbacks with observable literal operands and native context reads.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_command(
        "canonical-completion",
        maestro_extensions_wasm::CommandOptions {
            description: Some(" literal command ".into()),
            handler: Rc::new(|args, context| {
                Box::pin(async move { Err(format!("{args}|{}", context.cwd()?)) })
            }),
            get_argument_completions: Some(Rc::new(|prefix| {
                Box::pin(async move {
                    Ok(Some(vec![maestro_extensions_wasm::AutocompleteItem {
                        value: format!("{prefix} literal value "),
                        label: " literal label ".into(),
                        description: Some(" literal description ".into()),
                    }]))
                })
            })),
        },
    )?;
    api.register_shortcut(
        "ctrl+alt+k",
        maestro_extensions_wasm::ShortcutOptions {
            description: Some(" literal shortcut ".into()),
            handler: Rc::new(|context| {
                Box::pin(async move { Err(format!("shortcut:{}", context.cwd()?)) })
            }),
        },
    )?;
    tool(api)?;
    shell(api)
}

/// Uses the original update callback, cancellation and per-call context capabilities.
fn tool(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: serde_json::from_value(json!({"name":"canonical-tool", "label":" Literal tool ", "description":" Literal tool description ", "parameters":{"type":"object"}})).map_err(|error| error.to_string())?,
        prepare_arguments: Some(Rc::new(|args, emitter| {
            emitter.emit("canonical-prepare", args.clone())?;
            Ok(json!({"prepared":args}))
        })),
        execute: Rc::new(|id, params, signal, update, context| Box::pin(async move {
            let details = json!({"id":id,"params":params,"cwd":context.cwd()?,"aborted":signal.ok_or("missing tool signal")?.aborted()});
            (update.ok_or("missing tool update")?)(serde_json::from_value(json!({"content":[],"details":details})).map_err(|error| error.to_string())?)?;
            serde_json::from_value(json!({"content":[{"type":"text","text":" literal result "}],"details":details,"terminate":false})).map_err(|error| error.to_string())
        })),
    })
}

/// Fixed lines a renderer returns.
struct Lines(Vec<String>);

impl ComponentView for Lines {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(self.0.clone())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A rendering's row fields, its count in the row's typed state and its previous component.
fn row_report(slot: &str, ctx: &ToolRenderContext) -> ExtensionResult<String> {
    let seen = ctx.state::<u32>();
    *seen.borrow_mut() += 1;
    let seen = *seen.borrow();
    if ctx.args.as_ref().is_some_and(|args| args["fail"] == true) {
        return Err("paint failed".into());
    }
    if ctx.args.as_ref().is_some_and(|args| args["redraw"] == true) {
        (ctx.invalidate)()?;
    }
    Ok(json!([
        slot,
        ctx.tool_call_id,
        ctx.args,
        ctx.cwd,
        ctx.phase.args_complete,
        ctx.expanded,
        seen,
        ctx.last_component.is_some()
    ])
    .to_string())
}

/// Registers the canonical tool or the painter tool.
pub(super) fn register_tool(api: &ExtensionAPI, args: &str) -> ExtensionResult<()> {
    if args == "register-painter" {
        return painter(api);
    }
    tool(api)
}

/// Registers a tool whose renderers report what they read about their row.
fn painter(api: &ExtensionAPI) -> ExtensionResult<()> {
    let render_call: ToolRenderCall = Rc::new(|args, _, ctx, _| {
        Ok(Component::new(Lines(vec![
            row_report("call", ctx)?,
            json!(args).to_string(),
        ])))
    });
    let render_result: ToolRenderResult = Rc::new(|result, options, _, ctx, _| {
        let report = row_report("result", ctx)?;
        Ok(Component::new(Lines(vec![
            report,
            json!(result).to_string(),
            json!([options.expanded, options.is_partial]).to_string(),
        ])))
    });
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        metadata: serde_json::from_value(json!({"name":"bridge-painter", "label":"Painter", "description":"Paints rows", "parameters":{"type":"object"}})).map_err(|error| error.to_string())?,
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, _, _| Box::pin(async { Err("not executed".to_owned()) })),
        render_call: Some(render_call),
        render_result: Some(render_result),
    })
}

/// Checks literal execution operands before streaming uninterpreted bytes.
fn shell_exec(
    command: String,
    cwd: String,
    options: maestro_extensions_wasm::BashExecOptions,
) -> maestro_extensions_wasm::ExtensionFuture<'static, maestro_extensions_wasm::BashExecResult> {
    Box::pin(async move {
        if command != " literal shell "
            || cwd != "./literal/../cwd"
            || options.timeout != Some(2.5)
            || !options.signal.ok_or("missing shell signal")?.aborted()
        {
            return Err("shell operands changed".into());
        }
        (options.on_data)(&[0, 255, 10])?;
        (options.on_data)(&[])?;
        Ok(maestro_extensions_wasm::BashExecResult {
            exit_code: Some(7.5),
        })
    })
}

/// Returns an authored executor without launching a process or adding timeout policy.
pub(super) fn shell(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.on(
        "user_bash",
        Rc::new(|event, _| {
            Box::pin(async move {
                let maestro_extensions_wasm::ExtensionEvent::UserBash(event) = event else {
                    return Err("expected shell event".into());
                };
                event.command.push_str(":edited");
                Ok(Some(
                    maestro_extensions_wasm::ExtensionEventResult::UserBash(
                        maestro_extensions_wasm::UserBashEventResult {
                            result: maestro_extensions_wasm::Presence::Missing,
                            operations: Some(maestro_extensions_wasm::BashOperations {
                                exec: Rc::new(shell_exec),
                            }),
                        },
                    ),
                ))
            })
        }),
    )
}

/// Registers the tool, painter or prefix callbacks selected by the author command.
pub(super) fn command(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
    args: &str,
) -> Option<ExtensionResult<()>> {
    match args {
        "register-tool" | "register-painter" => Some(register_tool(api, args)),
        "register-prefixes" => Some(super::prefixes::register(api, context.clone())),
        _ => None,
    }
}
